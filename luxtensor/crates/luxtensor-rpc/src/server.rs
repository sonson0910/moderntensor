use crate::{types::*, RpcError, Result, TransactionBroadcaster, NoOpBroadcaster, eth_rpc::{EvmState, register_eth_methods}};
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_core::{StateDB, Transaction, Hash};
use luxtensor_storage::{BlockchainDB, MetagraphDB};
use luxtensor_consensus::{ValidatorSet, CommitRevealManager, CommitRevealConfig, AILayerCircuitBreaker};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use crate::handlers::{
    register_subnet_handlers, register_neuron_handlers,
    register_staking_handlers, register_weight_handlers,
    register_checkpoint_handlers
};
use std::path::PathBuf;
use crate::helpers::{parse_address, parse_block_number};
use crate::query_rpc::{QueryRpcContext, register_query_methods as register_query_methods_new};
use crate::ai_rpc::{AiRpcContext, register_ai_methods as register_ai_methods_new};
use crate::tx_rpc::{TxRpcContext, register_tx_methods};

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
    state: Arc<RwLock<StateDB>>,
    validators: Arc<RwLock<ValidatorSet>>,
    // Persistent storage (RocksDB)
    metagraph: Arc<MetagraphDB>,
    // In-memory caches for fast access
    subnets: Arc<RwLock<HashMap<u64, SubnetInfo>>>,
    neurons: Arc<RwLock<HashMap<(u64, u64), NeuronInfo>>>,
    weights: Arc<RwLock<HashMap<(u64, u64), Vec<WeightInfo>>>>,
    pending_txs: Arc<RwLock<HashMap<Hash, Transaction>>>,
    ai_tasks: Arc<RwLock<HashMap<Hash, AITaskInfo>>>,
    broadcaster: Arc<dyn TransactionBroadcaster>,
    evm_state: Arc<RwLock<EvmState>>,
    commit_reveal: Arc<RwLock<CommitRevealManager>>,
    /// Circuit breaker for AI layer operations
    ai_circuit_breaker: Arc<AILayerCircuitBreaker>,
    /// Data directory for checkpoints and other persistent data
    data_dir: PathBuf,
}

