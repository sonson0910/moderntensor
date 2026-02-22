use crate::config::Config;
use crate::executor::{calculate_receipts_root, TransactionExecutor};
use crate::mempool::Mempool;
use crate::task_dispatcher::{DispatchService, DispatcherConfig, TaskDispatcher};

/// Maximum allowed clock drift (in seconds) for future block timestamps.
const MAX_BLOCK_CLOCK_DRIFT_SECS: u64 = 30;

/// How often (in blocks) to run receipt pruning.
const PRUNING_INTERVAL: u64 = 1000;

/// Number of recent blocks whose receipts are kept during pruning.
const KEEP_RECEIPTS_BLOCKS: u64 = 10000;

/// Maximum gas limit per block
const BLOCK_GAS_LIMIT: u64 = 30_000_000;

/// Maximum number of transactions per block
const MAX_TRANSACTIONS_PER_BLOCK: usize = 1000;

/// Round-robin leader selection (fallback for bootstrap when PoS has no stake data)
fn is_leader_for_slot(validator_id: &str, slot: u64, validators: &[String]) -> bool {
    if validators.is_empty() {
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
    ConsensusConfig, DelegatorInfo, MinerInfo, NodeRegistry, ProofOfStake, RewardExecutor,
    SubnetInfo, TokenAllocation, UtilityMetrics, ValidatorInfo,
};
use luxtensor_core::{Block, StateDB, Transaction};
use luxtensor_storage::CachedStateDB;
use dashmap::DashMap;
use luxtensor_core::bridge::{BridgeConfig, InMemoryBridge};
use luxtensor_core::multisig::MultisigManager;
use luxtensor_crypto::{KeyPair, MerkleTree};
use luxtensor_network::eclipse_protection::{EclipseConfig, EclipseProtection};
use luxtensor_network::rate_limiter::{
    RateLimiter as NetworkRateLimiter, RateLimiterConfig as NetworkRateLimiterConfig,
};
use luxtensor_network::{
    get_seeds_for_chain, print_connection_info, NodeIdentity, SwarmCommand, SwarmP2PEvent,
    SwarmP2PNode,
};
use luxtensor_rpc::RpcServer;
use luxtensor_contracts::{AgentRegistry, AgentTriggerEngine, EvmExecutor};
use luxtensor_oracle::DisputeManager;
use luxtensor_storage::maintenance::{BackupConfig, DbMaintenance, PruningConfig};
use luxtensor_storage::metagraph_store::MetagraphDB;
use luxtensor_storage::BlockchainDB;
use luxtensor_storage::{CheckpointManager, CHECKPOINT_INTERVAL};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Get current Unix timestamp (seconds since epoch)
/// Panics only if system time is before Unix epoch (practically impossible)
#[inline]
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_secs()
}

