use crate::config::Config;
use crate::executor::TransactionExecutor;
use crate::mempool::Mempool;
use crate::metrics::NodeMetrics;
use crate::task_dispatcher::{DispatcherConfig, TaskDispatcher};

/// Maximum allowed clock drift (in seconds) for future block timestamps.
pub(crate) const MAX_BLOCK_CLOCK_DRIFT_SECS: u64 = 30;

/// How often (in blocks) to run receipt pruning.
pub(crate) const PRUNING_INTERVAL: u64 = 1000;

/// Number of recent blocks whose receipts are kept during pruning.
pub(crate) const KEEP_RECEIPTS_BLOCKS: u64 = 10000;

/// Maximum gas limit per block
pub(crate) const BLOCK_GAS_LIMIT: u64 = 30_000_000;

/// Maximum number of transactions per block
pub(crate) const MAX_TRANSACTIONS_PER_BLOCK: usize = 1000;

/// Round-robin leader selection (fallback for bootstrap when PoS has no stake data)
pub(crate) fn is_leader_for_slot(validator_id: &str, slot: u64, validators: &[String]) -> bool {
    if validators.is_empty() {
        // ⚠️ This should rarely be reached in block production (block_production.rs
        // uses is_solo_leader_for_slot instead). If you see this, register validators.
        return true;
    }
    let leader_index = (slot % validators.len() as u64) as usize;
    validators.get(leader_index).map_or(false, |leader| leader == validator_id)
}

use crate::graceful_shutdown::{GracefulShutdown, ShutdownConfig};
use crate::health::{HealthConfig, HealthMonitor};
use anyhow::{Context, Result};
use luxtensor_consensus::fast_finality::FastFinality;
use luxtensor_consensus::fork_choice::ForkChoice;
use luxtensor_consensus::liveness::{LivenessConfig, LivenessMonitor};
use luxtensor_consensus::long_range_protection::{LongRangeConfig, LongRangeProtection};
use luxtensor_consensus::randao::{RandaoConfig, RandaoMixer};
use luxtensor_consensus::slashing::{SlashingConfig, SlashingManager};
use luxtensor_consensus::{
    ConsensusConfig, NodeRegistry, ProofOfStake, RewardExecutor,
    TokenAllocation,
};
use luxtensor_consensus::weight_consensus::{VTrustScorer, VTrustSnapshot};
use luxtensor_core::{Block, StateDB};
use luxtensor_storage::CachedStateDB;
use luxtensor_core::bridge::{BridgeConfig, InMemoryBridge};
use luxtensor_core::multisig::MultisigManager;
use luxtensor_crypto::KeyPair;
use luxtensor_network::eclipse_protection::{EclipseConfig, EclipseProtection};
use luxtensor_network::rate_limiter::{
    RateLimiter as NetworkRateLimiter, RateLimiterConfig as NetworkRateLimiterConfig,
};
use luxtensor_network::SwarmCommand;

use luxtensor_rpc::BroadcastEvent;
use luxtensor_contracts::{AgentRegistry, AgentTriggerEngine, EvmExecutor};
use luxtensor_oracle::DisputeManager;
use luxtensor_storage::maintenance::{BackupConfig, DbMaintenance, PruningConfig};
use luxtensor_storage::metagraph_store::MetagraphDB;
use luxtensor_storage::BlockchainDB;

use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Get current Unix timestamp (seconds since epoch)
/// Panics only if system time is before Unix epoch (practically impossible)
#[inline]
pub(crate) fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_secs()
}

/// Parse a hex address string (with or without 0x prefix) into [u8; 20]
pub(crate) fn parse_address_from_hex(addr_str: &str) -> Result<[u8; 20]> {
    let addr_str = addr_str.strip_prefix("0x").unwrap_or(addr_str);
    if addr_str.len() != 40 {
        return Err(anyhow::anyhow!("Invalid address length"));
    }
    let bytes =
        hex::decode(addr_str).context(format!("Failed to decode hex address: {}", addr_str))?;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Ok(addr)
}

