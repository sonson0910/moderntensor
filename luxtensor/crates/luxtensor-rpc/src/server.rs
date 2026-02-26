use crate::blockchain_rpc::{self, BlockchainRpcContext};
use crate::logs::LogStore;
use crate::rate_limiter::RateLimiter;
use crate::system_rpc::{self, SystemRpcContext};
use crate::{
    eth_rpc::{register_aa_methods, register_eth_methods, register_log_methods, FaucetRpcConfig},
    rpc_cache::RpcStateCache,
    types::*,
    NoOpBroadcaster, Result, RpcError, TransactionBroadcaster,
};
use jsonrpc_core::IoHandler;
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_consensus::{
    AILayerCircuitBreaker, CommitRevealConfig, CommitRevealManager, ValidatorSet,
};
use luxtensor_core::{Hash, Transaction, UnifiedStateDB};
use luxtensor_storage::{BlockchainDB, CachedStateDB, MetagraphDB};
use dashmap::DashMap;
use parking_lot::RwLock;

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::ai_rpc::{register_ai_methods as register_ai_methods_new, AiRpcContext};
use crate::handlers::{
    register_admin_epoch_handler, register_checkpoint_handlers, register_metagraph_methods,
    register_neuron_handlers, register_staking_handlers, register_subnet_handlers,
    register_weight_handlers, register_debug_metagraph_handler,
};
#[cfg(test)]
use crate::helpers::parse_address;
#[cfg(test)]
use crate::helpers::parse_block_number;
use crate::query_rpc::{register_query_methods as register_query_methods_new, QueryRpcContext};
use crate::tx_rpc::{register_tx_methods, TxRpcContext};
use crate::agent_rpc::{register_agent_methods as register_agent_methods_new, AgentRpcContext};
use crate::dispute_rpc::{register_dispute_methods as register_dispute_methods_new, DisputeRpcContext};
use crate::bridge_rpc::{register_bridge_methods, BridgeRpcContext};
use crate::multisig_rpc::{register_multisig_methods, MultisigRpcContext};
use crate::miner_dispatch_rpc::{MinerDispatchContext, register_miner_dispatch_methods};
use crate::rewards_rpc::register_reward_methods;
use luxtensor_consensus::RewardExecutor;
use luxtensor_contracts::AgentRegistry;
use luxtensor_core::bridge::PersistentBridge;
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
/// let server = RpcServer::new_with_shared_pending_txs(db, mempool, broadcaster, pending_txs, chain_id);
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
    mempool: Arc<luxtensor_core::UnifiedMempool>,
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
    bridge: Option<Arc<PersistentBridge>>,
    /// Multisig wallet manager for multi-signature transactions
    multisig_manager: Option<Arc<MultisigManager>>,
    /// Merkle root cache for state root caching stats (optional)
    merkle_cache: Option<Arc<CachedStateDB>>,
    /// Shared EVM executor from block execution for eth_call storage reads
    evm_executor: Option<luxtensor_contracts::EvmExecutor>,
    /// Callback returning node metrics as JSON (from NodeMetrics::to_json)
    metrics_json_fn: Option<Arc<dyn Fn() -> serde_json::Value + Send + Sync>>,
    /// Callback returning Prometheus metrics text (from NodeMetrics::export)
    metrics_prometheus_fn: Option<Arc<dyn Fn() -> String + Send + Sync>>,
    /// Callback returning health status as JSON (from HealthMonitor::get_health)
    health_fn: Option<Arc<dyn Fn() -> serde_json::Value + Send + Sync>>,
    /// Reward executor for rewards_* RPC methods (optional, shared with block production)
    reward_executor: Option<Arc<parking_lot::RwLock<RewardExecutor>>>,
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

    /// Create a new RPC server for testing (uses temp storage)
    #[cfg(test)]
    pub fn new_for_testing(db: Arc<BlockchainDB>) -> Self {
        let chain_id = 8898_u64; // LuxTensor devnet chain_id
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_test_{}", std::process::id()));
        let metagraph =
            Arc::new(MetagraphDB::open(&temp_dir).expect("Failed to create test MetagraphDB"));
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
            broadcaster: Arc::new(NoOpBroadcaster),
            mempool: Arc::new(luxtensor_core::UnifiedMempool::new(1000, 8898)),
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
            metrics_json_fn: None,
            metrics_prometheus_fn: None,
            health_fn: None,
            reward_executor: None,
        }
    }

    /// Get mempool reference for block production polling
    pub fn mempool(&self) -> Arc<luxtensor_core::UnifiedMempool> {
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

    /// Replace the internal UnifiedStateDB with an externally-created instance.
    /// This allows P2P handler and block production to share the same UnifiedStateDB
    /// so that `sync_from_state_db` calls from either path update the RPC layer.
    pub fn set_unified_state(&mut self, state: Arc<RwLock<UnifiedStateDB>>) {
        self.unified_state = state;
    }

    /// Set the cross-chain bridge instance (optional, enables bridge_* RPC methods)
    pub fn set_bridge(&mut self, bridge: Arc<PersistentBridge>) {
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

    /// Set NodeMetrics provider callbacks for system_metrics / system_prometheusMetrics RPCs
    pub fn set_metrics_provider(
        &mut self,
        json_fn: Arc<dyn Fn() -> serde_json::Value + Send + Sync>,
        prometheus_fn: Arc<dyn Fn() -> String + Send + Sync>,
    ) {
        self.metrics_json_fn = Some(json_fn);
        self.metrics_prometheus_fn = Some(prometheus_fn);
    }

    /// Set HealthMonitor provider callback for enhanced system_health RPC
    pub fn set_health_provider(
        &mut self,
        health_fn: Arc<dyn Fn() -> serde_json::Value + Send + Sync>,
    ) {
        self.health_fn = Some(health_fn);
    }

    /// Set RewardExecutor — enables rewards_getPending, rewards_claim, rewards_getStats etc.
    /// Should be the SAME Arc shared with block_production so state is consistent.
    pub fn set_reward_executor(&mut self, executor: Arc<parking_lot::RwLock<RewardExecutor>>) {
        self.reward_executor = Some(executor);
    }

    /// Inject shared MetagraphDB from NodeService so staking_registerValidator,
    /// neuron_register etc. write into the SAME DB that Yuma consensus reads from.
    /// MUST be called before start() to fix the split-DB bug.
    pub fn set_metagraph(&mut self, metagraph: Arc<MetagraphDB>) {
        self.metagraph = metagraph;
    }





    /// Create a new RPC server with external shared pending_txs for unified storage
    /// Use this when you need P2P handlers to share the same TX pool as RPC
    /// 🔧 FIX: Added chain_id parameter — was hardcoded to 1337
    pub fn new_with_shared_pending_txs(
        db: Arc<BlockchainDB>,
        mempool: Arc<luxtensor_core::UnifiedMempool>,
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
            metrics_json_fn: None,
            metrics_prometheus_fn: None,
            health_fn: None,
            reward_executor: None,
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
                                hotkey: format!("0x{}", hex::encode(neuron.hotkey)),
                                coldkey: format!("0x{}", hex::encode(neuron.coldkey)),
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
                                emission: neuron.emission,
                                last_update: neuron.last_update,
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
    ///
    /// # Architecture
    /// Registration is split into 4 phases for maintainability:
    /// 1. Core methods — blockchain, system, staking, subnet, neuron, weight, AI, query
    /// 2. ETH-compatible methods — eth_*, net_*, web3_*, logs, AA, transactions
    /// 3. Optional modules — agent, dispute, bridge, multisig (if configured)
    /// 4. HTTP server — rate limiter middleware, CORS, bind address
    pub fn start(self, addr: &str, threads: usize, cors_origins: &[String]) -> Result<Server> {
        let mut io = IoHandler::new();

        // Phase 1: Core RPC methods (blockchain, system, handlers, AI, query)
        self.register_core_methods(&mut io);

        // Phase 2: Ethereum-compatible methods (eth_*, logs, AA, tx)
        self.register_eth_methods_all(&mut io);

        // Phase 3: Optional feature modules (agent, dispute, bridge, multisig)
        self.register_optional_modules(&mut io);

        // Phase 4: Build and start HTTP server with rate limiting + CORS
        self.build_http_server(io, addr, threads, cors_origins)
    }

    /// Phase 1: Register core blockchain, system, and domain-specific RPC methods.
    ///
    /// Includes: blockchain queries, system/monitoring, staking, subnet, neuron,
    /// weight, rewards, metagraph, admin, miner dispatch, checkpoint, AI, and query methods.
    fn register_core_methods(&self, io: &mut IoHandler) {
        // ── Blockchain & Account query methods (extracted to blockchain_rpc) ──
        let blockchain_ctx = BlockchainRpcContext {
            db: self.db.clone(),
            unified_state: self.unified_state.clone(),
            cached_block_number: self.cached_block_number.clone(),
            pending_txs: self.pending_txs.clone(),
            mempool: self.mempool.clone(),
        };
        blockchain_rpc::register_blockchain_query_methods(&blockchain_ctx, io);
        blockchain_rpc::register_account_query_methods(
            self.unified_state.clone(),
            self.db.clone(),
            io,
        );

        // ── System, monitoring, debug, and sync methods (extracted to system_rpc) ──
        let system_ctx = SystemRpcContext {
            db: self.db.clone(),
            unified_state: self.unified_state.clone(),
            mempool: self.mempool.clone(),
            validators: self.validators.clone(),
            chain_id: self.chain_id(),
            ai_circuit_breaker: self.ai_circuit_breaker.clone(),
            rate_limiter: self.rate_limiter.clone(),
            merkle_cache: self.merkle_cache.clone(),
            metrics_json_fn: self.metrics_json_fn.clone(),
            metrics_prometheus_fn: self.metrics_prometheus_fn.clone(),
            health_fn: self.health_fn.clone(),
        };
        system_rpc::register_system_methods(&system_ctx, io);
        system_rpc::register_monitoring_methods(&system_ctx, io);

        // ── Modular handlers (with DB persistence) ──
        register_staking_handlers(
            io,
            self.validators.clone(),
            self.db.clone(),
            self.chain_id(),
            self.unified_state.clone(),
            self.metagraph.clone(),
            self.mempool.clone(),
        );
        register_subnet_handlers(io, self.subnets.clone(), self.db.clone(), self.metagraph.clone(), self.mempool.clone());
        register_neuron_handlers(
            io,
            self.neurons.clone(),
            self.subnets.clone(),
            self.db.clone(),
            self.metagraph.clone(),
            self.mempool.clone(),
        );
        register_weight_handlers(io, self.weights.clone(), self.db.clone(), self.metagraph.clone());

        // Register rewards_* RPC methods if RewardExecutor is provided
        if let Some(reward_exec) = self.reward_executor.clone() {
            register_reward_methods(io, reward_exec);
        }

        // Register lux_* metagraph methods backed by MetagraphDB (RocksDB)
        register_metagraph_methods(io, self.metagraph.clone());

        // Register admin_runEpoch — triggers YumaConsensus manually (for testing/debug)
        register_admin_epoch_handler(io, self.metagraph.clone());

        // Register admin_debugMetagraph — dumps MetagraphDB state for debugging
        register_debug_metagraph_handler(io, self.metagraph.clone());

        // Register miner dispatch methods (lux_registerMiner, lux_listMiners, etc.)
        let miner_ctx = Arc::new(MinerDispatchContext::new(self.metagraph.clone()));
        register_miner_dispatch_methods(miner_ctx, io);

        // Register checkpoint handlers for fast sync
        register_checkpoint_handlers(io, self.db.clone(), self.data_dir.clone());

        // Register AI-specific methods (refactored to ai_rpc module)
        let ai_ctx = AiRpcContext::new(
            self.ai_tasks.clone(),
            self.validators.clone(),
            self.neurons.clone(),
            self.subnets.clone(),
        );
        register_ai_methods_new(&ai_ctx, io);

        // Register SDK query methods (query_*) - refactored to query_rpc module
        let query_ctx = QueryRpcContext::new(
            self.neurons.clone(),
            self.subnets.clone(),
            self.validators.clone(),
            self.commit_reveal.clone(),
        );
        register_query_methods_new(&query_ctx, io);

        // Register rpc_listMethods — API introspection endpoint
        crate::api_registry::register_list_methods(io);
    }

    /// Phase 2: Register Ethereum-compatible RPC methods.
    ///
    /// Includes: eth_* (sendRawTransaction, call, getBalance, etc.), net_version,
    /// web3_clientVersion, faucet, log queries, ERC-4337 Account Abstraction,
    /// and transaction broadcasting methods.
    fn register_eth_methods_all(&self, io: &mut IoHandler) {
        // Create RpcStateCache for zero-lock hot-path RPC queries
        let initial_block = self.unified_state.read().block_number();
        let initial_base_fee = {
            use luxtensor_consensus::FeeMarket;
            FeeMarket::new().current_base_fee()
        };
        let rpc_cache = Arc::new(RpcStateCache::new(
            self.chain_id(),
            initial_block,
            initial_base_fee as u64,
        ));

        register_eth_methods(
            io,
            self.mempool.clone(),
            self.unified_state.clone(),
            self.db.clone(),
            self.broadcaster.clone(),
            self.evm_executor.clone(),
            FaucetRpcConfig::default(),
            rpc_cache.clone(),
        );

        // Register log query methods (eth_getLogs, eth_newFilter, etc.)
        let log_store = Arc::new(RwLock::new(LogStore::new(10_000)));
        register_log_methods(io, log_store, self.unified_state.clone(), rpc_cache.clone());

        // Register ERC-4337 Account Abstraction methods
        let entry_point =
            Arc::new(RwLock::new(luxtensor_contracts::EntryPoint::new(self.chain_id())));
        register_aa_methods(io, entry_point);

        // Register transaction methods with P2P broadcasting
        let tx_ctx = TxRpcContext::new(
            self.mempool.clone(),
            self.pending_txs.clone(),
            self.unified_state.clone(),
            self.broadcaster.clone(),
            self.db.clone(),
        );
        register_tx_methods(&tx_ctx, io);
    }

    /// Phase 3: Register optional feature modules (only if configured).
    ///
    /// These modules are conditionally enabled via `set_*` methods on RpcServer
    /// before calling `start()`. Includes: agent registry, dispute resolution,
    /// cross-chain bridge, and multisig wallet management.
    fn register_optional_modules(&self, io: &mut IoHandler) {
        if let Some(ref registry) = self.agent_registry {
            let agent_ctx = AgentRpcContext::new(registry.clone());
            register_agent_methods_new(&agent_ctx, io);
        }
        if let Some(ref dm) = self.dispute_manager {
            let dispute_ctx = DisputeRpcContext::new(dm.clone());
            register_dispute_methods_new(&dispute_ctx, io);
        }
        if let Some(ref bridge) = self.bridge {
            let bridge_ctx = BridgeRpcContext::new(bridge.clone());
            register_bridge_methods(&bridge_ctx, io);
        }
        if let Some(ref mm) = self.multisig_manager {
            let multisig_ctx = MultisigRpcContext::new(mm.clone());
            register_multisig_methods(&multisig_ctx, io);
        }
    }

    /// Phase 4: Build and start the HTTP server with rate limiting and CORS.
    ///
    /// Configures request middleware for IP-based rate limiting (X-Forwarded-For
    /// and X-Real-IP aware), CORS origins, thread pool, and max request body size.
    fn build_http_server(
        &self,
        io: IoHandler,
        addr: &str,
        threads: usize,
        cors_origins: &[String],
    ) -> Result<Server> {
        let thread_count = if threads > 0 { threads } else { 4 };
        let mut builder =
            ServerBuilder::new(io).threads(thread_count).max_request_body_size(2 * 1024 * 1024);

        if !cors_origins.is_empty() {
            builder = builder.cors(jsonrpc_http_server::DomainsValidation::AllowOnly(
                cors_origins
                    .iter()
                    .map(|s| jsonrpc_http_server::AccessControlAllowOrigin::Value(s.clone().into()))
                    .collect(),
            ));
        }

        // SECURITY: Apply rate limiter middleware for DoS protection.
        let rate_limiter_mw = self.rate_limiter.clone();
        builder = builder.request_middleware(
            move |request: jsonrpc_http_server::hyper::Request<jsonrpc_http_server::hyper::Body>| {
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

    // NOTE: register_blockchain_methods and register_account_methods have been
    // extracted to blockchain_rpc.rs for better separation of concerns.
    // They are now called from start() via blockchain_rpc::register_*.
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
    mempool: Option<Arc<luxtensor_core::UnifiedMempool>>,
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
    pub fn mempool(mut self, m: Arc<luxtensor_core::UnifiedMempool>) -> Self {
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
            mempool: self.mempool.unwrap_or_else(|| Arc::new(luxtensor_core::UnifiedMempool::new(1000, 8898))),
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
            metrics_json_fn: None,
            metrics_prometheus_fn: None,
            health_fn: None,
            reward_executor: None,
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
                vrf_proof: None,
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
