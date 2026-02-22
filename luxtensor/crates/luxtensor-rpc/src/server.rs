use crate::logs::LogStore;
use crate::rate_limiter::RateLimiter;
use crate::{
    eth_rpc::{register_aa_methods, register_eth_methods, register_log_methods, Mempool},
    types::*,
    NoOpBroadcaster, Result, RpcError, TransactionBroadcaster,
};
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_consensus::{
    AILayerCircuitBreaker, CommitRevealConfig, CommitRevealManager, ValidatorSet,
};
use luxtensor_core::{Hash, Transaction, UnifiedStateDB};
use luxtensor_storage::{BlockchainDB, CachedStateDB, MetagraphDB};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::ai_rpc::{register_ai_methods as register_ai_methods_new, AiRpcContext};
use crate::handlers::{
    register_checkpoint_handlers, register_neuron_handlers, register_staking_handlers,
    register_subnet_handlers, register_weight_handlers,
};
use crate::helpers::{parse_address, parse_block_number_with_latest};
#[cfg(test)]
use crate::helpers::parse_block_number;
use crate::query_rpc::{register_query_methods as register_query_methods_new, QueryRpcContext};
use crate::tx_rpc::{register_tx_methods, TxRpcContext};
use crate::agent_rpc::{register_agent_methods as register_agent_methods_new, AgentRpcContext};
use crate::dispute_rpc::{register_dispute_methods as register_dispute_methods_new, DisputeRpcContext};
use crate::bridge_rpc::{register_bridge_methods, BridgeRpcContext};
use crate::multisig_rpc::{register_multisig_methods, MultisigRpcContext};
use luxtensor_contracts::AgentRegistry;
use luxtensor_core::bridge::InMemoryBridge;
use luxtensor_core::multisig::MultisigManager;
use luxtensor_oracle::DisputeManager;
use std::path::PathBuf;
use tracing::warn;

/// JSON-RPC server for LuxTensor blockchain
///
/// # Production Design
///
/// Uses hybrid storage approach:
/// - In-memory caches for fast read access
/// - MetagraphDB for persistent storage (survives restarts)
/// - Writes sync to both cache and DB
///
/// # Example
///
/// ```ignore
/// use luxtensor_rpc::{RpcServer, BroadcasterBuilder};
/// use luxtensor_storage::MetagraphDB;
///
/// let metagraph_db = MetagraphDB::open("./data/metagraph").unwrap();
/// let broadcaster = BroadcasterBuilder::new()
///     .with_p2p(p2p_sender)
///     .build();
///
/// let server = RpcServer::new(db, state, metagraph_db, broadcaster);
/// ```
pub struct RpcServer {
    db: Arc<BlockchainDB>,
    validators: Arc<RwLock<ValidatorSet>>,
    // Persistent storage (RocksDB)
    metagraph: Arc<MetagraphDB>,
    // In-memory caches for fast access
    subnets: Arc<DashMap<u64, SubnetInfo>>,
    neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
    weights: Arc<DashMap<(u64, u64), Vec<WeightInfo>>>,
    pending_txs: Arc<DashMap<Hash, Transaction>>,
    ai_tasks: Arc<DashMap<Hash, AITaskInfo>>,
    broadcaster: Arc<dyn TransactionBroadcaster>,
    mempool: Arc<RwLock<Mempool>>,
    commit_reveal: Arc<RwLock<CommitRevealManager>>,
    /// Circuit breaker for AI layer operations
    ai_circuit_breaker: Arc<AILayerCircuitBreaker>,
    /// Rate limiter for DoS protection
    rate_limiter: Arc<RateLimiter>,
    /// Data directory for checkpoints and other persistent data
    data_dir: PathBuf,
    /// Atomic cache for block number (lock-free fast path)
    cached_block_number: Arc<AtomicU64>,
    /// Atomic cache for chain ID (constant, never changes)
    cached_chain_id: Arc<AtomicU64>,
    /// Unified state - THE source of truth for all state operations
    unified_state: Arc<RwLock<UnifiedStateDB>>,
    /// Agentic EVM — agent registry for on-chain autonomous agents
    agent_registry: Option<Arc<AgentRegistry>>,
    /// Optimistic AI — dispute manager for fraud-proof resolution
    dispute_manager: Option<Arc<DisputeManager>>,
    /// Cross-chain bridge for asset transfers
    bridge: Option<Arc<InMemoryBridge>>,
    /// Multisig wallet manager for multi-signature transactions
    multisig_manager: Option<Arc<MultisigManager>>,
    /// Merkle root cache for state root caching stats (optional)
    merkle_cache: Option<Arc<CachedStateDB>>,
    /// Shared EVM executor from block execution for eth_call storage reads
    evm_executor: Option<luxtensor_contracts::EvmExecutor>,
}

impl RpcServer {
    /// M-3 FIX: Builder pattern for constructing `RpcServer`.
    ///
    /// Consolidates the 6 separate constructors into a single composable API.
    /// Required fields: `db`, `chain_id`. All other fields have sensible defaults.
    ///
    /// # Example
    /// ```ignore
    /// let server = RpcServer::builder(db, 8898)
    ///     .metagraph(metagraph)
    ///     .broadcaster(broadcaster)
    ///     .mempool(mempool)
    ///     .validators(validator_set)
    ///     .data_dir("./data".into())
    ///     .build();
    /// ```
    pub fn builder(db: Arc<BlockchainDB>, chain_id: u64) -> RpcServerBuilder {
        RpcServerBuilder {
            db,
            chain_id,
            metagraph: None,
            broadcaster: None,
            mempool: None,
            pending_txs: None,
            validators: None,
            data_dir: None,
        }
    }