/// Detect external IP address using local network interfaces
/// Returns the first non-loopback IPv4 address found
pub(crate) fn detect_external_ip() -> Option<String> {
    // Try to get local IP by connecting to a public address (doesn't actually send data)
    use std::net::UdpSocket;

    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        // Connect to Google's DNS to determine local IP that would be used for external traffic
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                let ip = addr.ip().to_string();
                // Don't return loopback or link-local addresses
                if !ip.starts_with("127.") && !ip.starts_with("169.254.") {
                    return Some(ip);
                }
            }
        }
    }

    None
}

/// Convert PeerId to synthetic IP for subnet diversity tracking
/// This is a hash-based approach since libp2p PeerIds don't directly contain IPs
pub(crate) fn peer_id_to_synthetic_ip(peer_id: &str) -> std::net::IpAddr {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    peer_id.hash(&mut hasher);
    let hash = hasher.finish();

    // Create a synthetic IPv4 from the hash for subnet diversity calculation
    let bytes = hash.to_be_bytes();
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))
}

/// Node service that orchestrates all components
#[allow(dead_code)] // Some fields are held for Arc ownership/lifecycle management and not read directly
pub struct NodeService {
    pub(crate) config: Config,
    pub(crate) storage: Arc<BlockchainDB>,
    pub(crate) state_db: Arc<RwLock<StateDB>>,
    pub(crate) consensus: Arc<RwLock<ProofOfStake>>,
    pub(crate) mempool: Arc<Mempool>,
    pub(crate) executor: Arc<TransactionExecutor>,
    pub(crate) reward_executor: Arc<RwLock<RewardExecutor>>,
    pub(crate) token_allocation: Arc<RwLock<TokenAllocation>>,
    pub(crate) node_registry: Arc<RwLock<NodeRegistry>>,
    pub(crate) shutdown_tx: broadcast::Sender<()>,
    pub(crate) tasks: Vec<JoinHandle<Result<()>>>,
    pub(crate) epoch_length: u64,
    /// Broadcast channel to P2P swarm for sending blocks/txs
    /// 🔧 FIX: Use bounded mpsc::Sender to match SwarmP2PNode::with_keypair() return type
    pub(crate) broadcast_tx: Option<mpsc::Sender<SwarmCommand>>,
    /// Genesis timestamp for slot calculation
    pub(crate) genesis_timestamp: u64,
    /// Validator keypair for block signing (None if not a validator)
    pub(crate) validator_keypair: Option<KeyPair>,
    /// Database maintenance (backup/restore/pruning)
    pub(crate) db_maintenance: Arc<DbMaintenance>,
    /// Eclipse attack protection
    pub(crate) eclipse_protection: Arc<EclipseProtection>,
    /// Long-range attack protection
    pub(crate) long_range_protection: Arc<LongRangeProtection>,
    /// Liveness monitor for detecting stalled validators
    pub(crate) liveness_monitor: Arc<RwLock<LivenessMonitor>>,
    /// Graceful shutdown handler
    pub(crate) graceful_shutdown: Arc<GracefulShutdown>,
    /// Health monitor for node health checks
    pub(crate) health_monitor: Arc<RwLock<HealthMonitor>>,
    /// Fast finality: BFT-style instant finality via validator signatures
    pub(crate) fast_finality: Arc<RwLock<FastFinality>>,
    /// Fork choice rule (longest chain / heaviest observed chain)
    pub(crate) fork_choice: Arc<RwLock<ForkChoice>>,
    /// AI Task Dispatcher for DePIN workload distribution
    pub(crate) task_dispatcher: Arc<TaskDispatcher>,
    /// Metagraph database for AI subnet/neuron metadata
    pub(crate) metagraph_db: Arc<MetagraphDB>,
    /// RANDAO randomness mixer for unbiased leader/task selection
    pub(crate) randao: Arc<RwLock<RandaoMixer>>,
    /// Slashing manager for punishing misbehaviour
    pub(crate) slashing_manager: Arc<RwLock<SlashingManager>>,
    /// Network-layer rate limiter for P2P flood protection
    pub(crate) network_rate_limiter: Arc<NetworkRateLimiter>,
    /// Atomic height guard to prevent block height race between P2P and block production
    pub(crate) best_height_guard: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Sync guard: block production pauses while this is true (prevents race with chain sync)
    pub(crate) is_syncing: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Agent registry for autonomous AI agents (Agentic EVM)
    pub(crate) agent_registry: Arc<AgentRegistry>,
    /// Agent trigger engine for block-level autonomous execution
    pub(crate) agent_trigger_engine: Arc<AgentTriggerEngine>,
    /// Dispute manager for optimistic AI fraud proofs
    pub(crate) dispute_manager: Arc<DisputeManager>,
    /// Cross-chain bridge for asset transfers
    pub(crate) bridge: Arc<InMemoryBridge>,
    /// Multisig wallet manager for multi-signature transactions
    pub(crate) multisig_manager: Arc<MultisigManager>,
    /// Merkle root caching layer wrapping state_db for efficient block production
    pub(crate) merkle_cache: Arc<CachedStateDB>,
    /// Node-level Prometheus metrics (block height, peers, TX throughput, etc.)
    pub(crate) metrics: Arc<NodeMetrics>,
    /// WebSocket broadcast sender for real-time event notifications
    pub(crate) ws_broadcast: Option<tokio::sync::mpsc::Sender<BroadcastEvent>>,
    /// VTrust scorer for validator trust scoring (persisted across restarts — L-NEW-1 fix)
    pub(crate) vtrust_scorer: Arc<RwLock<VTrustScorer>>,
    /// Path to VTrust snapshot file (bincode-serialized VTrustSnapshot)
    pub(crate) vtrust_snapshot_path: std::path::PathBuf,
}