/// Parse a hex address string (with or without 0x prefix) into [u8; 20]
fn parse_address_from_hex(addr_str: &str) -> Result<[u8; 20]> {
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
fn detect_external_ip() -> Option<String> {
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
fn peer_id_to_synthetic_ip(peer_id: &str) -> std::net::IpAddr {
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
#[allow(dead_code)]
pub struct NodeService {
    config: Config,
    storage: Arc<BlockchainDB>,
    state_db: Arc<RwLock<StateDB>>,
    consensus: Arc<RwLock<ProofOfStake>>,
    mempool: Arc<Mempool>,
    executor: Arc<TransactionExecutor>,
    reward_executor: Arc<RwLock<RewardExecutor>>,
    token_allocation: Arc<RwLock<TokenAllocation>>,
    node_registry: Arc<RwLock<NodeRegistry>>,
    shutdown_tx: broadcast::Sender<()>,
    tasks: Vec<JoinHandle<Result<()>>>,
    epoch_length: u64,
    /// Broadcast channel to P2P swarm for sending blocks/txs
    /// 🔧 FIX: Use bounded mpsc::Sender to match SwarmP2PNode::with_keypair() return type
    broadcast_tx: Option<mpsc::Sender<SwarmCommand>>,
    /// Genesis timestamp for slot calculation
    genesis_timestamp: u64,
    /// Validator keypair for block signing (None if not a validator)
    validator_keypair: Option<KeyPair>,
    /// Database maintenance (backup/restore/pruning)
    db_maintenance: Arc<DbMaintenance>,
    /// Eclipse attack protection
    eclipse_protection: Arc<EclipseProtection>,
    /// Long-range attack protection
    long_range_protection: Arc<LongRangeProtection>,
    /// Liveness monitor for detecting stalled validators
    liveness_monitor: Arc<RwLock<LivenessMonitor>>,
    /// Graceful shutdown handler
    graceful_shutdown: Arc<GracefulShutdown>,
    /// Health monitor for node health checks
    health_monitor: Arc<RwLock<HealthMonitor>>,
    /// Fast finality: BFT-style instant finality via validator signatures
    fast_finality: Arc<RwLock<FastFinality>>,
    /// Fork choice rule (longest chain / heaviest observed chain)
    fork_choice: Arc<RwLock<ForkChoice>>,
    /// AI Task Dispatcher for DePIN workload distribution
    task_dispatcher: Arc<TaskDispatcher>,
    /// Metagraph database for AI subnet/neuron metadata
    metagraph_db: Arc<MetagraphDB>,
    /// RANDAO randomness mixer for unbiased leader/task selection
    randao: Arc<RwLock<RandaoMixer>>,
    /// Slashing manager for punishing misbehaviour
    slashing_manager: Arc<RwLock<SlashingManager>>,
    /// Network-layer rate limiter for P2P flood protection
    network_rate_limiter: Arc<NetworkRateLimiter>,
    /// Atomic height guard to prevent block height race between P2P and block production
    best_height_guard: std::sync::Arc<std::sync::atomic::AtomicU64>,
    /// Agent registry for autonomous AI agents (Agentic EVM)
    agent_registry: Arc<AgentRegistry>,
    /// Agent trigger engine for block-level autonomous execution
    agent_trigger_engine: Arc<AgentTriggerEngine>,
    /// Dispute manager for optimistic AI fraud proofs
    dispute_manager: Arc<DisputeManager>,
    /// Cross-chain bridge for asset transfers
    bridge: Arc<InMemoryBridge>,
    /// Multisig wallet manager for multi-signature transactions
    multisig_manager: Arc<MultisigManager>,
    /// Merkle root caching layer wrapping state_db for efficient block production
    merkle_cache: Arc<CachedStateDB>,
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
        info!("  ✓ State database initialized");

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
            agent_registry,
            agent_trigger_engine,
            dispute_manager,
            bridge,
            multisig_manager,
            merkle_cache,
        })
    }

    /// Start all node services
    pub async fn start(&mut self) -> Result<()> {
        info!("🚀 Starting node services...");

        // Create shared Mempool for transaction bridge
        let rpc_mempool = Arc::new(parking_lot::RwLock::new(luxtensor_rpc::Mempool::new()));

        // ============================================================
        // Create shared pending_txs for unified TX storage (RPC + P2P)
        // ============================================================
        let shared_pending_txs: Arc<DashMap<luxtensor_core::Hash, Transaction>> =
            Arc::new(DashMap::new());

        // ============================================================
        // PHASE 1: Start P2P Swarm FIRST (to get command channel)
        // ============================================================
        info!("🌐 Starting P2P Swarm network...");
        let (p2p_event_tx, mut p2p_event_rx) = mpsc::channel::<SwarmP2PEvent>(4096);

        // NOTE: RPC→P2P transaction relay is now handled directly by SwarmBroadcaster
        // which sends transactions to the P2P swarm via the command channel.
        // The previously-unused mpsc channel has been removed.

        // Load or generate persistent node identity (Peer ID)
        let node_key_path = self
            .config
            .network
            .node_key_path
            .clone()
            .unwrap_or_else(|| self.config.node.data_dir.join("node.key"));
        let node_key_path_str = node_key_path.to_string_lossy().to_string();

        let node_identity = match NodeIdentity::load_or_generate(&node_key_path_str) {
            Ok(id) => {
                info!("🔑 Node Identity loaded");
                info!("   Peer ID: {}", id.peer_id_string());
                id
            }
            Err(e) => {
                warn!("⚠️ Failed to load node identity: {}. Using random ID.", e);
                NodeIdentity::generate_new()?
            }
        };

        // Print connection info for other nodes
        let peer_id_str = node_identity.peer_id_string();
        print_connection_info(
            &peer_id_str,
            self.config.network.listen_port,
            detect_external_ip().as_deref(),
        );

        // Create swarm with persistent identity
        let keypair = node_identity.into_keypair();

        // Get bootstrap nodes: config > hardcoded seeds > empty (use mDNS)
        let bootstrap_nodes = if !self.config.network.bootstrap_nodes.is_empty() {
            info!("📡 Using bootstrap nodes from config");
            self.config.network.bootstrap_nodes.clone()
        } else {
            let hardcoded = get_seeds_for_chain(self.config.node.chain_id);
            if !hardcoded.is_empty() {
                info!(
                    "📡 Using {} hardcoded seed node(s) for chain {}",
                    hardcoded.len(),
                    self.config.node.chain_id
                );
                hardcoded
            } else {
                info!("📡 No bootstrap nodes configured, using mDNS discovery");
                vec![]
            }
        };

        let enable_mdns = self.config.network.enable_mdns;

        match SwarmP2PNode::with_keypair(
            self.config.network.listen_port,
            p2p_event_tx,
            keypair,
            bootstrap_nodes.clone(),
            enable_mdns,
        )
        .await
        {
            Ok((mut swarm_node, command_tx)) => {
                info!("  ✓ P2P Swarm started");
                info!("    Listen port: {}", self.config.network.listen_port);
                if enable_mdns {
                    info!("    mDNS discovery: enabled");
                }
                if !bootstrap_nodes.is_empty() {
                    info!("    Bootstrap nodes: {}", bootstrap_nodes.len());
                }

                // Save broadcast_tx for block production
                self.broadcast_tx = Some(command_tx.clone());

                // 🔧 FIX: Run swarm in tokio::spawn (same runtime as RPC)
                // This ensures channels work correctly between tasks
                // 🔧 FIX: Track swarm JoinHandle in self.tasks so it is awaited on shutdown
                let swarm_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
                    swarm_node.run().await;
                    // 🔧 FIX #19: Log if swarm exits unexpectedly
                    tracing::error!(
                        "🚨 CRITICAL: P2P swarm event loop exited — node is now isolated!"
                    );
                    Ok(())
                });
                self.tasks.push(swarm_handle);

                // Start P2P event handler
                let storage_for_p2p = self.storage.clone();
                let broadcast_tx_for_sync = self.broadcast_tx.clone();
                let node_name = self.config.node.name.clone();
                let shared_pending_txs_for_p2p = shared_pending_txs.clone(); // Shared TX storage
                let eclipse_protection_for_p2p = self.eclipse_protection.clone(); // Eclipse attack protection
                let long_range_protection_for_p2p = self.long_range_protection.clone(); // Long-range attack protection
                let liveness_monitor_for_p2p = self.liveness_monitor.clone(); // Liveness monitoring
                let fast_finality_for_p2p = self.fast_finality.clone(); // Fast finality
                let fork_choice_for_p2p = self.fork_choice.clone(); // Fork choice
                let mempool_for_p2p = self.mempool.clone(); // Mempool for P2P txs
                let health_monitor_for_p2p = self.health_monitor.clone(); // Health monitoring
                let rate_limiter_for_p2p = self.network_rate_limiter.clone(); // Network rate limiter
                                                                              // 🔧 FIX #6: Clone state_db and executor for P2P block state execution
                let state_db_for_p2p = self.state_db.clone();
                let executor_for_p2p = self.executor.clone();
                // 🔧 FIX #9: Atomic height guard to prevent block height race between
                // P2P handler and block production (both reading/writing at the same height)
                let best_height_guard = self.best_height_guard.clone();
                let best_height_for_p2p = best_height_guard.clone();
                let _best_height_for_block_prod_p2p = best_height_guard.clone();
                let event_task = tokio::spawn(async move {
                    while let Some(event) = p2p_event_rx.recv().await {
                        match event {
                            SwarmP2PEvent::NewBlock(block) => {
                                let height = block.header.height;
                                let block_hash = block.hash();

                                // 🛡️ Rate-limit: only for blocks far ahead of our chain tip.
                                // Sync batches deliver many historical blocks at once (same proposer)
                                // which would be incorrectly rate-limited.
                                let current_best = best_height_for_p2p
                                    .load(std::sync::atomic::Ordering::Relaxed);
                                let proposer_id = hex::encode(&block.header.validator);
                                if height > current_best + 100 && !rate_limiter_for_p2p.check(&proposer_id) {
                                    warn!(
                                        "🛡️ Block #{} rate-limited from proposer {}",
                                        height, proposer_id
                                    );
                                    continue;
                                }

                                // 🛡️ Long-range attack protection: validate against checkpoints
                                if !long_range_protection_for_p2p
                                    .validate_against_checkpoints(block_hash, height)
                                {
                                    warn!("🛡️ Block #{} rejected: checkpoint mismatch (potential long-range attack)", height);
                                    continue;
                                }

                                // Check if we already have this block
                                if storage_for_p2p
                                    .get_block_by_height(height)
                                    .ok()
                                    .flatten()
                                    .is_some()
                                {
                                    debug!("Already have block #{}, skipping", height);
                                    continue;
                                }

                                // Check weak subjectivity
                                if !long_range_protection_for_p2p
                                    .is_within_weak_subjectivity(height)
                                {
                                    warn!(
                                        "🛡️ Block #{} rejected: outside weak subjectivity window",
                                        height
                                    );
                                    continue;
                                }

                                // ====================================================================
                                // 🔐 BLOCK VALIDATION — verify before storing (CRITICAL)
                                // ====================================================================

                                // 1. Validate sequential height
                                let my_height = storage_for_p2p
                                    .get_best_height()
                                    .unwrap_or(Some(0))
                                    .unwrap_or(0);
                                if height > my_height + 1 {
                                    // Gap — request sync instead of storing out-of-order block
                                    // 🔧 FIX: Cap sync range to MAX_SYNC_RANGE (1000) to avoid
                                    // oversized sync rejection. The periodic sync will continue
                                    // requesting subsequent chunks until caught up.
                                    const MAX_SYNC_RANGE: u64 = 1000;
                                    let sync_to = (my_height + MAX_SYNC_RANGE).min(height);
                                    debug!(
                                        "Block #{} is ahead of our height {}, requesting sync {}-{}",
                                        height, my_height, my_height + 1, sync_to
                                    );
                                    if let Some(ref tx) = broadcast_tx_for_sync {
                                        if let Err(e) = tx
                                            .send(SwarmCommand::RequestSync {
                                                from_height: my_height + 1,
                                                to_height: sync_to,
                                                my_id: node_name.clone(),
                                            })
                                            .await
                                        {
                                            warn!("Failed to send sync request: {}", e);
                                        }
                                    }
                                    continue;
                                }
                                if height <= my_height {
                                    debug!(
                                        "Block #{} is not newer than our height {}",
                                        height, my_height
                                    );
                                    continue;
                                }

                                // 2. Validate previous_hash chain link
                                if let Ok(Some(prev_block)) =
                                    storage_for_p2p.get_block_by_height(my_height)
                                {
                                    if block.header.previous_hash != prev_block.hash() {
                                        warn!("🚫 Block #{} rejected: previous_hash mismatch (expected {:?}, got {:?})",
                                            height, &prev_block.hash()[..4], &block.header.previous_hash[..4]);
                                        continue;
                                    }

                                    // 2b. SECURITY: Validate timestamp monotonicity against parent
                                    if block.header.timestamp < prev_block.header.timestamp {
                                        warn!("🚫 Block #{} rejected: timestamp regression ({} < parent {})",
                                            height, block.header.timestamp, prev_block.header.timestamp);
                                        continue;
                                    }
                                }

                                // 3. Validate block signature (if validator field is set)
                                if block.header.validator != [0u8; 32]
                                    && !block.header.signature.is_empty()
                                {
                                    // Extract the 20-byte address from the 32-byte validator field
                                    let validator_addr = &block.header.validator[12..32];
                                    // Reconstruct unsigned header for hash verification
                                    let mut unsigned_header = block.header.clone();
                                    let sig_backup = unsigned_header.signature.clone();
                                    unsigned_header.signature = vec![];
                                    let header_hash = unsigned_header.hash();

                                    // Verify signature using ECDSA recovery
                                    if sig_backup.len() >= 64 {
                                        match luxtensor_crypto::recover_address(
                                            &header_hash,
                                            &sig_backup,
                                        ) {
                                            Ok(recovered_addr) => {
                                                if recovered_addr.as_bytes() != validator_addr {
                                                    warn!("🚫 Block #{} rejected: invalid validator signature (recovered {:?} != expected {:?})",
                                                        height, &recovered_addr.as_bytes()[..4], &validator_addr[..4]);
                                                    continue;
                                                }
                                            }
                                            Err(_) => {
                                                warn!("🚫 Block #{} rejected: signature recovery failed", height);
                                                continue;
                                            }
                                        }
                                    }
                                }

                                // 4. Validate txs_root (Merkle root of transactions)
                                let tx_hashes: Vec<[u8; 32]> =
                                    block.transactions.iter().map(|tx| tx.hash()).collect();
                                let expected_txs_root = if tx_hashes.is_empty() {
                                    [0u8; 32]
                                } else {
                                    luxtensor_crypto::MerkleTree::new(tx_hashes).root()
                                };
                                if block.header.txs_root != expected_txs_root {
                                    warn!("🚫 Block #{} rejected: txs_root mismatch", height);
                                    continue;
                                }

                                // 5. Validate reasonable timestamp (not too far in future)
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                if block.header.timestamp > now + MAX_BLOCK_CLOCK_DRIFT_SECS {
                                    warn!("🚫 Block #{} rejected: timestamp {} is too far in the future (now={})",
                                        height, block.header.timestamp, now);
                                    continue;
                                }

                                // 6. Full structural validation via Block::validate()
                                if let Err(e) = block.validate() {
                                    warn!(
                                        "🚫 Block #{} rejected: validate() failed: {}",
                                        height, e
                                    );
                                    continue;
                                }

                                // 🔧 FIX #9 (revised): Sequential height check instead of CAS
                                // CAS was silently rejecting blocks that arrived out-of-order
                                // during sync batches. Simple check is safe because P2P ingestion
                                // runs in a single task — no concurrent writes from this path.
                                let my_height = best_height_for_p2p
                                    .load(std::sync::atomic::Ordering::SeqCst);
                                if height != my_height + 1 {
                                    debug!("Block #{} out of order (current={}), skipping", height, my_height);
                                    continue;
                                }

                                if let Err(e) = storage_for_p2p.store_block(&block) {
                                    warn!("Failed to store received block: {}", e);
                                } else {
                                    best_height_for_p2p.store(
                                        height,
                                        std::sync::atomic::Ordering::SeqCst,
                                    );
                                    info!("📥 Synced block #{} from peer", height);

                                    // 🔧 FIX #21: Remove confirmed txs from pending pool to prevent ghost entries
                                    {
                                        let tx_hashes: Vec<[u8; 32]> =
                                            block.transactions.iter().map(|tx| tx.hash()).collect();
                                        for hash in &tx_hashes {
                                            shared_pending_txs_for_p2p.remove(hash);
                                        }
                                        mempool_for_p2p.remove_transactions(&tx_hashes);
                                    }

                                    // 🔧 FIX #6: Execute transactions against StateDB for P2P-received blocks
                                    // Previously only the block producer executed txs, causing state divergence
                                    // on non-validator nodes (incorrect RPC balances/nonces)
                                    //
                                    // SECURITY: Split lock scope — write lock only during TX execution,
                                    // read lock for disk flush to minimize RPC query starvation.
                                    {
                                        let mut state = state_db_for_p2p.write();
                                        for (tx_index, tx) in block.transactions.iter().enumerate()
                                        {
                                            if let Err(e) = executor_for_p2p.execute(
                                                tx,
                                                &mut state,
                                                height,
                                                block_hash,
                                                tx_index,
                                                block.header.timestamp,
                                            ) {
                                                debug!("P2P block #{} tx {} execution failed: {} (may be expected for already-applied state)", height, tx_index, e);
                                            }
                                        }
                                        // Drop write lock before disk I/O
                                    }
                                    // Persist state with read lock only (flush_to_db takes &self)
                                    {
                                        let state = state_db_for_p2p.read();
                                        if let Err(e) = state.flush_to_db(storage_for_p2p.as_ref())
                                        {
                                            warn!("Failed to persist P2P block state: {}", e);
                                        }
                                    }

                                    // 🔗 Feed block to ForkChoice (GHOST) for canonical chain tracking
                                    if let Err(e) =
                                        fork_choice_for_p2p.read().add_block(block.clone())
                                    {
                                        debug!("ForkChoice: {}", e);
                                    }

                                    // 🔐 FastFinality: record the block producer's attestation
                                    if block.header.validator != [0u8; 32] {
                                        let mut validator_addr = [0u8; 20];
                                        validator_addr
                                            .copy_from_slice(&block.header.validator[12..32]);
                                        let addr = luxtensor_core::Address::from(validator_addr);
                                        let finalized = fast_finality_for_p2p
                                            .write()
                                            .add_signature(block_hash, height, addr);
                                        match finalized {
                                            Ok(true) => {
                                                info!("⚡ Block #{} reached fast finality!", height)
                                            }
                                            Ok(false) => debug!(
                                                "Block #{} collecting finality signatures",
                                                height
                                            ),
                                            Err(e) => debug!("FastFinality skipped: {}", e),
                                        }
                                    }

                                    // 🎯 Record block for liveness monitoring
                                    liveness_monitor_for_p2p.write().record_block(height);

                                    // 🏥 Update health monitor with block height
                                    health_monitor_for_p2p.write().update_block_height(height);

                                    // Update finalized state for blocks past confirmation threshold
                                    let finality_depth = 32; // Same as min_finality_confirmations
                                    if height > finality_depth {
                                        let finalized_height = height - finality_depth;
                                        if let Ok(Some(finalized_block)) =
                                            storage_for_p2p.get_block_by_height(finalized_height)
                                        {
                                            long_range_protection_for_p2p.update_finalized(
                                                finalized_block.hash(),
                                                finalized_height,
                                                finalized_block.header.state_root,
                                            );
                                        }
                                    }
                                }
                            }
                            SwarmP2PEvent::NewTransaction(tx) => {
                                // �️ Rate-limit: check per-sender message rate
                                let sender_id = hex::encode(&tx.from);
                                if !rate_limiter_for_p2p.check(&sender_id) {
                                    warn!("🛡️ Transaction rate-limited from sender {}", sender_id);
                                    continue;
                                }

                                // �🚀 Add received TX to shared pending_txs for RPC query
                                // SECURITY FIX: Validate via mempool FIRST, then insert into
                                // shared_pending_txs only if accepted. This prevents invalid
                                // transactions from polluting the shared pool.
                                let tx_hash = tx.hash();
                                {
                                    if shared_pending_txs_for_p2p.contains_key(&tx_hash) {
                                        // Already in pool, skip duplicate
                                        continue;
                                    }
                                }

                                // Validate through mempool first
                                match mempool_for_p2p.add_transaction(tx.clone()) {
                                    Ok(_) => {
                                        // Mempool accepted — now add to shared pending pool
                                        shared_pending_txs_for_p2p.insert(tx_hash, tx);
                                        info!("📥 Added validated transaction from peer to shared pool");
                                    }
                                    Err(e) => {
                                        debug!("Mempool rejected P2P tx: {}", e);
                                    }
                                }
                            }
                            SwarmP2PEvent::PeerConnected(peer_id) => {
                                // 🛡️ Register peer with Eclipse Protection
                                let peer_id_str = peer_id.to_string();
                                let synthetic_ip = peer_id_to_synthetic_ip(&peer_id_str);
                                let is_outbound = false; // Default to inbound

                                if eclipse_protection_for_p2p
                                    .should_allow_connection(&synthetic_ip, is_outbound)
                                {
                                    eclipse_protection_for_p2p.add_peer(
                                        peer_id_str.clone(),
                                        synthetic_ip,
                                        is_outbound,
                                    );
                                    info!(
                                        "👋 Peer connected: {} (diversity: {}%)",
                                        peer_id,
                                        eclipse_protection_for_p2p.calculate_diversity_score()
                                    );
                                } else {
                                    warn!("🛡️ Peer blocked by eclipse protection: {}", peer_id);
                                }

                                // Update global peer count for RPC
                                luxtensor_rpc::peer_count::increment_peer_count();

                                // 🎯 Update liveness monitor with current peer count
                                let current_peer_count =
                                    luxtensor_rpc::peer_count::get_peer_count();
                                liveness_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);

                                // 🏥 Update health monitor with peer count
                                health_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);

                                // Request sync when peer connects
                                let my_height = storage_for_p2p
                                    .get_best_height()
                                    .unwrap_or(Some(0))
                                    .unwrap_or(0);
                                if let Some(ref tx) = broadcast_tx_for_sync {
                                    // Request blocks we don't have (up to 100 ahead)
                                    if let Err(e) = tx
                                        .send(SwarmCommand::RequestSync {
                                            from_height: my_height + 1,
                                            to_height: my_height + 100,
                                            my_id: node_name.clone(),
                                        })
                                        .await
                                    {
                                        warn!("Failed to send sync request on peer connect: {}", e);
                                    }
                                    info!("🔄 Requesting sync from height {}", my_height + 1);
                                }
                            }
                            SwarmP2PEvent::PeerDisconnected(peer_id) => {
                                // 🛡️ Remove peer from Eclipse Protection tracking
                                eclipse_protection_for_p2p.remove_peer(&peer_id.to_string());
                                info!("👋 Peer disconnected: {}", peer_id);
                                // Update global peer count for RPC
                                luxtensor_rpc::peer_count::decrement_peer_count();

                                // 🎯 Update liveness monitor with current peer count
                                let current_peer_count =
                                    luxtensor_rpc::peer_count::get_peer_count();
                                liveness_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);

                                // 🏥 Update health monitor with peer count
                                health_monitor_for_p2p
                                    .write()
                                    .update_peer_count(current_peer_count);
                            }
                            SwarmP2PEvent::SyncRequest { from_height, to_height, requester_id } => {
                                // Cap sync response to prevent memory exhaustion
                                let max_blocks_per_response = 50u64;
                                let capped_to =
                                    to_height.min(from_height + max_blocks_per_response - 1);
                                debug!(
                                    "🔄 Got sync request from {} for blocks {}-{} (capped at {})",
                                    requester_id, from_height, to_height, capped_to
                                );
                                // Collect blocks we have in range
                                let mut blocks_to_send = Vec::new();
                                for h in from_height..=capped_to {
                                    if let Ok(Some(block)) = storage_for_p2p.get_block_by_height(h)
                                    {
                                        blocks_to_send.push(block);
                                    }
                                }
                                if !blocks_to_send.is_empty() {
                                    let first_h = blocks_to_send.first().map(|b| b.header.height).unwrap_or(0);
                                    let last_h = blocks_to_send.last().map(|b| b.header.height).unwrap_or(0);
                                    debug!(
                                        "📤 Sending {} blocks (#{}-#{}) to {}",
                                        blocks_to_send.len(),
                                        first_h,
                                        last_h,
                                        requester_id
                                    );
                                    if let Some(ref tx) = broadcast_tx_for_sync {
                                        if let Err(e) = tx
                                            .send(SwarmCommand::SendBlocks {
                                                blocks: blocks_to_send,
                                            })
                                            .await
                                        {
                                            warn!("Failed to send blocks in sync response: {}", e);
                                        }
                                    }
                                } else {
                                    debug!("📭 No blocks found for range {}-{}", from_height, capped_to);
                                }
                            }
                        }
                    }
                    // 🔧 FIX #22: Log when P2P event handler exits (channel closed = swarm dropped or shutdown)
                    tracing::info!("📡 P2P event handler loop exited (channel closed)");
                    Ok::<(), anyhow::Error>(())
                });
                self.tasks.push(event_task);

                // ============================================================
                // PERIODIC SYNC TASK: Retry sync every 10 seconds
                // This ensures late-joining nodes can sync even if initial
                // sync request fails due to InsufficientPeers
                // ============================================================
                let sync_command_tx = command_tx.clone();
                let sync_storage = self.storage.clone();
                let sync_node_name = self.config.node.name.clone();
                let sync_task = tokio::spawn(async move {
                    let mut last_sync_height = 0u64;
                    let mut sync_interval_secs = 10u64;
                    let mut consecutive_no_progress = 0u32;
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(sync_interval_secs)).await;

                        // Check current height from storage
                        let my_height =
                            sync_storage.get_best_height().unwrap_or(Some(0)).unwrap_or(0);

                        if my_height > last_sync_height {
                            // Made progress since last check → stay aggressive
                            consecutive_no_progress = 0;
                            sync_interval_secs = 10;
                        } else {
                            // No progress → backoff: 10 → 20 → 40 → 60 (cap)
                            consecutive_no_progress += 1;
                            sync_interval_secs = (10u64 * 2u64.saturating_pow(consecutive_no_progress.min(3))).min(60);
                        }

                        // Only request sync if we've made no progress since last check
                        if my_height == last_sync_height {
                            let batch_size = 50u64;
                            if let Err(e) = sync_command_tx
                                .send(SwarmCommand::RequestSync {
                                    from_height: my_height + 1,
                                    to_height: my_height + batch_size,
                                    my_id: sync_node_name.clone(),
                                })
                                .await
                            {
                                warn!("Failed to send periodic sync request: {}", e);
                            }

                            if my_height == 0 {
                                info!("🔄 Initial sync: requesting blocks 1-{}...", batch_size);
                            } else {
                                debug!(
                                    "🔄 Periodic sync check: height={}, next check in {}s",
                                    my_height, sync_interval_secs
                                );
                            }
                        }
                        last_sync_height = my_height;
                    }
                });
                self.tasks.push(sync_task);
            }
            Err(e) => {
                warn!("Failed to start P2P Swarm: {}. Running in standalone mode.", e);
            }
        }

        // ============================================================
        // PHASE 2: Start RPC server WITH DIRECT Swarm broadcaster
        // ============================================================
        // Shared unified_state for syncing between RPC and block production.
        // Populated when RPC is enabled; None when RPC is disabled.
        let mut unified_state_for_blocks: Option<
            Arc<parking_lot::RwLock<luxtensor_core::UnifiedStateDB>>,
        > = None;

        if self.config.rpc.enabled {
            info!("🔌 Starting RPC server with direct Swarm broadcaster...");

            // Use command_tx directly from P2P swarm (bypassing tx_relay task)
            let broadcaster: Arc<dyn luxtensor_rpc::TransactionBroadcaster> =
                match &self.broadcast_tx {
                    Some(cmd_tx) => {
                        Arc::new(crate::swarm_broadcaster::SwarmBroadcaster::new(cmd_tx.clone()))
                    }
                    None => {
                        warn!("No P2P swarm available, using NoOp broadcaster");
                        Arc::new(luxtensor_rpc::NoOpBroadcaster)
                    }
                };

            // Use shared pending_txs for unified TX storage between RPC and P2P
            // 🔧 FIX: Pass config chain_id instead of hardcoded 1337
            let mut rpc_server = RpcServer::new_with_shared_pending_txs(
                self.storage.clone(),
                rpc_mempool.clone(),
                broadcaster,
                shared_pending_txs.clone(),
                self.config.node.chain_id as u64,
            );

            // Wire optional subsystems into the RPC server
            rpc_server.set_bridge(self.bridge.clone());
            rpc_server.set_multisig_manager(self.multisig_manager.clone());
            rpc_server.set_merkle_cache(self.merkle_cache.clone());

            // Wire shared EVM executor for eth_call storage reads.
            // Clone shares the underlying Arc<RwLock<...>> state, so eth_call
            // reads the same storage that block execution has committed to.
            rpc_server.set_evm_executor(self.executor.evm().clone());

            // Extract unified_state Arc BEFORE moving rpc_server into the spawn closure.
            // This allows block production to sync state into the RPC layer after each block.
            unified_state_for_blocks = Some(rpc_server.unified_state());

            let addr = format!("{}:{}", self.config.rpc.listen_addr, self.config.rpc.listen_port);
            let rpc_threads = self.config.rpc.threads;
            let rpc_cors_origins = self.config.rpc.cors_origins.clone();

            // 🔧 FIX: Use shutdown_rx instead of a second ctrl_c handler.
            // Previously both this task and wait_for_shutdown() raced on ctrl_c,
            // requiring 2× Ctrl+C to stop the node.
            let mut shutdown_rx = self.shutdown_tx.subscribe();

            let task = tokio::spawn(async move {
                info!("  ✓ RPC server listening on {}", addr);
                match rpc_server.start(&addr, rpc_threads, &rpc_cors_origins) {
                    Ok(_server) => {
                        info!("RPC server started successfully");
                        // Keep server alive until shutdown signal is received
                        let _ = shutdown_rx.recv().await;
                        info!("RPC server shutting down");
                        Ok(())
                    }
                    Err(e) => Err(e.into()),
                }
            });

            self.tasks.push(task);
        }

        // ============================================================
        // PHASE 2b: Start WebSocket server for real-time subscriptions
        // ============================================================
        if self.config.rpc.enabled && self.config.rpc.ws_enabled {
            info!("🔌 Starting WebSocket RPC server...");
            let ws_addr = format!("{}:{}", self.config.rpc.listen_addr, self.config.rpc.ws_port);
            let ws_server = luxtensor_rpc::WebSocketServer::new();

            // Store broadcast sender for block production to emit events
            let ws_broadcast_tx = ws_server.get_broadcast_sender();
            // Save the broadcast sender so block production can emit events later
            // (The WS broadcast sender is available via the returned task handle)
            let _ws_sender = ws_broadcast_tx.clone();

            let task = tokio::spawn(async move {
                info!("  ✓ WebSocket RPC listening on ws://{}", ws_addr);
                if let Err(e) = ws_server.start(&ws_addr).await {
                    error!("WebSocket server error: {:?}", e);
                }
                Ok::<(), anyhow::Error>(())
            });

            self.tasks.push(task);
        }

        // Start block production if validator
        let best_height_for_block_prod = self.best_height_guard.clone();
        if self.config.node.is_validator {
            info!("🔨 Starting block production...");
            let consensus = self.consensus.clone();
            let storage = self.storage.clone();
            let state_db = self.state_db.clone();
            let merkle_cache = self.merkle_cache.clone();
            let mempool = self.mempool.clone();
            let executor = self.executor.clone();
            let reward_executor = self.reward_executor.clone();
            let block_time = self.config.consensus.block_time;
            let epoch_length = self.epoch_length;
            let shutdown_rx = self.shutdown_tx.subscribe();
            let rpc_mempool_for_block = rpc_mempool.clone();

            // Leader election params
            let validator_id = self
                .config
                .node
                .validator_id
                .clone()
                .unwrap_or_else(|| self.config.node.name.clone());
            let validators = self.config.consensus.validators.clone();
            let genesis_timestamp = self.genesis_timestamp;
            let broadcast_tx = self.broadcast_tx.clone();
            let chain_id = self.config.node.chain_id as u64;
            // Get our validator address for PoS leader election
            let our_validator_address = self.validator_keypair.as_ref().map(|kp| kp.address());
            // 🔧 FIX: Clone keypair for the block production closure
            let validator_keypair_for_block = self.validator_keypair.clone();
            let metagraph_db_clone = self.metagraph_db.clone();
            let unified_state_clone = unified_state_for_blocks.clone();
            let randao_clone = self.randao.clone();
            let agent_trigger_clone = self.agent_trigger_engine.clone();
            let dispute_manager_clone = self.dispute_manager.clone();
            let slashing_manager_clone = self.slashing_manager.clone();
            let task = tokio::spawn(async move {
                Self::block_production_loop(
                    consensus,
                    storage,
                    state_db,
                    mempool,
                    executor,
                    reward_executor,
                    block_time,
                    epoch_length,
                    shutdown_rx,
                    rpc_mempool_for_block,
                    validator_id,
                    validators,
                    genesis_timestamp,
                    broadcast_tx,
                    chain_id,
                    our_validator_address,
                    validator_keypair_for_block,
                    best_height_for_block_prod, // 🔧 FIX #9: Atomic height guard
                    metagraph_db_clone,
                    unified_state_clone, // For syncing RPC state after each block
                    randao_clone,        // RANDAO mixer for epoch finalization
                    agent_trigger_clone, // Agentic EVM triggers
                    dispute_manager_clone, // Optimistic AI dispute processing
                    slashing_manager_clone, // For dispute slashing
                    merkle_cache,        // Merkle root caching layer
                )
                .await
            });

            self.tasks.push(task);
            info!("  ✓ Block production started");
            if let Some(ref vid) = self.config.node.validator_id {
                info!("    Validator ID: {}", vid);
            }
            info!("    Known validators: {:?}", self.config.consensus.validators);
        }

        // Start AI Task Dispatcher service (DePIN workload distribution)
        {
            let dispatch_service = if let Some(ref cmd_tx) = self.broadcast_tx {
                DispatchService::with_p2p(self.task_dispatcher.clone(), cmd_tx.clone())
            } else {
                DispatchService::new(self.task_dispatcher.clone())
            };
            let dispatch_handle = tokio::spawn(async move {
                dispatch_service.start().await;
                Ok::<(), anyhow::Error>(())
            });
            self.tasks.push(dispatch_handle);
            info!("  ✓ AI Task Dispatcher service started");
        }

        info!("✅ All services started successfully");
        self.print_status();

        Ok(())
    }

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

    /// Block production loop for validators
    async fn block_production_loop(
        consensus: Arc<RwLock<ProofOfStake>>,
        storage: Arc<BlockchainDB>,
        state_db: Arc<RwLock<StateDB>>,
        mempool: Arc<Mempool>,
        executor: Arc<TransactionExecutor>,
        reward_executor: Arc<RwLock<RewardExecutor>>,
        block_time: u64,
        epoch_length: u64,
        mut shutdown: broadcast::Receiver<()>,
        rpc_mempool: Arc<parking_lot::RwLock<luxtensor_rpc::Mempool>>,
        validator_id: String,
        validators: Vec<String>,
        genesis_timestamp: u64,
        broadcast_tx: Option<mpsc::Sender<SwarmCommand>>,
        chain_id: u64,
        our_validator_address: Option<luxtensor_crypto::CryptoAddress>,
        // 🔧 FIX: Accept validator keypair for block signing
        validator_keypair_for_block: Option<KeyPair>,
        // 🔧 FIX #9: Atomic height guard shared with P2P handler
        best_height_guard: std::sync::Arc<std::sync::atomic::AtomicU64>,
        metagraph_db: Arc<MetagraphDB>,
        // Unified RPC state — synced after each block so eth_* RPCs return fresh data
        unified_state: Option<Arc<parking_lot::RwLock<luxtensor_core::UnifiedStateDB>>>,
        // RANDAO mixer for epoch finalization
        randao: Arc<RwLock<RandaoMixer>>,
        // Agentic EVM: block-level autonomous agent triggers
        agent_trigger_engine: Arc<AgentTriggerEngine>,
        // Optimistic AI: dispute manager for fraud proofs
        dispute_manager: Arc<DisputeManager>,
        // Slashing manager for dispute-triggered slashing
        slashing_manager: Arc<RwLock<SlashingManager>>,
        // Merkle root caching layer for efficient state root computation
        merkle_cache: Arc<CachedStateDB>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(block_time));
        let mut slot_counter: u64 = 0;
        // 🔧 FIX: Store keypair reference for repeated use across slots
        let validator_keypair_ref = validator_keypair_for_block;
        // 🔧 FIX MC-6: Accumulate TX count across the entire epoch instead of
        // using only the last block's count at the epoch boundary.
        let mut epoch_tx_accumulator: u64 = 0;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Calculate current slot
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::ZERO)
                        .as_secs();
                    let slot = if now > genesis_timestamp && block_time > 0 {
                        (now - genesis_timestamp) / block_time
                    } else {
                        slot_counter
                    };
                    slot_counter = slot + 1;

                    // 🔧 DEBUG: Log every slot to confirm block production is running
                    debug!("⏰ Slot {} processing (chain_id: {})", slot, chain_id);

                    // 🔧 FIX: Drain tx_queue EVERY slot to ensure TXs are not missed
                    // Add to mempool regardless of leader status, produce block only if leader
                    let pending_txs = rpc_mempool.read().drain_tx_queue();
                    if !pending_txs.is_empty() {
                        debug!("📤 Drained {} transactions from tx_queue", pending_txs.len());
                    }
                    for ready_tx in pending_txs {
                        let from_addr = luxtensor_core::Address::from(ready_tx.from);
                        let to_addr = ready_tx.to.map(luxtensor_core::Address::from);

                        let mut tx = Transaction::with_chain_id(
                            chain_id,
                            ready_tx.nonce,
                            from_addr,
                            to_addr,
                            ready_tx.value,
                            ready_tx.gas_price,
                            ready_tx.gas,
                            ready_tx.data.clone(),
                        );

                        // Use the original signature from eth_sendRawTransaction.
                        // Only fall back to dev-key signing when the signature is
                        // empty (e.g., internal/faucet transactions in dev mode).
                        let has_real_sig = ready_tx.r != [0u8; 32] || ready_tx.s != [0u8; 32];
                        if has_real_sig {
                            tx.r = ready_tx.r;
                            tx.s = ready_tx.s;
                            tx.v = ready_tx.v;
                            debug!("Using original RLP signature for tx from 0x{}", hex::encode(&ready_tx.from));
                        } else if is_dev_chain(chain_id) {
                            // Dev chains may use unsigned internal transactions
                            if !sign_transaction_with_dev_key(&mut tx, &ready_tx.from) {
                                warn!("Failed to sign transaction with dev key (from=0x{})", hex::encode(&ready_tx.from));
                                continue;
                            }
                        } else {
                            // Production: reject unsigned transactions
                            warn!("Rejecting unsigned transaction from 0x{} on non-dev chain {}",
                                  hex::encode(&ready_tx.from), chain_id);
                            continue;
                        }

                        // With EIP-155 aligned signing_message(), all transactions
                        // (external signed + dev-key signed) can use the standard path.
                        if let Err(e) = mempool.add_transaction(tx) {
                            warn!("Failed to add TX to mempool: {}", e);
                        } else {
                            debug!("✅ Transaction added to node mempool successfully");
                        }
                    }

                    // Check if we are the leader for this slot using PoS VRF selection
                    // Fallback to round-robin if validator set is empty (bootstrapping)
                    let is_our_turn = if let Some(our_addr) = our_validator_address {
                        let our_addr_typed = luxtensor_core::Address::from(our_addr);
                        match consensus.read().select_validator(slot) {
                            Ok(selected) => {
                                if selected != our_addr_typed {
                                    debug!("⏳ Slot {}: Not selected by PoS (leader: 0x{})",
                                           slot, hex::encode(selected.as_bytes()));
                                    false
                                } else {
                                    true
                                }
                            }
                            Err(_) => {
                                // Validator set empty — fall back to round-robin for bootstrap
                                if !validators.is_empty() {
                                    is_leader_for_slot(&validator_id, slot, &validators)
                                } else {
                                    true // No validators configured, produce blocks
                                }
                            }
                        }
                    } else {
                        // No keypair — use legacy round-robin
                        if !validators.is_empty() {
                            is_leader_for_slot(&validator_id, slot, &validators)
                        } else {
                            true
                        }
                    };

                    if !is_our_turn {
                        continue;
                    }

                    info!("🎯 Slot {}: We are the leader! Producing block...", slot);

                    // Produce a block (TXs already in mempool from earlier drain)
                    match Self::produce_block(
                        &consensus, &storage, &state_db, &mempool, &executor,
                        &reward_executor, epoch_length,
                        // 🔧 FIX: Pass validator keypair for block signing
                        // Previously hardcoded to None — blocks were always unsigned
                        validator_keypair_ref.as_ref(),
                        &best_height_guard,  // 🔧 FIX #9: Atomic height guard
                        &metagraph_db,   // For reward distribution from metagraph
                        &randao,         // RANDAO mixer for epoch finalization
                        epoch_tx_accumulator, // 🔧 FIX MC-6: pass accumulated count
                        &agent_trigger_engine, // Agentic EVM triggers
                        &dispute_manager, // Optimistic AI disputes
                        &slashing_manager, // For dispute slashing
                        &merkle_cache,   // Merkle root caching
                    ).await {
                        Ok(block) => {
                            // 🔧 FIX MC-6: Accumulate TX count for the whole epoch
                            epoch_tx_accumulator += block.transactions.len() as u64;

                            // 🔧 FIX C3: Reset accumulator at epoch boundaries so it
                            // doesn't inflate utility scores across epochs.
                            if epoch_length > 0 && block.header.height % epoch_length == 0 {
                                epoch_tx_accumulator = 0;
                            }

                            // Sync UnifiedStateDB so the RPC layer returns fresh state
                            if let Some(ref us) = unified_state {
                                let state_read = state_db.read();
                                let mut unified = us.write();
                                unified.sync_from_state_db(&state_read, block.header.height);
                                debug!("📊 UnifiedStateDB synced to height {}", block.header.height);
                            }

                            // Broadcast block to P2P network
                            if let Some(ref tx) = broadcast_tx {
                                if let Err(e) = tx.send(SwarmCommand::BroadcastBlock(block.clone())).await {
                                    warn!("Failed to send block to broadcast channel: {}", e);
                                } else {
                                    info!("📡 Block #{} broadcasted to network", block.header.height);
                                }
                            } else {
                                info!("📦 Block #{} produced (standalone mode)", block.header.height);
                            }

                            // Auto-checkpoint: create snapshot at checkpoint intervals
                            let current_height = block.header.height;
                            if current_height > 0 && current_height % CHECKPOINT_INTERVAL == 0 {
                                let checkpoint_dir = std::path::PathBuf::from("./data/checkpoints");
                                let mut manager = CheckpointManager::new(&checkpoint_dir, storage.inner_db());

                                if let Err(e) = manager.create_checkpoint(current_height, block.header.hash(), block.header.state_root) {
                                    warn!("⚠️ Failed to create checkpoint at height {}: {:?}", current_height, e);
                                } else {
                                    info!("📸 Checkpoint created at height {} (every {} blocks)", current_height, CHECKPOINT_INTERVAL);
                                }
                            }

                            // Auto-pruning: clean up old receipts periodically
                            if current_height > KEEP_RECEIPTS_BLOCKS && current_height % PRUNING_INTERVAL == 0 {
                                let prune_before = current_height.saturating_sub(KEEP_RECEIPTS_BLOCKS);
                                match storage.prune_receipts_before_height(prune_before) {
                                    Ok(pruned) if pruned > 0 => {
                                        info!("🗑️ Auto-pruned {} old receipts (keeping last {} blocks)", pruned, KEEP_RECEIPTS_BLOCKS);
                                    }
                                    Ok(_) => {} // Nothing to prune
                                    Err(e) => {
                                        warn!("⚠️ Failed to auto-prune receipts: {:?}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to produce block: {}", e);
                        }
                    }
                }
                _ = shutdown.recv() => {
                    info!("Block production shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Produce a single block
    async fn produce_block(
        consensus: &Arc<RwLock<ProofOfStake>>,
        storage: &Arc<BlockchainDB>,
        state_db: &Arc<RwLock<StateDB>>,
        mempool: &Arc<Mempool>,
        executor: &Arc<TransactionExecutor>,
        reward_executor: &Arc<RwLock<RewardExecutor>>,
        epoch_length: u64,
        validator_keypair: Option<&KeyPair>,
        // 🔧 FIX #9: Atomic height guard shared with P2P handler
        best_height_guard: &std::sync::Arc<std::sync::atomic::AtomicU64>,
        metagraph_db: &Arc<MetagraphDB>,
        // RANDAO mixer — finalized at each epoch boundary to feed PoS seed
        randao: &Arc<RwLock<RandaoMixer>>,
        // 🔧 FIX MC-6: Accumulated TX count from prior blocks in this epoch
        epoch_tx_count: u64,
        // 🤖 Agentic EVM: autonomous agent trigger engine
        agent_trigger_engine: &Arc<AgentTriggerEngine>,
        // ⚖️ Optimistic AI: dispute manager for fraud proofs
        dispute_manager: &Arc<DisputeManager>,
        // Slashing manager for dispute-triggered slashing
        slashing_manager: &Arc<RwLock<SlashingManager>>,
        // 📦 Merkle root caching layer — caches state roots by block height
        merkle_cache: &Arc<CachedStateDB>,
    ) -> Result<Block> {
        // Get current height
        let height = storage.get_best_height()?.unwrap_or(0);
        let new_height = height + 1;

        // 🔧 FIX #9: Atomic CAS to prevent block height race
        if best_height_guard
            .compare_exchange(
                height,
                new_height,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            return Err(anyhow::anyhow!(
                "Block height race: expected height {} but another block was committed first",
                height
            ));
        }

        // Get previous block
        let previous_block = storage
            .get_block_by_height(height)
            .context(format!("Failed to read block at height {} from storage", height))?
            .ok_or_else(|| anyhow::anyhow!("Previous block not found at height {}", height))?;

        // 🔧 FIX MC-2: Capture timestamp once and reuse for both preliminary and final
        // headers. Previously SystemTime::now() was called twice, which could yield
        // different seconds across the two headers (race / clock skew).
        let block_timestamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();

        // Get transactions from mempool
        let transactions = mempool.get_transactions_for_block(MAX_TRANSACTIONS_PER_BLOCK);

        // Create preliminary header to get block hash
        let preliminary_header = luxtensor_core::BlockHeader {
            version: 1,
            height: new_height,
            timestamp: block_timestamp,
            previous_hash: previous_block.hash(),
            state_root: [0u8; 32], // Will be updated after execution
            txs_root: [0u8; 32],
            receipts_root: [0u8; 32],
            validator: [0u8; 32],
            signature: vec![0u8; 64],
            gas_used: 0,
            gas_limit: BLOCK_GAS_LIMIT,
            extra_data: vec![],
        };

        let preliminary_block = Block::new(preliminary_header.clone(), transactions.clone());
        let block_hash = preliminary_block.hash();

        // Execute transactions against a snapshot (M-4 FIX: no lock held during execution)
        let accounts_snapshot = {
            let state = state_db.read();
            state.snapshot_accounts()
        };

        // Execute TXs on a temporary StateDB — no lock needed
        let mut temp_state = StateDB::from_accounts(accounts_snapshot);
        let mut valid_transactions = Vec::new();
        let mut valid_receipts = Vec::new();
        let mut total_gas = 0u64;

        // ── 🤖 Agentic EVM: process autonomous agent triggers ──
        // Agents get executed before user transactions, allowing them to react
        // to on-chain state changes from the previous block.
        let gas_price: u128 = 1_000_000_000; // 1 Gwei baseline for agent triggers
        let trigger_outcome = agent_trigger_engine.process_block_triggers(
            new_height, block_timestamp, gas_price,
        );
        if trigger_outcome.successful > 0 || trigger_outcome.failed > 0 {
            info!(
                "🤖 Block #{}: {} agent triggers executed ({} failed, {} skipped, {} gas)",
                new_height,
                trigger_outcome.successful,
                trigger_outcome.failed,
                trigger_outcome.skipped,
                trigger_outcome.total_gas_used,
            );
        }

        for (tx_index, tx) in transactions.into_iter().enumerate() {
            match executor.execute(
                &tx,
                &mut temp_state,
                new_height,
                block_hash,
                tx_index,
                block_timestamp,
            ) {
                Ok(receipt) => {
                    total_gas += receipt.gas_used;
                    valid_receipts.push(receipt);
                    valid_transactions.push(tx);
                }
                Err(e) => {
                    warn!("Transaction {:?} failed: {}", tx.hash(), e);
                }
            }
        }

        // Calculate transaction root
        let tx_hashes: Vec<[u8; 32]> = valid_transactions.iter().map(|tx| tx.hash()).collect();
        let txs_root =
            if tx_hashes.is_empty() { [0u8; 32] } else { MerkleTree::new(tx_hashes).root() };

        // Calculate receipts root
        let receipts_root = calculate_receipts_root(&valid_receipts);

        // Short write lock: merge results into shared state, then commit via
        // CachedStateDB for height-indexed root caching.
        //
        // Lock ordering: write lock for merge only, then drop before commit()
        // which acquires its own read lock internally.
        {
            let mut state = state_db.write();
            state.merge_accounts(temp_state.snapshot_accounts());
        }

        // Commit via merkle_cache: computes root & caches it by block height.
        // This acquires a read lock on state_db internally.
        let state_root = merkle_cache.commit(new_height)?;

        // Flush persisted state to disk
        {
            let state = state_db.read();
            if let Err(e) = state.flush_to_db(storage.as_ref()) {
                warn!("Failed to persist state to disk: {} (state is in-memory only)", e);
            }
        }
        // FIXED (M-4): Block production now uses clone-then-commit pattern.
        // Read lock is held only briefly to snapshot accounts, TX execution runs
        // against an unlocked temporary StateDB, and write lock is held only for
        // the final merge + commit + flush (<10ms). RPC reads are no longer blocked
        // during block production.

        // Create new block header with signing
        // First create unsigned header to get hash
        let mut unsigned_header = luxtensor_core::BlockHeader {
            version: 1,
            height: new_height,
            timestamp: block_timestamp, // 🔧 FIX MC-2: Reuse single timestamp
            previous_hash: previous_block.hash(),
            state_root,
            txs_root,
            receipts_root,
            validator: [0u8; 32],
            signature: vec![], // Empty for signing
            gas_used: total_gas,
            gas_limit: BLOCK_GAS_LIMIT,
            extra_data: vec![],
        };

        // Sign with validator keypair if available
        let (validator_pubkey, signature) = if let Some(keypair) = validator_keypair {
            // Get public key bytes (padded to 32 bytes for now)
            let address = keypair.address();
            let mut validator = [0u8; 32];
            validator[12..32].copy_from_slice(address.as_bytes());

            // Sign the unsigned header hash
            let header_hash = unsigned_header.hash();
            match keypair.sign(&header_hash) {
                Ok(sig) => {
                    info!(
                        "🔐 Block #{} signed by validator 0x{}",
                        new_height,
                        hex::encode(&address)
                    );
                    (validator, sig.to_vec())
                }
                Err(e) => {
                    error!(
                        "CRITICAL: Failed to sign block #{}: {}. \
                         Refusing to produce unsigned block in validator mode.",
                        new_height, e
                    );
                    return Err(anyhow::anyhow!(
                        "Block signing failed: {}. Validator cannot produce unsigned blocks.",
                        e
                    ));
                }
            }
        } else {
            // No validator keypair — node is not a validator, produce unsigned block
            // This is only allowed in dev mode or for non-validator observer nodes
            warn!("⚠️  Producing unsigned block #{} (no validator keypair configured)", new_height);
            ([0u8; 32], vec![0u8; 64])
        };

        // Update header with signature
        unsigned_header.validator = validator_pubkey;
        unsigned_header.signature = signature;
        let header = unsigned_header;

        // Create new block
        let block = Block::new(header.clone(), valid_transactions.clone());

        // Store block
        storage
            .store_block(&block)
            .context(format!("Failed to store block at height {}", header.height))?;

        // ── ⚖️ Optimistic AI: process disputes and finalize/slash ──
        // Run dispute resolution after storing the block so all state is committed.
        let slash_percent = slashing_manager.read().config().fraudulent_ai_slash_percent;
        let dispute_outcome = dispute_manager.process_block(new_height, slash_percent).await;
        if dispute_outcome.finalized_count > 0 || dispute_outcome.disputes_verified > 0 {
            info!(
                "⚖️ Block #{}: {} results finalized, {} disputes verified, {} rejected",
                new_height,
                dispute_outcome.finalized_count,
                dispute_outcome.disputes_verified,
                dispute_outcome.rejected_disputes,
            );
        }
        // Apply slashing for miners proven fraudulent via SlashingManager
        for (miner_addr, _slash_amount) in &dispute_outcome.slashed_miners {
            let miner_address = luxtensor_core::Address::from(*miner_addr);
            let evidence = luxtensor_consensus::slashing::SlashingEvidence {
                validator: miner_address,
                reason: luxtensor_consensus::slashing::SlashReason::FraudulentAI,
                height: new_height,
                evidence_hash: None,
                timestamp: block_timestamp,
            };
            match slashing_manager.write().slash(evidence, new_height) {
                Ok(event) => {
                    info!(
                        "⚖️ Slashed miner 0x{} for {} wei (fraudulent AI result, jailed: {})",
                        hex::encode(miner_addr),
                        event.amount_slashed,
                        event.jailed,
                    );
                }
                Err(e) => {
                    warn!(
                        "⚠️ Failed to slash miner 0x{} for FraudulentAI: {}",
                        hex::encode(miner_addr),
                        e,
                    );
                }
            }
        }

        // Update consensus with the new block hash for VRF entropy
        consensus.read().update_last_block_hash(block.hash());

        // Distribute block reward using halving schedule
        let producer_addr = if header.validator != [0u8; 32] {
            let mut addr = [0u8; 20];
            addr.copy_from_slice(&header.validator[12..32]);
            luxtensor_core::Address::from(addr)
        } else {
            luxtensor_core::Address::zero()
        };
        match consensus.read().distribute_reward_with_height(&producer_addr, new_height) {
            Ok(reward) if reward > 0 => {
                info!(
                    "💰 Block #{} reward: {} wei to 0x{}",
                    new_height,
                    reward,
                    hex::encode(producer_addr.as_bytes())
                );
            }
            Ok(_) => {}
            Err(e) => {
                debug!("Block reward distribution skipped: {}", e);
            }
        }

        // 🔧 FIX: Store receipts for eth_getTransactionReceipt
        for receipt in &valid_receipts {
            if let Ok(receipt_bytes) = bincode::serialize(receipt) {
                if let Err(e) = storage.store_receipt(&receipt.transaction_hash, &receipt_bytes) {
                    warn!("Failed to store receipt: {}", e);
                }
            }

            // Also store contract code if this was a deployment
            if let Some(ref contract_addr) = receipt.contract_address {
                // Get code from StateDB (bytecode is now stored in Account.code)
                if let Some(code) = state_db.read().get_code(contract_addr) {
                    if let Err(e) = storage.store_contract(contract_addr.as_bytes(), &code) {
                        warn!("Failed to store contract: {}", e);
                    }
                }
            }
        }

        // Remove transactions from mempool
        let tx_hashes: Vec<_> = valid_transactions.iter().map(|tx| tx.hash()).collect();
        mempool.remove_transactions(&tx_hashes);

        info!(
            "📦 Produced block #{} with {} transactions, {} gas used, hash {:?}",
            new_height,
            valid_transactions.len(),
            total_gas,
            block.hash()
        );

        // Check if this is an epoch boundary and process rewards
        if new_height % epoch_length == 0 && epoch_length > 0 {
            let epoch_num = new_height / epoch_length;
            info!(
                "🎯 Epoch {} completed at block #{}, processing rewards...",
                epoch_num, new_height
            );

            // Create utility metrics for this epoch
            // Calculate actual block utilization based on gas used vs gas limit
            let actual_utilization = ((total_gas as f64 / BLOCK_GAS_LIMIT as f64) * 100.0) as u32;

            // Query metagraph for active validators and neurons
            let metagraph_validators = metagraph_db.get_all_validators().unwrap_or_default();
            let metagraph_subnets = metagraph_db.get_all_subnets().unwrap_or_default();
            let metagraph_delegations = metagraph_db.get_all_delegations().unwrap_or_default();

            let active_validator_count =
                metagraph_validators.iter().filter(|v| v.is_active).count();

            let utility = UtilityMetrics {
                active_validators: active_validator_count.max(1) as u64,
                active_subnets: metagraph_subnets.len().max(1) as u64,
                // 🔧 FIX MC-6: Use accumulated epoch TX count (prior blocks + this block)
                epoch_transactions: epoch_tx_count + valid_transactions.len() as u64,
                epoch_ai_tasks: 0, // Tracked via MetagraphDB AI task store
                block_utilization: actual_utilization.min(100) as u8,
            };

            // Build miner list from neurons in all subnets
            let mut miners: Vec<MinerInfo> = Vec::new();
            for subnet in &metagraph_subnets {
                let neurons = metagraph_db.get_neurons_by_subnet(subnet.id).unwrap_or_default();
                for neuron in &neurons {
                    if neuron.active {
                        let score = neuron.incentive as f64 / 65535.0;
                        miners.push(MinerInfo {
                            address: neuron.hotkey,
                            score: if score > 0.0 { score } else { 0.01 }, // min score to avoid zero rewards
                            has_gpu: false, // Deprecated: use MinerEpochStats
                        });
                    }
                }
            }

            // Build validator list from metagraph
            let validators: Vec<ValidatorInfo> = metagraph_validators
                .iter()
                .filter(|v| v.is_active && v.stake > 0)
                .map(|v| ValidatorInfo { address: v.address, stake: v.stake })
                .collect();

            // Build delegator list from metagraph
            let delegators: Vec<DelegatorInfo> = metagraph_delegations
                .iter()
                .map(|d| DelegatorInfo {
                    address: d.delegator,
                    stake: d.amount,
                    lock_days: d.lock_days,
                })
                .collect();

            // Build subnet list for emission
            let subnets: Vec<SubnetInfo> = metagraph_subnets
                .iter()
                .map(|s| SubnetInfo { owner: s.owner, emission_weight: s.emission_rate })
                .collect();

            // Fallback: if metagraph is empty (bootstrapping), use block producer
            let miners = if miners.is_empty() {
                let miner_addr = if header.validator != [0u8; 32] {
                    let mut addr = [0u8; 20];
                    addr.copy_from_slice(&header.validator[12..32]);
                    addr
                } else {
                    [0u8; 20]
                };
                vec![MinerInfo { address: miner_addr, score: 1.0, has_gpu: false }]
            } else {
                miners
            };
            let validators = if validators.is_empty() {
                let miner_addr = if header.validator != [0u8; 32] {
                    let mut addr = [0u8; 20];
                    addr.copy_from_slice(&header.validator[12..32]);
                    addr
                } else {
                    [0u8; 20]
                };
                vec![ValidatorInfo { address: miner_addr, stake: 1000 }]
            } else {
                validators
            };

            // Process epoch rewards
            let result = reward_executor.write().process_epoch(
                epoch_num,
                new_height,
                &utility,
                &miners,
                &validators,
                &delegators,
                &subnets,
            );

            info!(
                "💰 Epoch {} rewards distributed: {} total emission, {} participants, {} DAO",
                epoch_num,
                result.total_emission,
                result.participants_rewarded,
                result.dao_allocation
            );

            // Finalize RANDAO mix for this epoch and feed it into PoS seed.
            // This provides unbiasable randomness for the next epoch's
            // validator selection, preventing leader-prediction attacks.
            match randao.write().finalize_epoch() {
                Ok(mix) => {
                    consensus.read().update_randao_mix(mix);
                    info!("🎲 Epoch {} RANDAO mix finalized: {:?}", epoch_num, &mix[..8]);
                }
                Err(e) => {
                    // Not fatal — PoS falls back to VRF-only seed when no RANDAO mix
                    // is available (e.g. during bootstrap when no validators have
                    // submitted commit-reveal yet).
                    debug!("⚠️  RANDAO finalize skipped for epoch {}: {}", epoch_num, e);
                }
            }
        }

        // Record block hash for EVM BLOCKHASH opcode (up to 256 recent blocks)
        executor.evm().record_block_hash(new_height, block.hash());

        Ok(block)
    }

    /// Print node status
    fn print_status(&self) {
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
    #[allow(dead_code)]
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

    /// Add transaction to mempool
    #[allow(dead_code)]
    pub fn add_transaction(&self, tx: Transaction) -> Result<()> {
        self.mempool
            .add_transaction(tx)
            .map_err(|e| anyhow::anyhow!("Failed to add transaction: {}", e))
    }

    /// Get mempool
    #[allow(dead_code)]
    pub fn mempool(&self) -> &Arc<Mempool> {
        &self.mempool
    }
}

/// Node statistics
#[allow(dead_code)]
#[derive(Debug, Clone)]
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

/// Check if the chain ID indicates a development/test chain.
/// Dev chains allow unsigned internal transactions to be signed with dev keys.
///
/// SECURITY: Only well-known test chain IDs are included here.
/// chain_id=1 (Ethereum Mainnet) was deliberately REMOVED to prevent
/// accidentally enabling dev-mode signing on production-like chains.
/// LuxTensor devnet uses chain_id=8898.
fn is_dev_chain(chain_id: u64) -> bool {
    matches!(chain_id, 1337 | 31337)
}

/// Sign a transaction with a development private key (proper secp256k1 signing)
/// Returns true if signing was successful, false if the address is not a known dev account
fn sign_transaction_with_dev_key(tx: &mut Transaction, from: &[u8; 20]) -> bool {
    use luxtensor_crypto::{keccak256, KeyPair};

    // Hardhat default private keys (for development only!)
    // These are well-known test keys - never use in production with real funds
    let dev_accounts: &[([u8; 20], [u8; 32])] = &[
        // Account #0: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
        (
            [
                0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8, 0x82, 0x72,
                0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66,
            ],
            [
                0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38,
                0xff, 0x94, 0x4b, 0xac, 0xb4, 0x78, 0xcb, 0xed, 0x5e, 0xfc, 0xae, 0x78, 0x4d, 0x7b,
                0xf4, 0xf2, 0xff, 0x80,
            ],
        ),
        // Account #1: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
        (
            [
                0x70, 0x99, 0x79, 0x70, 0xc5, 0x18, 0x12, 0xdc, 0x3a, 0x01, 0x0c, 0x7d, 0x01, 0xb5,
                0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xc8,
            ],
            [
                0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94,
                0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84, 0x12, 0xf4, 0x60, 0x3b,
                0x6b, 0x78, 0x69, 0x0d,
            ],
        ),
        // Account #2: 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
        (
            [
                0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58, 0x5d, 0xd2, 0x99, 0xe0,
                0x3d, 0x12, 0xfa, 0x42, 0x93, 0xbc,
            ],
            [
                0x5d, 0xe4, 0x11, 0x1a, 0xfa, 0x1a, 0x4b, 0x94, 0x90, 0x8f, 0x83, 0x10, 0x3e, 0xb1,
                0xf1, 0x70, 0x63, 0x67, 0xc2, 0xe6, 0x8c, 0xa8, 0x70, 0xfc, 0x3f, 0xb9, 0xa8, 0x04,
                0xcd, 0xab, 0x36, 0x5a,
            ],
        ),
    ];

    // Find matching private key (case-insensitive by converting to lowercase)
    let from_lower: [u8; 20] = from.map(|b| b.to_ascii_lowercase());

    for (addr, privkey) in dev_accounts {
        let addr_lower: [u8; 20] = addr.map(|b| b.to_ascii_lowercase());
        if addr_lower == from_lower {
            // Create keypair from private key using proper secp256k1
            match KeyPair::from_secret(privkey) {
                Ok(keypair) => {
                    // Verify keypair address matches expected address
                    let keypair_addr = keypair.address();
                    info!("🔑 Signing with keypair address: {:?}", keypair_addr);
                    info!("🔑 Expected from address: {:?}", from);

                    // Get signing message and hash (same format as verify_signature)
                    let msg = tx.signing_message();
                    let msg_hash = keccak256(&msg);
                    info!("🔑 Message hash: {:?}", &msg_hash[..8]);

                    // Sign with secp256k1 ECDSA
                    match keypair.sign(&msg_hash) {
                        Ok(signature) => {
                            tx.r.copy_from_slice(&signature[..32]);
                            tx.s.copy_from_slice(&signature[32..]);

                            // Try recovery ID 0 first, then 1 if verification fails
                            for v in [0u8, 1u8] {
                                tx.v = v;
                                if tx.verify_signature().is_ok() {
                                    info!("✅ Signature verified with v={}", v);
                                    return true;
                                }
                            }

                            // If neither works, log error
                            warn!("❌ Signature verification failed for both v=0 and v=1");
                            return false;
                        }
                        Err(e) => {
                            warn!("Failed to sign transaction: {}", e);
                            return false;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to create keypair: {}", e);
                    return false;
                }
            }
        }
    }

    warn!("No matching dev account found for address: {:?}", from);
    false
}