    /// Create a new RPC server with persistent MetagraphDB
    pub fn new(
        db: Arc<BlockchainDB>,
        metagraph: Arc<MetagraphDB>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
        chain_id: u64,
    ) -> Self {
        // Load initial data from metagraph into caches
        let subnets = Arc::new(DashMap::from_iter(Self::load_subnets_cache(&metagraph)));
        let neurons = Arc::new(DashMap::from_iter(Self::load_neurons_cache(&metagraph)));

        Self {
            db,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets,
            neurons,
            weights: Arc::new(DashMap::new()),
            pending_txs: Arc::new(DashMap::new()),
            ai_tasks: Arc::new(DashMap::new()),
            broadcaster,
            mempool: Arc::new(RwLock::new(Mempool::new())),
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(
                CommitRevealConfig::default(),
            ))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            data_dir: PathBuf::from("./data"), // Default data directory
            cached_block_number: Arc::new(AtomicU64::new(0)),
            cached_chain_id: Arc::new(AtomicU64::new(chain_id)),
            unified_state: Arc::new(RwLock::new(UnifiedStateDB::new(chain_id))),
            agent_registry: None,
            dispute_manager: None,
            bridge: None,
            multisig_manager: None,
            merkle_cache: None,
            evm_executor: None,
        }
    }

    /// Create a new RPC server for testing (uses temp storage)
    #[cfg(test)]
    pub fn new_for_testing(db: Arc<BlockchainDB>) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_test_{}", std::process::id()));
        let metagraph =
            Arc::new(MetagraphDB::open(&temp_dir).expect("Failed to create test MetagraphDB"));
        Self::new(db, metagraph, Arc::new(NoOpBroadcaster), 8898) // LuxTensor devnet chain_id
    }

    /// Get mempool reference for block production polling
    pub fn mempool(&self) -> Arc<RwLock<Mempool>> {
        self.mempool.clone()
    }

    /// Get AI layer circuit breaker reference for monitoring
    pub fn ai_circuit_breaker(&self) -> Arc<AILayerCircuitBreaker> {
        self.ai_circuit_breaker.clone()
    }

    /// Get chain ID (fast atomic read)
    pub fn chain_id(&self) -> u64 {
        self.cached_chain_id.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get unified state reference (C1 Phase 2B)
    pub fn unified_state(&self) -> Arc<RwLock<UnifiedStateDB>> {
        self.unified_state.clone()
    }

    /// Set the cross-chain bridge instance (optional, enables bridge_* RPC methods)
    pub fn set_bridge(&mut self, bridge: Arc<InMemoryBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set the multisig wallet manager (optional, enables multisig_* RPC methods)
    pub fn set_multisig_manager(&mut self, manager: Arc<MultisigManager>) {
        self.multisig_manager = Some(manager);
    }

    /// Set the merkle cache (optional, enables system_cacheStats RPC method)
    pub fn set_merkle_cache(&mut self, cache: Arc<CachedStateDB>) {
        self.merkle_cache = Some(cache);
    }

    /// Set the shared EVM executor for eth_call storage reads
    pub fn set_evm_executor(&mut self, executor: luxtensor_contracts::EvmExecutor) {
        self.evm_executor = Some(executor);
    }

    /// Create a new RPC server for testing with external mempool
    #[cfg(test)]
    pub fn new_for_testing_with_mempool(
        db: Arc<BlockchainDB>,
        mempool: Arc<RwLock<Mempool>>,
    ) -> Self {
        let chain_id = 8898_u64; // LuxTensor devnet chain_id
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_test_{}", std::process::id()));
        let metagraph =
            Arc::new(MetagraphDB::open(&temp_dir).expect("Failed to create test MetagraphDB"));

        Self {
            db,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets: Arc::new(DashMap::new()),
            neurons: Arc::new(DashMap::new()),
            weights: Arc::new(DashMap::new()),
            pending_txs: Arc::new(DashMap::new()),
            ai_tasks: Arc::new(DashMap::new()),
            broadcaster: Arc::new(NoOpBroadcaster),
            mempool,
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(
                CommitRevealConfig::default(),
            ))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            data_dir: temp_dir,
            cached_block_number: Arc::new(AtomicU64::new(0)),
            cached_chain_id: Arc::new(AtomicU64::new(chain_id)),
            unified_state: Arc::new(RwLock::new(UnifiedStateDB::new(chain_id))),
            agent_registry: None,
            dispute_manager: None,
            bridge: None,
            multisig_manager: None,
            merkle_cache: None,
            evm_executor: None,
        }
    }

    /// Create a new RPC server with external mempool and P2P broadcaster
    /// Use this for production multi-node setup
    /// 🔧 FIX: Added chain_id parameter — was hardcoded to 1337
    pub fn new_with_mempool_and_broadcaster(
        db: Arc<BlockchainDB>,
        mempool: Arc<RwLock<Mempool>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
        chain_id: u64,
    ) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_{}", std::process::id()));
        let metagraph = Arc::new(MetagraphDB::open(&temp_dir).unwrap_or_else(|e| {
            tracing::error!(
                "MetagraphDB::open failed at {:?}: {} — falling back to in-memory temp",
                temp_dir,
                e
            );
            // Retry with a unique fallback path
            let fallback =
                std::env::temp_dir().join(format!("luxtensor_fb_{}", std::process::id()));
            MetagraphDB::open(&fallback)
                .unwrap_or_else(|e2| {
                    // SECURITY: Clean exit instead of panic to avoid stack-unwind side effects
                    tracing::error!("FATAL: MetagraphDB fallback also failed: {} — shutting down", e2);
                    std::process::exit(1);
                })
        }));

        Self {
            db,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets: Arc::new(DashMap::new()),
            neurons: Arc::new(DashMap::new()),
            weights: Arc::new(DashMap::new()),
            pending_txs: Arc::new(DashMap::new()),
            ai_tasks: Arc::new(DashMap::new()),
            broadcaster,
            mempool,
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(
                CommitRevealConfig::default(),
            ))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            data_dir: temp_dir,
            cached_block_number: Arc::new(AtomicU64::new(0)),
            cached_chain_id: Arc::new(AtomicU64::new(chain_id)),
            unified_state: Arc::new(RwLock::new(UnifiedStateDB::new(chain_id))),
            agent_registry: None,
            dispute_manager: None,
            bridge: None,
            multisig_manager: None,
            merkle_cache: None,
            evm_executor: None,
        }
    }

    /// Create a new RPC server with external shared pending_txs for unified storage
    /// Use this when you need P2P handlers to share the same TX pool as RPC
    /// 🔧 FIX: Added chain_id parameter — was hardcoded to 1337
    pub fn new_with_shared_pending_txs(
        db: Arc<BlockchainDB>,
        mempool: Arc<RwLock<Mempool>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
        pending_txs: Arc<DashMap<Hash, Transaction>>,
        chain_id: u64,
    ) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_{}", std::process::id()));
        let metagraph = Arc::new(MetagraphDB::open(&temp_dir).unwrap_or_else(|e| {
            tracing::error!(
                "MetagraphDB::open failed at {:?}: {} — falling back to in-memory temp",
                temp_dir,
                e
            );
            let fallback =
                std::env::temp_dir().join(format!("luxtensor_fb_{}", std::process::id()));
            MetagraphDB::open(&fallback)
                .unwrap_or_else(|e2| {
                    // SECURITY: Clean exit instead of panic to avoid stack-unwind side effects
                    tracing::error!("FATAL: MetagraphDB fallback also failed: {} — shutting down", e2);
                    std::process::exit(1);
                })
        }));

        Self {
            db,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets: Arc::new(DashMap::new()),
            neurons: Arc::new(DashMap::new()),
            weights: Arc::new(DashMap::new()),
            pending_txs,
            ai_tasks: Arc::new(DashMap::new()),
            broadcaster,
            mempool,
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(
                CommitRevealConfig::default(),
            ))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            data_dir: temp_dir,
            cached_block_number: Arc::new(AtomicU64::new(0)),
            cached_chain_id: Arc::new(AtomicU64::new(chain_id)),
            unified_state: Arc::new(RwLock::new(UnifiedStateDB::new(chain_id))),
            agent_registry: None,
            dispute_manager: None,
            bridge: None,
            multisig_manager: None,
            merkle_cache: None,
            evm_executor: None,
        }
    }

    /// Create a new RPC server with validator set
    pub fn with_validators(
        db: Arc<BlockchainDB>,
        metagraph: Arc<MetagraphDB>,
        validators: Arc<RwLock<ValidatorSet>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
        chain_id: u64,
    ) -> Self {
        let subnets = Arc::new(DashMap::from_iter(Self::load_subnets_cache(&metagraph)));
        let neurons = Arc::new(DashMap::from_iter(Self::load_neurons_cache(&metagraph)));

        Self {
            db,
            validators,
            metagraph,
            subnets,
            neurons,
            weights: Arc::new(DashMap::new()),
            pending_txs: Arc::new(DashMap::new()),
            ai_tasks: Arc::new(DashMap::new()),
            broadcaster,
            mempool: Arc::new(RwLock::new(Mempool::new())),
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(
                CommitRevealConfig::default(),
            ))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            data_dir: PathBuf::from("./data"),
            cached_block_number: Arc::new(AtomicU64::new(0)),
            cached_chain_id: Arc::new(AtomicU64::new(chain_id)),
            unified_state: Arc::new(RwLock::new(UnifiedStateDB::new(chain_id))),
            agent_registry: None,
            dispute_manager: None,
            bridge: None,
            multisig_manager: None,
            merkle_cache: None,
            evm_executor: None,
        }
    }

    /// Load subnets from MetagraphDB into cache
    fn load_subnets_cache(metagraph: &MetagraphDB) -> HashMap<u64, SubnetInfo> {
        let mut cache = HashMap::new();
        if let Ok(subnets) = metagraph.get_all_subnets() {
            for subnet in subnets {
                cache.insert(
                    subnet.id,
                    SubnetInfo {
                        id: subnet.id,
                        name: subnet.name.clone(),
                        owner: format!("0x{}", hex::encode(subnet.owner)),
                        emission_rate: subnet.emission_rate,
                        participant_count: 0,
                        total_stake: 0,
                        created_at: subnet.created_at,
                    },
                );
            }
        }
        cache
    }

    /// Load neurons from MetagraphDB into cache
    fn load_neurons_cache(metagraph: &MetagraphDB) -> HashMap<(u64, u64), NeuronInfo> {
        // Load neurons for each subnet
        let mut cache = HashMap::new();
        if let Ok(subnets) = metagraph.get_all_subnets() {
            for subnet in subnets {
                if let Ok(neurons) = metagraph.get_neurons_by_subnet(subnet.id) {
                    for neuron in neurons {
                        let trust_val = neuron.trust as f64 / 65535.0;
                        cache.insert(
                            (neuron.subnet_id, neuron.uid),
                            NeuronInfo {
                                uid: neuron.uid,
                                address: format!("0x{}", hex::encode(neuron.hotkey)),
                                subnet_id: neuron.subnet_id,
                                stake: neuron.stake,
                                trust: trust_val,
                                // Consensus is derived from trust after Yuma consensus.
                                // NeuronData currently stores trust only; when the consensus
                                // engine produces a separate consensus score, pipe it through.
                                consensus: trust_val,
                                rank: neuron.rank as u64,
                                incentive: neuron.incentive as f64 / 65535.0,
                                dividends: neuron.dividends as f64 / 65535.0,
                                active: neuron.active,
                                endpoint: Some(neuron.endpoint),
                            },
                        );
                    }
                }
            }
        }
        cache
    }

    /// Get MetagraphDB reference (for external persistence)
    pub fn metagraph(&self) -> Arc<MetagraphDB> {
        self.metagraph.clone()
    }

    /// Start the RPC server on the given address
    ///
    /// # Arguments
    /// * `addr` - Address to bind (e.g. "127.0.0.1:8545")
    /// * `threads` - Number of worker threads for the HTTP server
    /// * `cors_origins` - CORS allowed origins (e.g. ["http://localhost:*"])
    pub fn start(self, addr: &str, threads: usize, cors_origins: &[String]) -> Result<Server> {
        let mut io = IoHandler::new();

        // Register blockchain query methods
        self.register_blockchain_methods(&mut io);

        // Register account methods
        self.register_account_methods(&mut io);

        // Register modular handlers (with DB persistence)
        register_staking_handlers(
            &mut io,
            self.validators.clone(),
            self.db.clone(),
            self.chain_id(),
            self.unified_state.clone(),
        );
        register_subnet_handlers(&mut io, self.subnets.clone(), self.db.clone());
        register_neuron_handlers(
            &mut io,
            self.neurons.clone(),
            self.subnets.clone(),
            self.db.clone(),
        );
        register_weight_handlers(&mut io, self.weights.clone(), self.db.clone());

        // Register checkpoint handlers for fast sync
        register_checkpoint_handlers(&mut io, self.db.clone(), self.data_dir.clone());

        // Register AI-specific methods (refactored to ai_rpc module)
        let ai_ctx = AiRpcContext::new(
            self.ai_tasks.clone(),
            self.validators.clone(),
            self.neurons.clone(),
            self.subnets.clone(),
        );
        register_ai_methods_new(&ai_ctx, &mut io);

        // Register SDK query methods (query_*) - refactored to query_rpc module
        let query_ctx = QueryRpcContext::new(
            self.neurons.clone(),
            self.subnets.clone(),
            self.validators.clone(),
            self.commit_reveal.clone(),
        );
        register_query_methods_new(&query_ctx, &mut io);

        // Register AI layer circuit breaker status endpoint
        let ai_cb = self.ai_circuit_breaker.clone();
        io.add_sync_method("system_getAICircuitBreakerStatus", move |_params: Params| {
            let status = ai_cb.summary();
            Ok(serde_json::json!({
                "healthy": status.healthy,
                "weight_consensus": {
                    "state": format!("{:?}", status.weight_consensus_state),
                    "operational": status.weight_consensus_state == luxtensor_consensus::CircuitState::Closed
                },
                "commit_reveal": {
                    "state": format!("{:?}", status.commit_reveal_state),
                    "operational": status.commit_reveal_state == luxtensor_consensus::CircuitState::Closed
                },
                "emission": {
                    "state": format!("{:?}", status.emission_state),
                    "operational": status.emission_state == luxtensor_consensus::CircuitState::Closed
                }
            }))
        });

        // Register rate limiter status endpoint for monitoring
        let _rl = self.rate_limiter.clone();
        io.add_sync_method("system_getRateLimitStatus", move |_params: Params| {
            Ok(serde_json::json!({
                "enabled": true,
                "config": {
                    "max_requests_per_minute": 100,
                    "window_seconds": 60
                },
                "message": "Rate limiting active for DoS protection"
            }))
        });

        // system_health - Return node health status (for monitoring and load balancers)
        let db_for_health = self.db.clone();
        let unified_for_health = self.unified_state.clone(); // C1 Phase 2B: Use unified_state
        io.add_sync_method("system_health", move |_params: Params| {
            // Get current block height
            let block_height = {
                let mut ceiling: u64 = 1;
                // Jump search for ceiling
                loop {
                    match db_for_health.get_block_by_height(ceiling) {
                        Ok(Some(_)) => {
                            ceiling *= 2;
                            if ceiling > 1_000_000 {
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                // Binary search for exact height
                let mut low = ceiling / 2;
                let mut high = ceiling;
                while low < high {
                    let mid = (low + high + 1) / 2;
                    match db_for_health.get_block_by_height(mid) {
                        Ok(Some(_)) => low = mid,
                        Ok(None) => high = mid - 1,
                        Err(_) => break,
                    }
                }
                low
            };

            let chain_id = unified_for_health.read().chain_id();

            Ok(serde_json::json!({
                "is_syncing": false,
                "block": block_height,
                "healthy": true,
                "chain_id": chain_id,
                "version": "0.1.0",
                "node_name": "luxtensor-node"
            }))
        });

        // system_cacheStats - Return Merkle cache statistics for monitoring
        if let Some(ref cache) = self.merkle_cache {
            let cache_for_stats = cache.clone();
            io.add_sync_method("system_cacheStats", move |_params: Params| {
                let stats = cache_for_stats.stats();
                Ok(serde_json::json!({
                    "full_computations": stats.full_computations,
                    "incremental_computations": stats.incremental_computations,
                    "root_cache_hits": stats.root_cache_hits,
                    "root_cache_misses": stats.root_cache_misses,
                    "hash_cache_hits": stats.hash_cache_hits,
                    "hit_ratio": stats.hit_ratio(),
                    "incremental_ratio": stats.incremental_ratio()
                }))
            });
        }

        // sync_getSyncStatus - Return current sync status for state sync protocol
        let db_for_sync = self.db.clone();
        let unified_for_sync = self.unified_state.clone(); // C1 Phase 2B: Use unified_state
        io.add_sync_method("sync_getSyncStatus", move |_params: Params| {
            let current_block = unified_for_sync.read().block_number();
            let highest_block = {
                // Simple linear scan from current to find highest
                let mut highest = current_block;
                for h in (current_block + 1)..(current_block + 100) {
                    if db_for_sync.get_block_by_height(h).ok().flatten().is_some() {
                        highest = h;
                    } else {
                        break;
                    }
                }
                highest
            };
            let is_syncing = highest_block > current_block;

            Ok(json!({
                "syncing": is_syncing,
                "currentBlock": format!("0x{:x}", current_block),
                "highestBlock": format!("0x{:x}", highest_block),
                "startingBlock": "0x0",
                "progress": if highest_block > 0 {
                    (current_block as f64 / highest_block as f64 * 100.0).min(100.0)
                } else {
                    100.0
                }
            }))
        });

        // Register Ethereum-compatible methods (eth_*)
        // Uses mempool for pending txs, unified_state for state reads, db for confirmed tx lookup
        // evm_executor for eth_call storage reads (shared EVM state from block execution)
        register_eth_methods(
            &mut io,
            self.mempool.clone(),
            self.unified_state.clone(),
            self.db.clone(),
            self.broadcaster.clone(),
            self.evm_executor.clone(),
        );

        // Register log query methods (eth_getLogs, eth_newFilter, etc.)
        let log_store = Arc::new(RwLock::new(LogStore::new(10_000))); // Keep logs for last 10K blocks
        register_log_methods(&mut io, log_store, self.unified_state.clone());

        // Register ERC-4337 Account Abstraction methods (eth_sendUserOperation, etc.)
        let entry_point =
            Arc::new(RwLock::new(luxtensor_contracts::EntryPoint::new(self.chain_id())));
        register_aa_methods(&mut io, entry_point);

        // Register transaction methods with P2P broadcasting (eth_sendTransaction, eth_getTransactionReceipt)
        // These override the base eth_rpc implementations with broadcast support
        // [C1 FIX] Uses unified_state for consistent nonce reads
        let tx_ctx = TxRpcContext::new(
            self.mempool.clone(),
            self.pending_txs.clone(),
            self.unified_state.clone(), // UNIFIED: consistent with eth_* handlers
            self.broadcaster.clone(),
            self.db.clone(),
        );
        register_tx_methods(&tx_ctx, &mut io);

        // Register Agent RPC methods (agent_*) — Agentic EVM
        if let Some(ref registry) = self.agent_registry {
            let agent_ctx = AgentRpcContext::new(registry.clone());
            register_agent_methods_new(&agent_ctx, &mut io);
        }

        // Register Dispute RPC methods (dispute_*) — Optimistic AI
        if let Some(ref dm) = self.dispute_manager {
            let dispute_ctx = DisputeRpcContext::new(dm.clone());
            register_dispute_methods_new(&dispute_ctx, &mut io);
        }

        // Register Bridge RPC methods (bridge_*) — Cross-Chain Asset Transfers
        if let Some(ref bridge) = self.bridge {
            let bridge_ctx = BridgeRpcContext::new(bridge.clone());
            register_bridge_methods(&bridge_ctx, &mut io);
        }

        // Register Multisig RPC methods (multisig_*) — Multi-Signature Wallets
        if let Some(ref mm) = self.multisig_manager {
            let multisig_ctx = MultisigRpcContext::new(mm.clone());
            register_multisig_methods(&multisig_ctx, &mut io);
        }

        // Start HTTP server with optimized settings
        let thread_count = if threads > 0 { threads } else { 4 };
        let mut builder =
            ServerBuilder::new(io).threads(thread_count).max_request_body_size(2 * 1024 * 1024); // 2 MB max request (reduced from 16 MB)

        // Apply CORS origins from config
        if !cors_origins.is_empty() {
            builder = builder.cors(jsonrpc_http_server::DomainsValidation::AllowOnly(
                cors_origins
                    .iter()
                    .map(|s| jsonrpc_http_server::AccessControlAllowOrigin::Value(s.clone().into()))
                    .collect(),
            ));
        }

        // SECURITY: Apply rate limiter middleware for DoS protection.
        // The RateLimiter was previously created but never called — this fixes that.
        let rate_limiter_mw = self.rate_limiter.clone();
        builder = builder.request_middleware(
            move |request: jsonrpc_http_server::hyper::Request<jsonrpc_http_server::hyper::Body>| {
                // Extract client IP from standard reverse-proxy headers, fallback to 0.0.0.0
                let ip = request
                    .headers()
                    .get("x-forwarded-for")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.split(',').next())
                    .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok())
                    .or_else(|| {
                        request
                            .headers()
                            .get("x-real-ip")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<std::net::IpAddr>().ok())
                    })
                    .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

                if !rate_limiter_mw.check(ip) {
                    warn!("Rate limited RPC request from {}", ip);
                    let body = r#"{"jsonrpc":"2.0","error":{"code":-32000,"message":"Rate limited: too many requests"},"id":null}"#;
                    jsonrpc_http_server::RequestMiddlewareAction::Respond {
                        should_validate_hosts: false,
                        response: Box::pin(async move {
                            Ok(jsonrpc_http_server::hyper::Response::builder()
                                .status(429)
                                .header("content-type", "application/json")
                                .body(jsonrpc_http_server::hyper::Body::from(body))
                                .expect("static response"))
                        }),
                    }
                } else {
                    jsonrpc_http_server::RequestMiddlewareAction::Proceed {
                        should_continue_on_invalid_cors: false,
                        request,
                    }
                }
            },
        );

        let server = builder
            .start_http(
                &addr
                    .parse()
                    .map_err(|e: std::net::AddrParseError| RpcError::ServerError(e.to_string()))?,
            )
            .map_err(|e| RpcError::ServerError(e.to_string()))?;

        Ok(server)
    }

    /// Register blockchain query methods
    fn register_blockchain_methods(&self, io: &mut IoHandler) {
        // eth_blockNumber - Get current block height (OPTIMIZED: atomic with proper ordering)
        let cached_block_num = self.cached_block_number.clone();
        let unified_for_block_num = self.unified_state.clone();
        let db_for_block_num = self.db.clone();
        io.add_sync_method("eth_blockNumber", move |_params: Params| {
            // Get block number from UnifiedStateDB first (source of truth)
            let unified_block = unified_for_block_num.read().block_number();
            if unified_block > 0 {
                // Update cache atomically with Release ordering for visibility
                cached_block_num.store(unified_block, Ordering::Release);
                return Ok(Value::String(format!("0x{:x}", unified_block)));
            }

            // Fallback: Check atomic cache (with Acquire for proper visibility)
            let cached = cached_block_num.load(Ordering::Acquire);
            if cached > 0 {
                return Ok(Value::String(format!("0x{:x}", cached)));
            }

            // SLOW PATH: Initialize from DB (only at startup)
            // Check genesis first
            match db_for_block_num.get_block_by_height(0) {
                Ok(None) => return Ok(Value::String("0x0".to_string())),
                Err(_) => return Err(jsonrpc_core::Error::internal_error()),
                Ok(Some(_)) => {}
            }

            // Jump search to find ceiling
            let mut ceiling: u64 = 1;
            loop {
                match db_for_block_num.get_block_by_height(ceiling) {
                    Ok(Some(_)) => {
                        ceiling *= 2;
                        if ceiling > 1_000_000 {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => return Err(jsonrpc_core::Error::internal_error()),
                }
            }

            // Binary search for exact height
            let mut low = ceiling / 2;
            let mut high = ceiling;
            while low < high {
                let mid = (low + high + 1) / 2;
                match db_for_block_num.get_block_by_height(mid) {
                    Ok(Some(_)) => low = mid,
                    Ok(None) => high = mid - 1,
                    Err(_) => return Err(jsonrpc_core::Error::internal_error()),
                }
            }

            // Cache the result
            cached_block_num.store(low, Ordering::Relaxed);
            Ok(Value::String(format!("0x{:x}", low)))
        });

        // eth_getBlockByNumber - Get block by number
        let db_for_get_block = self.db.clone();
        let cached_for_get_block = self.cached_block_number.clone();
        let unified_for_get_block = self.unified_state.clone();
        io.add_sync_method("eth_getBlockByNumber", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing block number"));
            }

            // Resolve "latest"/"pending" to the actual chain tip height
            let latest = {
                let ub = unified_for_get_block.read().block_number();
                if ub > 0 {
                    ub
                } else {
                    cached_for_get_block.load(Ordering::Acquire)
                }
            };
            let height = parse_block_number_with_latest(&parsed[0], latest)?;
            let _include_txs = parsed.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

            match db_for_get_block.get_block_by_height(height) {
                Ok(Some(block)) => {
                    let rpc_block = RpcBlock::from(block);
                    serde_json::to_value(rpc_block)
                        .map_err(|_| jsonrpc_core::Error::internal_error())
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });

        let db = self.db.clone();

        // eth_getBlockByHash - Get block by hash
        io.add_sync_method("eth_getBlockByHash", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing block hash"));
            }

            let hash_str = parsed[0].trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_str)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hash format"))?;

            if hash_bytes.len() != 32 {
                return Err(jsonrpc_core::Error::invalid_params("Hash must be 32 bytes"));
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);

            match db.get_block(&hash) {
                Ok(Some(block)) => {
                    let rpc_block = RpcBlock::from(block);
                    serde_json::to_value(rpc_block)
                        .map_err(|_| jsonrpc_core::Error::internal_error())
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });

        let db = self.db.clone();
        let pending_txs_query = self.pending_txs.clone();

        // eth_getTransactionByHash - Get transaction by hash
        // Checks pending transactions first, then confirmed transactions in DB
        io.add_sync_method("eth_getTransactionByHash", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing transaction hash"));
            }

            let hash_str = parsed[0].trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_str)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hash format"))?;

            if hash_bytes.len() != 32 {
                return Err(jsonrpc_core::Error::invalid_params("Hash must be 32 bytes"));
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);

            // 1. Check pending transactions first (in-memory mempool)
            if let Some(tx) = pending_txs_query.get(&hash) {
                let rpc_tx = RpcTransaction::from(tx.value().clone());
                return serde_json::to_value(rpc_tx)
                    .map_err(|_| jsonrpc_core::Error::internal_error());
            }

            // 2. Fallback to confirmed transactions in database
            match db.get_transaction(&hash) {
                Ok(Some(tx)) => {
                    let rpc_tx = RpcTransaction::from(tx);
                    serde_json::to_value(rpc_tx).map_err(|_| jsonrpc_core::Error::internal_error())
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });
    }

    /// Register account methods
    fn register_account_methods(&self, io: &mut IoHandler) {
        let unified_state = self.unified_state.clone();

        // eth_getBalance - Get account balance
        io.add_sync_method("eth_getBalance", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;

            let balance = unified_state.read().get_balance(&address);
            Ok(Value::String(format!("0x{:x}", balance)))
        });

        // NOTE: eth_getTransactionCount and eth_sendRawTransaction are registered
        // in eth_rpc::register_eth_methods() with proper RLP decoding.
        // Do NOT duplicate them here — the eth_rpc versions are canonical.

        // tx_getReceipt - Get transaction receipt
        let db = self.db.clone();

        io.add_sync_method("tx_getReceipt", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing transaction hash"));
            }

            let hash_str = parsed[0].trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_str)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hash format"))?;

            if hash_bytes.len() != 32 {
                return Err(jsonrpc_core::Error::invalid_params("Hash must be 32 bytes"));
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);

            // Query transaction from database
            match db.get_transaction(&hash) {
                Ok(Some(tx)) => {
                    // Try to get the real receipt from DB (bincode-serialized)
                    let (status, gas_used, logs_json, contract_addr, block_height) = match db.get_receipt(&hash) {
                        Ok(Some(receipt_bytes)) => {
                            match bincode::deserialize::<luxtensor_core::receipt::Receipt>(&receipt_bytes) {
                                Ok(r) => {
                                    let status_hex = match r.status {
                                        luxtensor_core::receipt::ExecutionStatus::Success => "0x1",
                                        luxtensor_core::receipt::ExecutionStatus::Failed => "0x0",
                                    };
                                    let logs: Vec<serde_json::Value> = r.logs.iter().map(|log| {
                                        serde_json::json!({
                                            "address": format!("0x{}", hex::encode(log.address.as_bytes())),
                                            "topics": log.topics.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
                                            "data": format!("0x{}", hex::encode(&log.data)),
                                        })
                                    }).collect();
                                    let ca = r.contract_address.map(|a| format!("0x{}", hex::encode(a.as_bytes())));
                                    (status_hex.to_string(), r.gas_used, serde_json::json!(logs), ca, r.block_height)
                                }
                                Err(_) => ("0x1".to_string(), 21000u64, serde_json::json!([]), None, 0u64),
                            }
                        }
                        _ => ("0x1".to_string(), 21000u64, serde_json::json!([]), None, 0u64),
                    };

                    let receipt = serde_json::json!({
                        "transactionHash": format!("0x{}", hex::encode(hash)),
                        "status": status,
                        "blockNumber": format!("0x{:x}", block_height),
                        "gasUsed": format!("0x{:x}", gas_used),
                        "cumulativeGasUsed": format!("0x{:x}", gas_used),
                        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                        "to": tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
                        "logs": logs_json,
                        "contractAddress": contract_addr,
                    });
                    Ok(receipt)
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });

        // NOTE: dev_faucet is registered in eth_rpc.rs register_eth_methods()
        // which updates EvmState.balances - the source queried by eth_getBalance
    }
}