impl RpcServer {
    /// Create a new RPC server with persistent MetagraphDB
    pub fn new(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        metagraph: Arc<MetagraphDB>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
    ) -> Self {
        // Load initial data from metagraph into caches
        let subnets = Arc::new(RwLock::new(Self::load_subnets_cache(&metagraph)));
        let neurons = Arc::new(RwLock::new(Self::load_neurons_cache(&metagraph)));

        Self {
            db,
            state,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets,
            neurons,
            weights: Arc::new(RwLock::new(HashMap::new())),
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
            ai_tasks: Arc::new(RwLock::new(HashMap::new())),
            broadcaster,
            evm_state: Arc::new(RwLock::new(EvmState::new(1337))), // Chain ID 1337
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(CommitRevealConfig::default()))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            data_dir: PathBuf::from("./data"), // Default data directory
        }
    }

    /// Create a new RPC server for testing (uses temp storage)
    pub fn new_for_testing(db: Arc<BlockchainDB>, state: Arc<RwLock<StateDB>>) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_test_{}", std::process::id()));
        let metagraph = Arc::new(
            MetagraphDB::open(&temp_dir).expect("Failed to create test MetagraphDB")
        );
        Self::new(db, state, metagraph, Arc::new(NoOpBroadcaster))
    }

    /// Get EVM state reference for block production polling
    pub fn evm_state(&self) -> Arc<RwLock<EvmState>> {
        self.evm_state.clone()
    }

    /// Get AI layer circuit breaker reference for monitoring
    pub fn ai_circuit_breaker(&self) -> Arc<AILayerCircuitBreaker> {
        self.ai_circuit_breaker.clone()
    }

    /// Create a new RPC server for testing with external EVM state
    pub fn new_for_testing_with_evm(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        evm_state: Arc<RwLock<EvmState>>,
    ) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_test_{}", std::process::id()));
        let metagraph = Arc::new(
            MetagraphDB::open(&temp_dir).expect("Failed to create test MetagraphDB")
        );

        Self {
            db,
            state,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
            ai_tasks: Arc::new(RwLock::new(HashMap::new())),
            broadcaster: Arc::new(NoOpBroadcaster),
            evm_state,
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(CommitRevealConfig::default()))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            data_dir: temp_dir,
        }
    }

    /// Create a new RPC server with external EVM state and P2P broadcaster
    /// Use this for production multi-node setup
    pub fn new_with_evm_and_broadcaster(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        evm_state: Arc<RwLock<EvmState>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
    ) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_{}", std::process::id()));
        let metagraph = Arc::new(
            MetagraphDB::open(&temp_dir).expect("Failed to create MetagraphDB")
        );

        Self {
            db,
            state,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
            ai_tasks: Arc::new(RwLock::new(HashMap::new())),
            broadcaster,
            evm_state,
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(CommitRevealConfig::default()))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            data_dir: temp_dir,
        }
    }

    /// Create a new RPC server with external shared pending_txs for unified storage
    /// Use this when you need P2P handlers to share the same TX pool as RPC
    pub fn new_with_shared_pending_txs(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        evm_state: Arc<RwLock<EvmState>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
        pending_txs: Arc<RwLock<HashMap<Hash, Transaction>>>,
    ) -> Self {
        let temp_dir = std::env::temp_dir().join(format!("luxtensor_{}", std::process::id()));
        let metagraph = Arc::new(
            MetagraphDB::open(&temp_dir).expect("Failed to create MetagraphDB")
        );

        Self {
            db,
            state,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            metagraph,
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            pending_txs,
            ai_tasks: Arc::new(RwLock::new(HashMap::new())),
            broadcaster,
            evm_state,
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(CommitRevealConfig::default()))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            data_dir: temp_dir,
        }
    }

    /// Create a new RPC server with validator set
    pub fn with_validators(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        metagraph: Arc<MetagraphDB>,
        validators: Arc<RwLock<ValidatorSet>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
    ) -> Self {
        let subnets = Arc::new(RwLock::new(Self::load_subnets_cache(&metagraph)));
        let neurons = Arc::new(RwLock::new(Self::load_neurons_cache(&metagraph)));

        Self {
            db,
            state,
            validators,
            metagraph,
            subnets,
            neurons,
            weights: Arc::new(RwLock::new(HashMap::new())),
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
            ai_tasks: Arc::new(RwLock::new(HashMap::new())),
            broadcaster,
            evm_state: Arc::new(RwLock::new(EvmState::new(1337))),
            commit_reveal: Arc::new(RwLock::new(CommitRevealManager::new(CommitRevealConfig::default()))),
            ai_circuit_breaker: Arc::new(AILayerCircuitBreaker::new()),
            data_dir: PathBuf::from("./data"),
        }
    }

    /// Load subnets from MetagraphDB into cache
    fn load_subnets_cache(metagraph: &MetagraphDB) -> HashMap<u64, SubnetInfo> {
        let mut cache = HashMap::new();
        if let Ok(subnets) = metagraph.get_all_subnets() {
            for subnet in subnets {
                cache.insert(subnet.id, SubnetInfo {
                    id: subnet.id,
                    name: subnet.name.clone(),
                    owner: format!("0x{}", hex::encode(subnet.owner)),
                    emission_rate: subnet.emission_rate,
                    participant_count: 0,
                    total_stake: 0,
                    created_at: subnet.created_at,
                });
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
                        cache.insert((neuron.subnet_id, neuron.uid), NeuronInfo {
                            uid: neuron.uid,
                            address: format!("0x{}", hex::encode(neuron.hotkey)),
                            subnet_id: neuron.subnet_id,
                            stake: neuron.stake,
                            trust: neuron.trust as f64 / 65535.0,
                            rank: neuron.rank as u64,
                            incentive: neuron.incentive as f64 / 65535.0,
                            dividends: neuron.dividends as f64 / 65535.0,
                            active: neuron.active,
                            endpoint: Some(neuron.endpoint),
                        });
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
    pub fn start(self, addr: &str) -> Result<Server> {
        let mut io = IoHandler::new();

        // Register blockchain query methods
        self.register_blockchain_methods(&mut io);

        // Register account methods
        self.register_account_methods(&mut io);

        // Register modular handlers (with DB persistence)
        register_staking_handlers(&mut io, self.validators.clone(), self.db.clone());
        register_subnet_handlers(&mut io, self.subnets.clone(), self.db.clone());
        register_neuron_handlers(&mut io, self.neurons.clone(), self.subnets.clone(), self.db.clone());
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

        // Register Ethereum-compatible methods (eth_*)
        register_eth_methods(&mut io, self.evm_state.clone());

        // Register transaction methods with P2P broadcasting (eth_sendTransaction, eth_getTransactionReceipt)
        // These override the base eth_rpc implementations with broadcast support
        let tx_ctx = TxRpcContext::new(
            self.evm_state.clone(),
            self.pending_txs.clone(),
            self.state.clone(),
            self.broadcaster.clone(),
            self.db.clone(),
        );
        register_tx_methods(&tx_ctx, &mut io);

        // Start HTTP server
        let server = ServerBuilder::new(io)
            .threads(4)
            .start_http(&addr.parse().map_err(|e: std::net::AddrParseError| {
                RpcError::ServerError(e.to_string())
            })?)
            .map_err(|e| RpcError::ServerError(e.to_string()))?;

        Ok(server)
    }

    /// Register blockchain query methods
    fn register_blockchain_methods(&self, io: &mut IoHandler) {
        // eth_blockNumber - Get current block height
        let db_for_block_num = self.db.clone();
        io.add_sync_method("eth_blockNumber", move |_params: Params| {
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
                        if ceiling > 1_000_000 { break; }
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

            Ok(Value::String(format!("0x{:x}", low)))
        });

        // eth_getBlockByNumber - Get block by number
        let db_for_get_block = self.db.clone();
        io.add_sync_method("eth_getBlockByNumber", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing block number"));
            }

            let height = parse_block_number(&parsed[0])?;
            let _include_txs = parsed
                .get(1)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

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
                return Err(jsonrpc_core::Error::invalid_params(
                    "Hash must be 32 bytes",
                ));
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
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing transaction hash",
                ));
            }

            let hash_str = parsed[0].trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_str)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hash format"))?;

            if hash_bytes.len() != 32 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Hash must be 32 bytes",
                ));
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);

            // 1. Check pending transactions first (in-memory mempool)
            {
                let pending = pending_txs_query.read();
                if let Some(tx) = pending.get(&hash) {
                    let rpc_tx = RpcTransaction::from(tx.clone());
                    return serde_json::to_value(rpc_tx)
                        .map_err(|_| jsonrpc_core::Error::internal_error());
                }
            }

            // 2. Fallback to confirmed transactions in database
            match db.get_transaction(&hash) {
                Ok(Some(tx)) => {
                    let rpc_tx = RpcTransaction::from(tx);
                    serde_json::to_value(rpc_tx)
                        .map_err(|_| jsonrpc_core::Error::internal_error())
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });
    }

    /// Register account methods
    fn register_account_methods(&self, io: &mut IoHandler) {
        let state = self.state.clone();

        // eth_getBalance - Get account balance
        io.add_sync_method("eth_getBalance", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;

            let balance = state.read().get_balance(&address);
            Ok(Value::String(format!("0x{:x}", balance)))
        });

        let state = self.state.clone();

        // eth_getTransactionCount - Get account nonce
        io.add_sync_method("eth_getTransactionCount", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;

            let nonce = state.read().get_nonce(&address);
            Ok(Value::String(format!("0x{:x}", nonce)))
        });

        // eth_sendRawTransaction - Submit raw signed transaction
        let state = self.state.clone();
        let pending_txs = self.pending_txs.clone();
        let broadcaster = self.broadcaster.clone();

        io.add_sync_method("eth_sendRawTransaction", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing transaction data"));
            }

            let tx_hex = parsed[0].trim_start_matches("0x");
            let tx_bytes = hex::decode(tx_hex)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid transaction hex"))?;

            // Calculate transaction hash
            let tx_hash = luxtensor_crypto::keccak256(&tx_bytes);

            // 1. Decode the transaction (RLP decode)
            // For now, use bincode for internal format. In production, use RLP.
            let tx: Transaction = bincode::deserialize(&tx_bytes)
                .map_err(|e| {
                    jsonrpc_core::Error::invalid_params(format!("Failed to decode transaction: {}", e))
                })?;

            // 2. Verify signature
            if tx.verify_signature().is_err() {
                return Err(jsonrpc_core::Error::invalid_params("Invalid transaction signature"));
            }

            // 3. Check nonce
            let state_guard = state.read();
            let expected_nonce = state_guard.get_nonce(&tx.from);
            if tx.nonce < expected_nonce {
                return Err(jsonrpc_core::Error::invalid_params(
                    format!("Nonce too low. Expected: {}, got: {}", expected_nonce, tx.nonce)
                ));
            }

            // 4. Check balance for gas
            let balance = state_guard.get_balance(&tx.from);
            let gas_cost = (tx.gas_price as u128) * (tx.gas_limit as u128);
            let required = tx.value.saturating_add(gas_cost);
            if balance < required {
                return Err(jsonrpc_core::Error::invalid_params(
                    format!("Insufficient balance. Required: {}, available: {}", required, balance)
                ));
            }
            drop(state_guard);

            // 5. Add to pending transactions (mempool)
            {
                let mut pending = pending_txs.write();

                // Check for duplicate
                if pending.contains_key(&tx_hash) {
                    return Err(jsonrpc_core::Error::invalid_params("Transaction already pending"));
                }

                // Check mempool size limit
                if pending.len() >= 10000 {
                    return Err(jsonrpc_core::Error::invalid_params("Mempool full"));
                }

                pending.insert(tx_hash, tx.clone());
                info!("Transaction added to mempool: 0x{}", hex::encode(&tx_hash));
            }

            // 6. Broadcast to P2P network and/or WebSocket subscribers
            if let Err(e) = broadcaster.broadcast(&tx) {
                warn!("Failed to broadcast transaction: {}", e);
                // Note: We don't return error here since tx is already in mempool
                // It will be included in blocks even if broadcast failed
            } else {
                debug!("Transaction broadcast successful via {}", broadcaster.name());
            }

            // Return the transaction hash
            Ok(Value::String(format!("0x{}", hex::encode(tx_hash))))
        });

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
                    // Build receipt
                    let receipt = serde_json::json!({
                        "transactionHash": format!("0x{}", hex::encode(hash)),
                        "status": "0x1", // Success
                        "blockNumber": "0x0",
                        "gasUsed": "0x5208",
                        "cumulativeGasUsed": "0x5208",
                        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                        "to": tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
                    });
                    Ok(receipt)
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::{Block, BlockHeader, Transaction, Address};
    use luxtensor_storage::BlockchainDB;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_setup() -> (TempDir, Arc<BlockchainDB>, Arc<RwLock<StateDB>>) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("blockchain");

        let blockchain_db = Arc::new(BlockchainDB::open(&db_path).unwrap());
        let state_db = Arc::new(RwLock::new(StateDB::new()));

        (temp_dir, blockchain_db, state_db)
    }

    #[test]
    fn test_rpc_server_creation() {
        let (_temp, db, state) = create_test_setup();
        let _server = RpcServer::new_for_testing(db, state);
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
        let tx = Transaction::new(
            1,
            Address::zero(),
            Some(Address::zero()),
            1000,
            1,
            21000,
            vec![],
        );

        let rpc_tx = RpcTransaction::from(tx);
        assert_eq!(rpc_tx.nonce, "0x1");
        assert_eq!(rpc_tx.value, "0x3e8");
    }
}