impl NodeService {
    /// Create a new node service
    pub async fn new(config: Config) -> Result<Self> {
        info!("🦀 Initializing LuxTensor Node v{}", env!("CARGO_PKG_VERSION"));
        info!("Node name: {}", config.node.name);
        info!("Chain ID: {}", config.node.chain_id);

        // Validate configuration
        config.validate()?;

        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&config.node.data_dir)
            .context(format!("Failed to create data directory: {:?}", config.node.data_dir))?;
        std::fs::create_dir_all(&config.storage.db_path)
            .context(format!("Failed to create storage directory: {:?}", config.storage.db_path))?;

        // Initialize storage
        info!("📦 Initializing storage...");
        let db_path_str = config
            .storage
            .db_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?;
        let storage = Arc::new(BlockchainDB::open(db_path_str)?);
        let initial_best_height = storage.get_best_height().unwrap_or(Some(0)).unwrap_or(0);
        info!("  ✓ Storage initialized at {:?}", config.storage.db_path);

        // Initialize state database
        info!("💾 Initializing state database...");
        let state_db = Arc::new(RwLock::new(StateDB::new()));

        // 🔧 FIX: Restore persisted state from RocksDB on startup
        {
            let mut state = state_db.write();
            match state.load_from_db(storage.as_ref()) {
                Ok(count) if count > 0 => {
                    info!("  ✓ Restored {} accounts from disk", count);
                }
                Ok(_) => {
                    info!("  ✓ No persisted state found (fresh node)");
                }
                Err(e) => {
                    warn!("  ⚠️ Failed to load persisted state: {} (starting fresh)", e);
                }
            }
        }

        // Wire lazy bytecode loader: StateDB will load contract code from
        // CF_CONTRACTS on demand rather than keeping it all in memory.
        {
            let mut state = state_db.write();
            state.set_code_store(storage.clone());
        }

        info!("  ✓ State database initialized (lazy bytecode loading enabled)");

        // Initialize Merkle cache wrapping a storage-layer StateDB (RocksDB-backed)
        let storage_state_db = Arc::new(parking_lot::RwLock::new(
            luxtensor_storage::StateDB::new(storage.inner_db()),
        ));
        let merkle_cache = Arc::new(CachedStateDB::with_defaults(storage_state_db));
        info!("  ✓ Merkle cache initialized (height_cache=256, account_hashes=4096)");