// ─── M-3 FIX: Builder Pattern ────────────────────────────────────────

/// Builder for constructing `RpcServer` with composable configuration.
///
/// Required: `db` and `chain_id` (set via `RpcServer::builder()`).
/// Optional: `metagraph`, `broadcaster`, `mempool`, `pending_txs`,
///           `validators`, `data_dir`.
pub struct RpcServerBuilder {
    db: Arc<BlockchainDB>,
    chain_id: u64,
    metagraph: Option<Arc<MetagraphDB>>,
    broadcaster: Option<Arc<dyn TransactionBroadcaster>>,
    mempool: Option<Arc<RwLock<Mempool>>>,
    pending_txs: Option<Arc<DashMap<Hash, Transaction>>>,
    validators: Option<Arc<RwLock<ValidatorSet>>>,
    data_dir: Option<PathBuf>,
}

impl RpcServerBuilder {
    /// Set persistent MetagraphDB (defaults to temp dir if not provided).
    pub fn metagraph(mut self, m: Arc<MetagraphDB>) -> Self {
        self.metagraph = Some(m);
        self
    }

    /// Set transaction broadcaster (defaults to `NoOpBroadcaster`).
    pub fn broadcaster(mut self, b: Arc<dyn TransactionBroadcaster>) -> Self {
        self.broadcaster = Some(b);
        self
    }

