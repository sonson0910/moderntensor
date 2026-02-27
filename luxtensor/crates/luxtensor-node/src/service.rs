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
use luxtensor_core::bridge::{BridgeConfig, PersistentBridge};
use luxtensor_storage::bridge_store::RocksDBBridgeStore;
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

// Utility functions have been extracted to `service_utils.rs` for modularity.
pub(crate) use crate::service_utils::{
    current_timestamp, parse_address_from_hex, detect_external_ip, peer_id_to_synthetic_ip,
};

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
    pub(crate) bridge: Arc<PersistentBridge>,
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
    /// Path to VTrust snapshot file (JSON-serialized VTrustSnapshot)
    pub(crate) vtrust_snapshot_path: std::path::PathBuf,
    /// Emission controller for block reward calculation with halving + utility adjustment
    pub(crate) emission_controller: Arc<RwLock<luxtensor_consensus::EmissionController>>,
    /// Halving schedule for Bitcoin-like reward reduction
    pub(crate) halving_schedule: Arc<luxtensor_consensus::HalvingSchedule>,
    /// Burn manager for token burning (tx fees, slashing, subnet registration)
    pub(crate) burn_manager: Arc<luxtensor_consensus::BurnManager>,
    /// EIP-1559 fee market for dynamic gas pricing
    pub(crate) fee_market: Arc<RwLock<luxtensor_consensus::FeeMarket>>,
    /// On-chain governance module for protocol parameter changes and upgrades
    pub(crate) governance: Arc<RwLock<luxtensor_consensus::GovernanceModule>>,
    /// Commit-reveal manager for tamper-proof validator weight submissions
    pub(crate) commit_reveal: Arc<luxtensor_consensus::CommitRevealManager>,
    /// Validator rotation manager for automatic epoch-based validator set updates
    pub(crate) validator_rotation: Arc<RwLock<luxtensor_consensus::ValidatorRotation>>,
    /// AI layer circuit breaker for cascading failure protection
    pub(crate) ai_circuit_breaker: Arc<luxtensor_consensus::AILayerCircuitBreaker>,
    /// Scoring manager for miner/validator performance tracking
    pub(crate) scoring_manager: Arc<RwLock<luxtensor_consensus::ScoringManager>>,
    /// World Semantic Index — cross-contract shared HNSW vector registry
    pub(crate) semantic_registry: Arc<RwLock<luxtensor_core::semantic_registry::SemanticRegistry>>,
    /// AI Precompile state (request tracking, training jobs) for EVM AI opcodes
    pub(crate) ai_precompile_state: Arc<luxtensor_contracts::AIPrecompileState>,
    /// Proof of Training verifier for federated learning gradient verification
    pub(crate) pot_verifier: Arc<RwLock<luxtensor_zkvm::pot_verifier::PoTVerifier>>,
    /// Oracle request processor for AI inference + ZK proof generation
    pub(crate) request_processor: Arc<luxtensor_oracle::RequestProcessor>,
    /// VRF keypair (secp256k1 EC-VRF) for attaching verifiable randomness proofs to blocks
    pub(crate) vrf_keypair: Option<Arc<luxtensor_crypto::vrf::VrfKeypair>>,
}