        // Initialize transaction executor
        info!("⚡ Initializing transaction executor...");
        let executor = Arc::new(TransactionExecutor::new(config.node.chain_id));
        info!("  ✓ Transaction executor initialized (chain_id: {})", config.node.chain_id);

        // Initialize consensus
        info!("⚖️  Initializing consensus...");
        let consensus_config = ConsensusConfig {
            slot_duration: config.consensus.block_time,
            min_stake: config.consensus.min_stake.parse().map_err(|e| {
                anyhow::anyhow!(
                    "min_stake '{}' is not a valid u128: {}",
                    config.consensus.min_stake,
                    e
                )
            })?,
            block_reward: luxtensor_core::constants::tokenomics::INITIAL_BLOCK_REWARD,
            epoch_length: config.consensus.epoch_length,
            ..Default::default()
        };
        let consensus = Arc::new(RwLock::new(ProofOfStake::new(consensus_config)));
        info!("  ✓ PoS consensus initialized");
        info!("    - Min stake: {}", config.consensus.min_stake);
        info!("    - Max validators: {}", config.consensus.max_validators);
        info!("    - Epoch length: {} blocks", config.consensus.epoch_length);

        // Initialize VTrust scorer — load persisted snapshot if available (L-NEW-1 fix)
        let vtrust_snapshot_path = config.node.data_dir.join("vtrust_snapshot.bin");
        let vtrust_scorer = {
            let mut scorer = VTrustScorer::new();
            match std::fs::read(&vtrust_snapshot_path) {
                Ok(bytes) => match bincode::deserialize::<VTrustSnapshot>(&bytes) {
                    Ok(snapshot) => {
                        scorer.restore(snapshot);
                        info!("  ✓ VTrust scorer restored from {:?}", vtrust_snapshot_path);
                    }
                    Err(e) => {
                        warn!("  ⚠️ Failed to deserialize VTrust snapshot: {} (starting fresh)", e);
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    info!("  ✓ No VTrust snapshot found (fresh node)");
                }
                Err(e) => {
                    warn!("  ⚠️ Failed to read VTrust snapshot file: {} (starting fresh)", e);
                }
            }
            Arc::new(RwLock::new(scorer))
        };
        info!("  ✓ VTrust scorer initialized");

        // Initialize mempool
        info!("📝 Initializing transaction mempool...");
        let mempool = Arc::new(Mempool::new(config.mempool.max_size, config.node.chain_id));
        info!(
            "  ✓ Mempool initialized (max size: {}, chain_id: {})",
            config.mempool.max_size, config.node.chain_id
        );

        // Check if genesis block exists, create if not
        if storage.get_block_by_height(0)?.is_none() {
            info!("🌱 Creating genesis block...");
            let genesis = Block::genesis();
            storage.store_block(&genesis)?;
            info!("  ✓ Genesis block created: {:?}", genesis.hash());
        } else {
            info!("  ✓ Genesis block found");
        }

        // Initialize development accounts ONLY in dev mode
        // In production, genesis balances should come from the genesis block/config
        if config.node.dev_mode {
            let dev_accounts: &[[u8; 20]] = &[
                [
                    0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8, 0x82,
                    0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66,
                ],
                [
                    0x70, 0x99, 0x79, 0x70, 0xc5, 0x18, 0x12, 0xdc, 0x3a, 0x01, 0x0c, 0x7d, 0x01,
                    0xb5, 0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xc8,
                ],
                [
                    0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58, 0x5d, 0xd2, 0x99,
                    0xe0, 0x3d, 0x12, 0xfa, 0x42, 0x93, 0xbc,
                ],
            ];

            for addr_bytes in dev_accounts {
                let dev_address = luxtensor_core::Address::from(*addr_bytes);
                let mut dev_account = luxtensor_core::Account::new();
                dev_account.balance = 10_000_000_000_000_000_000_000_u128; // 10000 ETH in wei
                state_db.write().set_account(dev_address, dev_account);
            }
            warn!(
                "⚠️  DEV MODE: {} genesis accounts initialized with 10000 ETH each",
                dev_accounts.len()
            );
        }

        // Initialize reward executor for epoch processing
        let dao_address = parse_address_from_hex(&config.node.dao_address).unwrap_or_else(|_| {
            warn!("Invalid DAO address in config, using default zero address");
            [0u8; 20]
        });
        let reward_executor = Arc::new(RwLock::new(RewardExecutor::new(dao_address)));
        info!("  ✓ Reward executor initialized with DAO: 0x{}", hex::encode(&dao_address));

        // Initialize token allocation for TGE and vesting
        let tge_timestamp = current_timestamp();
        let token_allocation = Arc::new(RwLock::new(TokenAllocation::new(tge_timestamp)));
        info!("  ✓ Token allocation initialized");

        // Initialize node registry for progressive staking
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        info!("  ✓ Node registry initialized");

        // Initialize database maintenance (backup/restore/pruning)
        let db_maintenance = Arc::new(DbMaintenance::new(
            config.storage.db_path.clone(),
            BackupConfig {
                backup_dir: config.node.data_dir.join("backups"),
                max_backups: 5,
                compress: true,
            },
            PruningConfig::default(),
        ));
        info!("  ✓ Database maintenance initialized");

        // Initialize eclipse attack protection
        let eclipse_protection = Arc::new(EclipseProtection::new(EclipseConfig::default()));
        info!("  ✓ Eclipse protection initialized");

        // Initialize long-range attack protection
        let genesis_hash = storage
            .get_block_by_height(0)
            .context("Failed to load genesis block for long-range protection")?
            .map(|b| b.hash())
            .unwrap_or([0u8; 32]);
        let long_range_protection =
            Arc::new(LongRangeProtection::new(LongRangeConfig::default(), genesis_hash));
        info!("  ✓ Long-range protection initialized");

        // Initialize liveness monitor
        let liveness_monitor =
            Arc::new(RwLock::new(LivenessMonitor::new(LivenessConfig::default())));
        info!("  ✓ Liveness monitor initialized");

        // Initialize graceful shutdown handler
        let graceful_shutdown = Arc::new(GracefulShutdown::new(ShutdownConfig::default()));
        info!("  ✓ Graceful shutdown handler initialized");

        // Restore mempool from previous shutdown backup (if exists)
        {
            let mempool_backup_path = format!("{}/mempool.bin", graceful_shutdown.config().backup_dir);
            match mempool.load_from_file(&mempool_backup_path) {
                Ok(0) => {} // No backup found or empty — silent
                Ok(count) => info!("  ✓ Restored {} pending transactions from previous session", count),
                Err(e) => warn!("  ⚠️ Failed to load mempool backup: {} (starting fresh)", e),
            }
        }

        // Initialize health monitor
        let health_monitor = Arc::new(RwLock::new(HealthMonitor::new(HealthConfig::default())));
        info!("  ✓ Health monitor initialized");

        // Initialize FastFinality (BFT-style finality via 2/3 validator signatures)
        let fast_finality = Arc::new(RwLock::new(
            FastFinality::new(67, luxtensor_consensus::ValidatorSet::new())
                .map_err(|e| anyhow::anyhow!("FastFinality init failed: {}", e))?,
        ));
        info!("  ✓ Fast finality initialized (threshold: 67%)");

        // Initialize ForkChoice (GHOST algorithm for canonical chain selection)
        let genesis_block = storage
            .get_block_by_height(0)
            .context("Failed to read genesis block for fork choice initialization")?
            .ok_or_else(|| anyhow::anyhow!("Genesis block required for fork choice"))?;
        let fork_choice = Arc::new(RwLock::new(ForkChoice::new(genesis_block)));
        info!("  ✓ Fork choice (GHOST) initialized");

        // Initialize Metagraph DB for AI subnet/neuron metadata
        let metagraph_path = config.node.data_dir.join("metagraph");
        std::fs::create_dir_all(&metagraph_path)
            .context(format!("Failed to create metagraph directory: {:?}", metagraph_path))?;
        let metagraph_db = Arc::new(
            MetagraphDB::open(&metagraph_path)
                .context(format!("Failed to open metagraph DB at {:?}", metagraph_path))?,
        );
        info!("  ✓ Metagraph DB initialized at {:?}", metagraph_path);

        // Initialize AI Task Dispatcher for DePIN workload distribution
        let task_dispatcher =
            Arc::new(TaskDispatcher::new(metagraph_db.clone(), DispatcherConfig::default()));
        info!("  ✓ AI Task Dispatcher initialized");

        // Initialize RANDAO mixer for unbiased randomness accumulation
        let randao =
            Arc::new(RwLock::new(RandaoMixer::with_genesis(RandaoConfig::default(), genesis_hash)));
        info!("  ✓ RANDAO mixer initialized");

        // Initialize Slashing manager
        let slashing_manager = Arc::new(RwLock::new(SlashingManager::new(
            SlashingConfig::default(),
            Arc::new(RwLock::new(luxtensor_consensus::ValidatorSet::new())),
        )));
        info!("  ✓ Slashing manager initialized");

        // Initialize Agentic EVM — Agent Registry + Trigger Engine
        let agent_registry = Arc::new(AgentRegistry::with_defaults());
        let agent_evm = Arc::new(EvmExecutor::new(config.node.chain_id as u64));
        let agent_trigger_engine = Arc::new(AgentTriggerEngine::new(
            agent_registry.clone(),
            agent_evm,
        ));
        info!("  ✓ Agentic EVM initialized (agent registry + trigger engine)");

        // Initialize Dispute Manager for optimistic AI execution
        let dispute_manager = Arc::new(DisputeManager::default_config());
        info!("  ✓ Dispute Manager initialized (optimistic AI fraud proofs)");

        // Initialize Cross-Chain Bridge (in-memory for now, swap for persistent later)
        let bridge = Arc::new(InMemoryBridge::new(BridgeConfig::default()));
        info!("  ✓ Cross-Chain Bridge initialized (lock-and-mint / burn-and-release)");

        // Initialize Multisig Wallet Manager
        let multisig_manager = Arc::new(MultisigManager::new());
        info!("  ✓ Multisig Manager initialized (N-of-M signature wallets)");

        // Initialize network-layer rate limiter
        let network_rate_limiter = Arc::new(NetworkRateLimiter::new(NetworkRateLimiterConfig {
            requests_per_second: 50, // 50 msgs/s per peer
            burst_size: 100,         // allow short bursts
            ban_duration: std::time::Duration::from_secs(300),
            violations_before_ban: 10,
        }));
        info!("  ✓ Network rate limiter initialized");

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(16);

        // Get epoch length from consensus config
        let epoch_length = config.consensus.epoch_length;

        // Get genesis timestamp from genesis block (for slot calculation)
        // This ensures all nodes use the same genesis timestamp from the chain
        let genesis_timestamp = storage
            .get_block_by_height(0)?
            .map(|block| block.header.timestamp)
            .unwrap_or_else(|| {
                tracing::warn!(
                    "No genesis block found — using current system time as genesis timestamp. \
                     This node should sync the genesis block before participating in consensus."
                );
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_secs()
            });

        // Load validator keypair if configured
        let validator_keypair = if config.node.is_validator {
            if let Some(key_path) = &config.node.validator_key_path {
                match std::fs::read(key_path) {
                    Ok(key_bytes) if key_bytes.len() >= 32 => {
                        let mut secret = [0u8; 32];
                        secret.copy_from_slice(&key_bytes[..32]);
                        let result = KeyPair::from_secret(&secret);

                        // ── 🎲 VRF key loading (production-vrf feature) ──────────────
                        // Derive the VRF keypair from the same 32-byte secret so validators
                        // don't need a separate key file.  Feature-gated so the standard
                        // build stays unchanged.
                        #[cfg(feature = "production-vrf")]
                        {
                            match consensus.read().set_vrf_key(&secret) {
                                Ok(()) => info!("🎲 VRF key loaded (production-vrf)"),
                                Err(e) => warn!("Failed to set VRF key: {}", e),
                            }
                        }

                        // SECURITY: Zeroize secret key bytes after use
                        secret.iter_mut().for_each(|b| *b = 0);
                        match result {
                            Ok(keypair) => {
                                let address = keypair.address();
                                info!(
                                    "🔑 Loaded validator key, address: 0x{}",
                                    hex::encode(&address)
                                );
                                Some(keypair)
                            }
                            Err(e) => {
                                warn!("Failed to parse validator key: {}", e);
                                None
                            }
                        }
                    }
                    Ok(_) => {
                        warn!("Validator key file too short, need at least 32 bytes");
                        None
                    }
                    Err(e) => {
                        warn!("Could not read validator key file: {}", e);
                        None
                    }
                }
            } else {
                warn!("Validator mode enabled but no key path configured, blocks will be unsigned");
                None
            }
        } else {
            None
        };

        Ok(Self {
            config,
            storage,
            state_db,
            consensus,
            mempool,
            executor,
            reward_executor,
            token_allocation,
            node_registry,
            shutdown_tx,
            tasks: Vec::new(),
            epoch_length,
            broadcast_tx: None, // Will be initialized in start()
            genesis_timestamp,
            validator_keypair,
            db_maintenance,
            eclipse_protection,
            long_range_protection,
            liveness_monitor,
            graceful_shutdown,
            health_monitor,
            fast_finality,
            fork_choice,
            task_dispatcher,
            metagraph_db,
            randao,
            slashing_manager,
            network_rate_limiter,
            best_height_guard: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(
                initial_best_height,
            )),
            // Start as syncing=true; block production pauses until sync completes or timeout
            is_syncing: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            agent_registry,
            agent_trigger_engine,
            dispute_manager,
            bridge,
            multisig_manager,
            merkle_cache,
            metrics: Arc::new(NodeMetrics::default()),
            ws_broadcast: None,
            vtrust_scorer,
            vtrust_snapshot_path,
        })
    }

    // NOTE: `start()` has been moved to `startup.rs`
    // NOTE: Block production functions have been moved to `block_production.rs`

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&mut self) -> Result<()> {
        info!("Node is running. Press Ctrl+C to shutdown.");

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("Received shutdown signal");

        self.shutdown().await
    }

    /// Shutdown all services
    async fn shutdown(&mut self) -> Result<()> {
        info!("🛑 Shutting down node services...");

        // Start graceful shutdown sequence
        self.graceful_shutdown.initiate_shutdown();

        // Send shutdown signal to all tasks
        let _ = self.shutdown_tx.send(());

        // Wait for all tasks to complete with timeout from graceful_shutdown
        for task in self.tasks.drain(..) {
            match task.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => warn!("Task error during shutdown: {}", e),
                Err(e) => error!("Task panicked during shutdown: {}", e),
            }
        }

        // Flush storage
        info!("💾 Flushing storage...");
        // Storage flush happens automatically on drop

        // Save shutdown checkpoint for recovery
        self.graceful_shutdown.begin_state_save();

        // Persist VTrust scorer snapshot for next startup (L-NEW-1 fix)
        {
            let snapshot = self.vtrust_scorer.read().snapshot();
            match bincode::serialize(&snapshot) {
                Ok(bytes) => {
                    if let Err(e) = std::fs::write(&self.vtrust_snapshot_path, &bytes) {
                        warn!("⚠️ Failed to save VTrust snapshot: {}", e);
                    } else {
                        info!("💾 VTrust snapshot saved ({} validators)", snapshot.scores.len());
                    }
                }
                Err(e) => warn!("⚠️ Failed to serialize VTrust snapshot: {}", e),
            }
        }

        // Save mempool transactions to disk (if configured)
        if self.graceful_shutdown.config().save_mempool {
            let backup_dir = &self.graceful_shutdown.config().backup_dir;
            let mempool_path = format!("{}/mempool.bin", backup_dir);
            // Ensure backup directory exists
            if let Err(e) = std::fs::create_dir_all(backup_dir) {
                warn!("Failed to create backup directory {}: {}", backup_dir, e);
            } else {
                match self.mempool.save_to_file(&mempool_path) {
                    Ok(count) => info!("💾 Saved {} mempool transactions to {}", count, mempool_path),
                    Err(e) => warn!("⚠️ Failed to save mempool: {}", e),
                }
            }
        }

        let current_height = self.storage.get_best_height().ok().flatten().unwrap_or(0);
        let current_hash = self
            .storage
            .get_block_by_height(current_height)
            .ok()
            .flatten()
            .map(|b| b.hash())
            .unwrap_or([0u8; 32]);

        let checkpoint = crate::graceful_shutdown::ShutdownCheckpoint {
            block_height: current_height,
            block_hash: current_hash,
            shutdown_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            node_version: env!("CARGO_PKG_VERSION").to_string(),
            pending_tx_count: self.mempool.len(),
            peer_count: 0, // Would need P2P reference
        };

        if let Err(e) = self.graceful_shutdown.save_checkpoint(&checkpoint) {
            warn!("Failed to save shutdown checkpoint: {}", e);
        }

        self.graceful_shutdown.complete_shutdown();
        info!("✅ Shutdown complete");
        Ok(())
    }

    /// Print node status
    pub(crate) fn print_status(&self) {
        info!("");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("  📊 LuxTensor Node Status");
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("  Name:         {}", self.config.node.name);
        info!("  Chain ID:     {}", self.config.node.chain_id);
        info!("  Validator:    {}", self.config.node.is_validator);
        info!("");
        info!("  🌐 Network");
        info!(
            "    Address:    {}:{}",
            self.config.network.listen_addr, self.config.network.listen_port
        );
        info!("    Max Peers:  {}", self.config.network.max_peers);
        info!("");
        if self.config.rpc.enabled {
            info!("  🔌 RPC");
            info!("    Enabled:    Yes");
            info!(
                "    Address:    {}:{}",
                self.config.rpc.listen_addr, self.config.rpc.listen_port
            );
        } else {
            info!("  🔌 RPC:       Disabled");
        }
        info!("");
        info!("  💾 Storage");
        info!("    Path:       {:?}", self.config.storage.db_path);
        info!("    Cache:      {} MB", self.config.storage.cache_size);
        info!("");
        info!("  ⚖️  Consensus");
        info!("    Type:       Proof of Stake");
        info!("    Block Time: {} seconds", self.config.consensus.block_time);
        info!("    Epoch:      {} blocks", self.config.consensus.epoch_length);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("");
    }

    /// Get node statistics
    pub async fn get_stats(&self) -> Result<NodeStats> {
        let height = self.storage.get_best_height()?.unwrap_or(0);
        let validator_count = {
            let consensus = self.consensus.read();
            consensus.validator_count()
        };
        let mempool_size = self.mempool.len();

        Ok(NodeStats {
            height,
            validator_count,
            is_validator: self.config.node.is_validator,
            chain_id: self.config.node.chain_id,
            mempool_size,
        })
    }

}

/// Node statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeStats {
    pub height: u64,
    pub validator_count: usize,
    pub is_validator: bool,
    pub chain_id: u64,
    pub mempool_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_node_service_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.node.data_dir = temp_dir.path().to_path_buf();
        config.storage.db_path = temp_dir.path().join("db");
        config.rpc.enabled = false; // Disable RPC for test

        let service = NodeService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_node_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.node.data_dir = temp_dir.path().to_path_buf();
        config.storage.db_path = temp_dir.path().join("db");
        config.rpc.enabled = false;

        let service = NodeService::new(config).await.unwrap();
        let stats = service.get_stats().await.unwrap();

        assert_eq!(stats.height, 0); // Genesis block
        assert_eq!(stats.chain_id, 8898); // LuxTensor devnet chain_id
    }
}

// NOTE: is_dev_chain() and sign_transaction_with_dev_key() have been removed.
// They were only used by the old drain+convert bridge between rpc::Mempool
// and node::Mempool. With UnifiedMempool, RPC transactions already have
// proper signatures and no conversion is needed.