    /// Set external mempool (defaults to a new empty mempool).
    pub fn mempool(mut self, m: Arc<RwLock<Mempool>>) -> Self {
        self.mempool = Some(m);
        self
    }

    /// Set shared pending transactions map (defaults to a new empty map).
    pub fn pending_txs(mut self, p: Arc<DashMap<Hash, Transaction>>) -> Self {
        self.pending_txs = Some(p);
        self
    }

    /// Set validator set (defaults to an empty set).
    pub fn validators(mut self, v: Arc<RwLock<ValidatorSet>>) -> Self {
        self.validators = Some(v);
        self
    }

    /// Set data directory for checkpoints (defaults to `./data`).
    pub fn data_dir(mut self, d: PathBuf) -> Self {
        self.data_dir = Some(d);
        self
    }

    /// Build the `RpcServer`.
    ///
    /// Creates a temp MetagraphDB if none was provided.
    pub fn build(self) -> RpcServer {
        let metagraph = self.metagraph.unwrap_or_else(|| {
            let temp_dir =
                std::env::temp_dir().join(format!("luxtensor_{}", std::process::id()));
            Arc::new(MetagraphDB::open(&temp_dir).unwrap_or_else(|e| {
                tracing::error!("MetagraphDB::open failed: {} — trying fallback", e);
                let fallback =
                    std::env::temp_dir().join(format!("luxtensor_fb_{}", std::process::id()));
                MetagraphDB::open(&fallback).unwrap_or_else(|e2| {
                    tracing::error!("FATAL: MetagraphDB fallback also failed: {}", e2);
                    std::process::exit(1);
                })
            }))
        });

        let subnets = Arc::new(DashMap::from_iter(RpcServer::load_subnets_cache(&metagraph)));
        let neurons = Arc::new(DashMap::from_iter(RpcServer::load_neurons_cache(&metagraph)));
        let data_dir = self.data_dir.unwrap_or_else(|| PathBuf::from("./data"));

        RpcServer {
            db: self.db,
            validators: self.validators.unwrap_or_else(|| Arc::new(RwLock::new(ValidatorSet::new()))),
            metagraph,
            subnets,
            neurons,
            weights: Arc::new(DashMap::new()),
            pending_txs: self.pending_txs.unwrap_or_else(|| Arc::new(DashMap::new())),
            ai_tasks: Arc::new(DashMap::new()),
            broadcaster: self.broadcaster.unwrap_or_else(|| Arc::new(NoOpBroadcaster)),
            mempool: self.mempool.unwrap_or_else(|| Arc::new(RwLock::new(Mempool::new()))),
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(
                CommitRevealConfig::default(),
            ))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            rate_limiter: Arc::new(RateLimiter::new()),
            data_dir,
            cached_block_number: Arc::new(AtomicU64::new(0)),
            cached_chain_id: Arc::new(AtomicU64::new(self.chain_id)),
            unified_state: Arc::new(RwLock::new(UnifiedStateDB::new(self.chain_id))),
            agent_registry: None,
            dispute_manager: None,
            bridge: None,
            multisig_manager: None,
            merkle_cache: None,
            evm_executor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::{Address, Block, BlockHeader, Transaction};
    use luxtensor_storage::BlockchainDB;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_setup() -> (TempDir, Arc<BlockchainDB>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("blockchain");

        let blockchain_db = Arc::new(BlockchainDB::open(&db_path).unwrap());

        (temp_dir, blockchain_db)
    }

    #[test]
    fn test_rpc_server_creation() {
        let (_temp, db) = create_test_setup();
        let _server = RpcServer::new_for_testing(db);
    }

    #[test]
    fn test_parse_block_number() {
        let value = serde_json::json!("0x10");
        assert_eq!(parse_block_number(&value).unwrap(), 16);

        let value = serde_json::json!("latest");
        assert!(parse_block_number(&value).is_ok());

        let value = serde_json::json!(42);
        assert_eq!(parse_block_number(&value).unwrap(), 42);
    }

    #[test]
    fn test_parse_address() {
        let addr = "0x1234567890123456789012345678901234567890";
        let parsed = parse_address(addr).unwrap();
        assert_eq!(parsed.as_bytes().len(), 20);
    }

    #[test]
    fn test_parse_address_invalid() {
        let addr = "0x123"; // Too short
        assert!(parse_address(addr).is_err());

        let addr = "0xzzzz"; // Invalid hex
        assert!(parse_address(addr).is_err());
    }

    #[test]
    fn test_rpc_block_conversion() {
        let block = Block {
            header: BlockHeader {
                version: 1,
                height: 100,
                timestamp: 1000,
                previous_hash: [0u8; 32],
                state_root: [1u8; 32],
                txs_root: [2u8; 32],
                receipts_root: [3u8; 32],
                validator: [4u8; 32],
                signature: vec![0u8; 64],
                gas_used: 21000,
                gas_limit: 1000000,
                extra_data: vec![],
            },
            transactions: vec![],
        };

        let rpc_block = RpcBlock::from(block);
        assert_eq!(rpc_block.number, "0x64");
        assert_eq!(rpc_block.gas_used, "0x5208");
    }

    #[test]
    fn test_rpc_transaction_conversion() {
        let tx =
            Transaction::new(1, Address::zero(), Some(Address::zero()), 1000, 1, 21000, vec![]);

        let rpc_tx = RpcTransaction::from(tx);
        assert_eq!(rpc_tx.nonce, "0x1");
        assert_eq!(rpc_tx.value, "0x3e8");
    }
}