impl NodeService {
    /// Create a new node service.
    ///
    /// Delegates initialization to 5 phase methods for better readability:
    /// 1. [`init_storage_layer`] — BlockchainDB, StateDB, CachedStateDB, genesis
    /// 2. [`init_consensus_and_mempool`] — PoS, VTrust, mempool, rewards, allocation
    /// 3. [`init_security_modules`] — Eclipse/LongRange protection, liveness, health, BFT, fork choice
    /// 4. [`init_ai_pipeline`] — MetagraphDB, executor, TaskDispatcher, RANDAO, agents, disputes
    /// 5. [`init_tokenomics_and_infra`] — Emission, halving, burn, fees, governance, bridge, multisig
    pub async fn new(config: Config) -> Result<Self> {
        info!("🦀 Initializing LuxTensor Node v{}", env!("CARGO_PKG_VERSION"));
        info!("Node name: {}", config.node.name);
        info!("Chain ID: {}", config.node.chain_id);

        config.validate()?;
        std::fs::create_dir_all(&config.node.data_dir)
            .context(format!("Failed to create data directory: {:?}", config.node.data_dir))?;
        std::fs::create_dir_all(&config.storage.db_path)
            .context(format!("Failed to create storage directory: {:?}", config.storage.db_path))?;

        // Phase 1: Storage
        let (storage, state_db, merkle_cache, initial_best_height, genesis_hash) =
            Self::init_storage_layer(&config).await?;

        // Phase 2: Consensus + mempool
        let (consensus, vtrust_scorer, vtrust_snapshot_path, mempool,
             reward_executor, token_allocation, node_registry) =
            Self::init_consensus_and_mempool(&config, &storage, &state_db)?;

        // Phase 3: Security modules
        let (db_maintenance, eclipse_protection, long_range_protection,
             liveness_monitor, graceful_shutdown, health_monitor,
             fast_finality, fork_choice) =
            Self::init_security_modules(&config, &storage, &mempool, genesis_hash)?;

        // Phase 4: AI pipeline
        let (metagraph_db, executor, ai_precompile_state, task_dispatcher,
             randao, slashing_manager, agent_registry, agent_trigger_engine,
             dispute_manager, scoring_manager, semantic_registry,
             pot_verifier, request_processor) =
            Self::init_ai_pipeline(&config, &storage, genesis_hash).await?;

        // Phase 5: Tokenomics + infrastructure
        let (network_rate_limiter, emission_controller, halving_schedule,
             burn_manager, fee_market, governance, commit_reveal,
             validator_rotation, ai_circuit_breaker, bridge, multisig_manager) =
            Self::init_tokenomics_and_infra(&config, &storage)?;

        // Shared channels
        let (shutdown_tx, _) = broadcast::channel(16);
        let epoch_length = config.consensus.epoch_length;
        let genesis_timestamp = Self::resolve_genesis_timestamp(&storage);

        // Validator keypair + VRF
        let (validator_keypair, vrf_keypair) =
            Self::load_validator_keypair(&config, &consensus)?;

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
            broadcast_tx: None,
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
            is_syncing: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(initial_best_height == 0)),
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
            emission_controller,
            halving_schedule,
            burn_manager,
            fee_market,
            governance,
            commit_reveal,
            validator_rotation,
            ai_circuit_breaker,
            scoring_manager,
            semantic_registry,
            ai_precompile_state,
            pot_verifier,
            request_processor,
            vrf_keypair,
        })
    }

    // ========================================================================
    // Phase 1: Storage Layer
    // ========================================================================

    /// Initialize BlockchainDB, StateDB (with lazy bytecode loading), CachedStateDB,
    /// genesis block, and dev-mode accounts.
    async fn init_storage_layer(
        config: &Config,
    ) -> Result<(
        Arc<BlockchainDB>,
        Arc<RwLock<StateDB>>,
        Arc<CachedStateDB>,
        u64, // initial_best_height
        [u8; 32], // genesis_hash
    )> {
        info!("📦 Initializing storage...");
        let db_path_str = config
            .storage
            .db_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?;
        let storage = Arc::new(BlockchainDB::open(db_path_str)?);
        let initial_best_height = storage.get_best_height().unwrap_or(Some(0)).unwrap_or(0);
        info!("  ✓ Storage initialized at {:?}", config.storage.db_path);

        info!("💾 Initializing state database...");
        let state_db = Arc::new(RwLock::new(StateDB::new()));

        // Restore persisted state from RocksDB on startup
        {
            let mut state = state_db.write();
            match state.load_from_db(storage.as_ref()) {
                Ok(count) if count > 0 => info!("  ✓ Restored {} accounts from disk", count),
                Ok(_) => info!("  ✓ No persisted state found (fresh node)"),
                Err(e) => warn!("  ⚠️ Failed to load persisted state: {} (starting fresh)", e),
            }
        }

        // Wire lazy bytecode loader
        {
            let mut state = state_db.write();
            state.set_code_store(storage.clone());
        }
        info!("  ✓ State database initialized (lazy bytecode loading enabled)");

        // Merkle cache
        let storage_state_db = Arc::new(parking_lot::RwLock::new(
            luxtensor_storage::StateDB::new(storage.inner_db()),
        ));
        let merkle_cache = Arc::new(CachedStateDB::with_defaults(storage_state_db));
        info!("  ✓ Merkle cache initialized (height_cache=256, account_hashes=4096)");

        // Genesis block
        let genesis_missing = match storage.get_block_by_height(0) {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(e) => {
                warn!("  ⚠️ Corrupt genesis entry in DB ({}), overwriting", e);
                true
            }
        };
        if genesis_missing {
            info!("🌱 Creating genesis block...");
            let genesis = Block::genesis();
            storage.store_block(&genesis)?;
            info!("  ✓ Genesis block created: {:?}", genesis.hash());
        } else {
            info!("  ✓ Genesis block found");
        }

        // Genesis hash for downstream modules
        let genesis_hash = match storage.get_block_by_height(0) {
            Ok(Some(block)) => block.hash(),
            _ => [0u8; 32],
        };

        // Dev-mode accounts
        if config.node.dev_mode {
            let dev_accounts: &[[u8; 20]] = &[
                [0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8, 0x82,
                 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66],
                [0x70, 0x99, 0x79, 0x70, 0xc5, 0x18, 0x12, 0xdc, 0x3a, 0x01, 0x0c, 0x7d, 0x01,
                 0xb5, 0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xc8],
                [0x3c, 0x44, 0xcd, 0xdd, 0xb6, 0xa9, 0x00, 0xfa, 0x2b, 0x58, 0x5d, 0xd2, 0x99,
                 0xe0, 0x3d, 0x12, 0xfa, 0x42, 0x93, 0xbc],
            ];
            for addr_bytes in dev_accounts {
                let dev_address = luxtensor_core::Address::from(*addr_bytes);
                let mut dev_account = luxtensor_core::Account::new();
                dev_account.balance = 10_000_000_000_000_000_000_000_u128;
                state_db.write().set_account(dev_address, dev_account);
            }
            warn!("⚠️  DEV MODE: {} genesis accounts initialized with 10000 ETH each", dev_accounts.len());
        }

        Ok((storage, state_db, merkle_cache, initial_best_height, genesis_hash))
    }

    // ========================================================================
    // Phase 2: Consensus + Mempool
    // ========================================================================

    /// Initialize PoS consensus, VTrust scorer, mempool, rewards, allocation, registry.
    fn init_consensus_and_mempool(
        config: &Config,
        storage: &Arc<BlockchainDB>,
        state_db: &Arc<RwLock<StateDB>>,
    ) -> Result<(
        Arc<RwLock<ProofOfStake>>,
        Arc<RwLock<VTrustScorer>>,
        std::path::PathBuf,
        Arc<Mempool>,
        Arc<RwLock<RewardExecutor>>,
        Arc<RwLock<TokenAllocation>>,
        Arc<RwLock<NodeRegistry>>,
    )> {
        info!("⚖️  Initializing consensus...");
        let consensus_config = ConsensusConfig {
            slot_duration: config.consensus.block_time,
            min_stake: config.consensus.min_stake.parse().map_err(|e| {
                anyhow::anyhow!("min_stake '{}' is not a valid u128: {}", config.consensus.min_stake, e)
            })?,
            block_reward: luxtensor_core::constants::tokenomics::INITIAL_BLOCK_REWARD,
            epoch_length: config.consensus.epoch_length,
            ..Default::default()
        };
        let consensus = Arc::new(RwLock::new(ProofOfStake::new(consensus_config)));
        info!("  ✓ PoS consensus initialized (min_stake: {}, epoch: {} blocks)",
            config.consensus.min_stake, config.consensus.epoch_length);

        // VTrust scorer
        let vtrust_snapshot_path = config.node.data_dir.join("vtrust_snapshot.json");
        let vtrust_scorer = {
            let mut scorer = VTrustScorer::new();
            match std::fs::read(&vtrust_snapshot_path) {
                Ok(bytes) => match serde_json::from_slice::<VTrustSnapshot>(&bytes) {
                    Ok(snapshot) => { scorer.restore(snapshot); info!("  ✓ VTrust scorer restored"); }
                    Err(e) => warn!("  ⚠️ Failed to deserialize VTrust snapshot: {}", e),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => info!("  ✓ No VTrust snapshot found"),
                Err(e) => warn!("  ⚠️ Failed to read VTrust snapshot: {}", e),
            }
            Arc::new(RwLock::new(scorer))
        };

        // Mempool
        info!("📝 Initializing transaction mempool...");
        let mempool = Arc::new(Mempool::new(config.mempool.max_size, config.node.chain_id));
        info!("  ✓ Mempool initialized (max: {}, chain_id: {})", config.mempool.max_size, config.node.chain_id);

        // Reward executor
        let dao_address = parse_address_from_hex(&config.node.dao_address).unwrap_or_else(|_| {
            warn!("Invalid DAO address in config, using zero address");
            [0u8; 20]
        });
        let reward_executor = Arc::new(RwLock::new(RewardExecutor::new(dao_address)));
        info!("  ✓ Reward executor initialized with DAO: 0x{}", hex::encode(&dao_address));

        let tge_timestamp = current_timestamp();
        let token_allocation = Arc::new(RwLock::new(TokenAllocation::new(tge_timestamp)));
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        info!("  ✓ Token allocation + node registry initialized");

        // Suppress unused variable warnings — these refs are used for type validation
        let _ = (storage, state_db);

        Ok((consensus, vtrust_scorer, vtrust_snapshot_path, mempool,
            reward_executor, token_allocation, node_registry))
    }

    // ========================================================================
    // Phase 3: Security Modules
    // ========================================================================

    /// Initialize security, monitoring, and consensus finality modules.
    fn init_security_modules(
        config: &Config,
        storage: &Arc<BlockchainDB>,
        mempool: &Arc<Mempool>,
        genesis_hash: [u8; 32],
    ) -> Result<(
        Arc<DbMaintenance>,
        Arc<EclipseProtection>,
        Arc<LongRangeProtection>,
        Arc<RwLock<LivenessMonitor>>,
        Arc<GracefulShutdown>,
        Arc<RwLock<HealthMonitor>>,
        Arc<RwLock<FastFinality>>,
        Arc<RwLock<ForkChoice>>,
    )> {
        let db_maintenance = Arc::new(DbMaintenance::new(
            config.storage.db_path.clone(),
            BackupConfig {
                backup_dir: config.node.data_dir.join("backups"),
                max_backups: 5,
                compress: true,
            },
            PruningConfig::default(),
        ));

        let eclipse_protection = Arc::new(EclipseProtection::new(EclipseConfig::default()));
        let long_range_protection = Arc::new(LongRangeProtection::new(LongRangeConfig::default(), genesis_hash));
        let liveness_monitor = Arc::new(RwLock::new(LivenessMonitor::new(LivenessConfig::default())));
        let graceful_shutdown = Arc::new(GracefulShutdown::new(ShutdownConfig::default()));
        info!("  ✓ Security modules initialized (eclipse, long-range, liveness, graceful shutdown)");

        // Restore mempool from previous shutdown backup
        {
            let mempool_backup_path = format!("{}/mempool.bin", graceful_shutdown.config().backup_dir);
            match mempool.load_from_file(&mempool_backup_path) {
                Ok(0) => {}
                Ok(count) => info!("  ✓ Restored {} pending transactions from previous session", count),
                Err(e) => warn!("  ⚠️ Failed to load mempool backup: {} (fresh)", e),
            }
        }

        let health_monitor = Arc::new(RwLock::new(HealthMonitor::new(HealthConfig::default())));

        let fast_finality = Arc::new(RwLock::new(
            FastFinality::new(67, luxtensor_consensus::ValidatorSet::new())
                .map_err(|e| anyhow::anyhow!("FastFinality init failed: {}", e))?,
        ));
        info!("  ✓ Fast finality initialized (BFT threshold: 67%)");

        let genesis_block = match storage.get_block_by_height(0) {
            Ok(Some(block)) => block,
            _ => Block::genesis(),
        };
        let fork_choice = Arc::new(RwLock::new(ForkChoice::new(genesis_block)));
        info!("  ✓ Fork choice (GHOST) initialized");

        Ok((db_maintenance, eclipse_protection, long_range_protection,
            liveness_monitor, graceful_shutdown, health_monitor,
            fast_finality, fork_choice))
    }

    // ========================================================================
    // Phase 4: AI Pipeline
    // ========================================================================

    /// Initialize MetagraphDB, transaction executor, AI task dispatcher, agents, and scoring.
    async fn init_ai_pipeline(
        config: &Config,
        _storage: &Arc<BlockchainDB>,
        genesis_hash: [u8; 32],
    ) -> Result<(
        Arc<MetagraphDB>,
        Arc<TransactionExecutor>,
        Arc<luxtensor_contracts::AIPrecompileState>,
        Arc<TaskDispatcher>,
        Arc<RwLock<RandaoMixer>>,
        Arc<RwLock<SlashingManager>>,
        Arc<AgentRegistry>,
        Arc<AgentTriggerEngine>,
        Arc<DisputeManager>,
        Arc<RwLock<luxtensor_consensus::ScoringManager>>,
        Arc<RwLock<luxtensor_core::semantic_registry::SemanticRegistry>>,
        Arc<RwLock<luxtensor_zkvm::pot_verifier::PoTVerifier>>,
        Arc<luxtensor_oracle::RequestProcessor>,
    )> {
        // MetagraphDB
        let metagraph_path = config.node.data_dir.join("metagraph");
        std::fs::create_dir_all(&metagraph_path)
            .context(format!("Failed to create metagraph directory: {:?}", metagraph_path))?;
        let metagraph_db = Arc::new(
            MetagraphDB::open(&metagraph_path)
                .context(format!("Failed to open metagraph DB at {:?}", metagraph_path))?,
        );
        info!("  ✓ Metagraph DB initialized at {:?}", metagraph_path);

        // AI precompile state + executor
        let ai_precompile_state = Arc::new(luxtensor_contracts::AIPrecompileState::new());
        info!("⚡ Initializing transaction executor...");
        let executor = Arc::new(
            TransactionExecutor::new(config.node.chain_id)
                .with_metagraph(metagraph_db.clone())
                .with_ai_precompiles(ai_precompile_state.clone())
        );
        info!("  ✓ Transaction executor initialized (chain_id: {}, metagraph + AI precompiles)", config.node.chain_id);

        // Task dispatcher, RANDAO, slashing
        let task_dispatcher = Arc::new(TaskDispatcher::new(metagraph_db.clone(), DispatcherConfig::default()));
        let randao = Arc::new(RwLock::new(RandaoMixer::with_genesis(RandaoConfig::default(), genesis_hash)));
        let slashing_manager = Arc::new(RwLock::new(SlashingManager::new(
            SlashingConfig::default(),
            Arc::new(RwLock::new(luxtensor_consensus::ValidatorSet::new())),
        )));

        // Agentic EVM
        let agent_registry = Arc::new(AgentRegistry::with_defaults());
        let agent_evm = Arc::new(EvmExecutor::new(config.node.chain_id as u64));
        let agent_trigger_engine = Arc::new(AgentTriggerEngine::new(agent_registry.clone(), agent_evm));
        info!("  ✓ AI pipeline initialized (task dispatcher, RANDAO, slashing, agentic EVM)");

        // Disputes, scoring, semantic
        let dispute_manager = Arc::new(DisputeManager::default_config());
        let scoring_manager = Arc::new(RwLock::new(luxtensor_consensus::ScoringManager::new()));
        let semantic_registry = Arc::new(RwLock::new(
            luxtensor_core::semantic_registry::SemanticRegistry::default(),
        ));
        let pot_verifier = Arc::new(RwLock::new(luxtensor_zkvm::pot_verifier::PoTVerifier::new()));
        info!("  ✓ Dispute manager + scoring + semantic registry + PoT verifier initialized");

        // Oracle processor
        let request_processor = Arc::new(luxtensor_oracle::RequestProcessor::new());
        if let Some(ref elf_path) = config.node.oracle_elf_path {
            match std::fs::read(elf_path) {
                Ok(elf_bytes) => {
                    info!("  📦 Loading Oracle ELF ({} bytes)", elf_bytes.len());
                    match request_processor.initialize(Some(elf_bytes)).await {
                        Ok(()) => info!("  ✓ Oracle processor initialized (production ZK proofs)"),
                        Err(e) => warn!("  ⚠️ Oracle ELF init failed: {} (dev-mode)", e),
                    }
                }
                Err(e) => warn!("  ⚠️ Failed to read Oracle ELF: {} (dev-mode)", e),
            }
        } else {
            info!("  ✓ Oracle processor initialized (dev-mode)");
        }

        Ok((metagraph_db, executor, ai_precompile_state, task_dispatcher,
            randao, slashing_manager, agent_registry, agent_trigger_engine,
            dispute_manager, scoring_manager, semantic_registry,
            pot_verifier, request_processor))
    }

    // ========================================================================
    // Phase 5: Tokenomics + Infrastructure
    // ========================================================================

    /// Initialize emission, halving, burn, fees, governance, bridge, and multisig.
    fn init_tokenomics_and_infra(
        _config: &Config,
        storage: &Arc<BlockchainDB>,
    ) -> Result<(
        Arc<NetworkRateLimiter>,
        Arc<RwLock<luxtensor_consensus::EmissionController>>,
        Arc<luxtensor_consensus::HalvingSchedule>,
        Arc<luxtensor_consensus::BurnManager>,
        Arc<RwLock<luxtensor_consensus::FeeMarket>>,
        Arc<RwLock<luxtensor_consensus::GovernanceModule>>,
        Arc<luxtensor_consensus::CommitRevealManager>,
        Arc<RwLock<luxtensor_consensus::ValidatorRotation>>,
        Arc<luxtensor_consensus::AILayerCircuitBreaker>,
        Arc<PersistentBridge>,
        Arc<MultisigManager>,
    )> {
        let network_rate_limiter = Arc::new(NetworkRateLimiter::new(NetworkRateLimiterConfig {
            requests_per_second: 50,
            burst_size: 100,
            ban_duration: std::time::Duration::from_secs(300),
            violations_before_ban: 10,
        }));

        let emission_controller = Arc::new(RwLock::new(
            luxtensor_consensus::EmissionController::new(luxtensor_consensus::EmissionConfig::default()),
        ));
        let halving_schedule = Arc::new(luxtensor_consensus::HalvingSchedule::default());
        let burn_manager = Arc::new(
            luxtensor_consensus::BurnManager::new(luxtensor_consensus::BurnConfig::default()),
        );
        let fee_market = Arc::new(RwLock::new(luxtensor_consensus::FeeMarket::new()));
        let governance = Arc::new(RwLock::new(
            luxtensor_consensus::GovernanceModule::new(luxtensor_consensus::GovernanceConfig::default()),
        ));
        let commit_reveal = Arc::new(
            luxtensor_consensus::CommitRevealManager::new(luxtensor_consensus::CommitRevealConfig::default()),
        );
        let validator_rotation = Arc::new(RwLock::new(
            luxtensor_consensus::ValidatorRotation::new(luxtensor_consensus::RotationConfig::default()),
        ));
        let ai_circuit_breaker = Arc::new(luxtensor_consensus::AILayerCircuitBreaker::new());
        info!("  ✓ Tokenomics pipeline initialized (emission, halving, burn, EIP-1559, governance, rotation)");

        // Bridge
        let bridge_store = Arc::new(RocksDBBridgeStore::new(storage.inner_db()));
        let bridge = Arc::new(
            PersistentBridge::new(bridge_store, BridgeConfig::default())
                .map_err(|e| luxtensor_core::CoreError::InvalidState(format!("Bridge init: {}", e)))?
        );
        let multisig_manager = Arc::new(MultisigManager::new());
        info!("  ✓ Bridge + multisig initialized");

        Ok((network_rate_limiter, emission_controller, halving_schedule,
            burn_manager, fee_market, governance, commit_reveal,
            validator_rotation, ai_circuit_breaker, bridge, multisig_manager))
    }

    // ========================================================================
    // Shared helpers for new()
    // ========================================================================

    /// Resolve genesis timestamp from the stored genesis block, falling back to system time.
    fn resolve_genesis_timestamp(storage: &Arc<BlockchainDB>) -> u64 {
        match storage.get_block_by_height(0) {
            Ok(Some(block)) => block.header.timestamp,
            Ok(None) => {
                tracing::warn!("No genesis block found — using system time as genesis timestamp.");
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_secs()
            }
            Err(e) => {
                tracing::warn!("Could not read genesis block for timestamp ({}), using system time", e);
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::ZERO)
                    .as_secs()
            }
        }
    }

    /// Load validator keypair and VRF keypair from config, if this node is a validator.
    fn load_validator_keypair(
        config: &Config,
        _consensus: &Arc<RwLock<ProofOfStake>>,
    ) -> Result<(Option<KeyPair>, Option<Arc<luxtensor_crypto::vrf::VrfKeypair>>)> {
        if !config.node.is_validator {
            return Ok((None, None));
        }

        let key_path = match &config.node.validator_key_path {
            Some(p) => p,
            None => {
                warn!("Validator mode enabled but no key path configured, blocks will be unsigned");
                return Ok((None, None));
            }
        };

        let key_bytes = match std::fs::read(key_path) {
            Ok(bytes) if bytes.len() >= 32 => bytes,
            Ok(_) => {
                warn!("Validator key file too short, need at least 32 bytes");
                return Ok((None, None));
            }
            Err(e) => {
                warn!("Could not read validator key file: {}", e);
                return Ok((None, None));
            }
        };

        let mut secret = [0u8; 32];
        secret.copy_from_slice(&key_bytes[..32]);
        let result = KeyPair::from_secret(&secret);

        // VRF keypair derivation (secp256k1 EC-VRF) — MUST be done BEFORE zeroization
        let vrf_kp = match luxtensor_crypto::vrf::VrfKeypair::from_seed(&secret) {
            Ok(kp) => { info!("🎲 VRF keypair derived (secp256k1 EC-VRF)"); Some(Arc::new(kp)) }
            Err(e) => { warn!("⚠️ Failed to derive VRF keypair: {}", e); None }
        };

        #[cfg(feature = "production-vrf")]
        {
            match _consensus.read().set_vrf_key(&secret) {
                Ok(()) => info!("🎲 VRF key loaded (production-vrf)"),
                Err(e) => warn!("Failed to set VRF key: {}", e),
            }
        }

        // SECURITY: Zeroize secret key bytes after use
        secret.iter_mut().for_each(|b| *b = 0);

        match result {
            Ok(keypair) => {
                info!("🔑 Loaded validator key, address: 0x{}", hex::encode(&keypair.address()));
                Ok((Some(keypair), vrf_kp))
            }
            Err(e) => {
                warn!("Failed to parse validator key: {}", e);
                Ok((None, None))
            }
        }
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
            match serde_json::to_vec(&snapshot) {
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
}
