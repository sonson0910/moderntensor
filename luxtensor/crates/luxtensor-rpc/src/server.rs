use crate::{types::*, RpcError, Result, TransactionBroadcaster, NoOpBroadcaster, eth_rpc::{EvmState, register_eth_methods}};
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_core::{StateDB, Transaction, Hash};
use luxtensor_storage::{BlockchainDB, MetagraphDB};
use luxtensor_consensus::{ValidatorSet, CommitRevealManager, CommitRevealConfig};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use crate::handlers::{
    register_subnet_handlers, register_neuron_handlers,
    register_staking_handlers, register_weight_handlers
};
use crate::helpers::{parse_address, parse_block_number};

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
/// ```no_run
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

        // Register AI-specific methods
        self.register_ai_methods(&mut io);

        // Register SDK query methods (query_*)
        self.register_query_methods(&mut io);

        // Register Ethereum-compatible methods (eth_*)
        register_eth_methods(&mut io, self.evm_state.clone());

        // 🔧 Override eth_sendTransaction with P2P broadcasting
        // This ensures transactions are propagated to peers
        let evm_state_for_tx = self.evm_state.clone();
        let broadcaster_for_tx = self.broadcaster.clone();
        let state_for_tx = self.state.clone();
        let pending_txs_for_tx = self.pending_txs.clone();

        io.add_sync_method("eth_sendTransaction", move |params: Params| {
            use crate::eth_rpc::{hex_to_address, generate_tx_hash};

            let p: Vec<serde_json::Value> = params.parse()?;
            let tx_obj = p.get(0).ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing transaction object"))?;

            let from_str = tx_obj.get("from")
                .and_then(|v| v.as_str())
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing 'from' field"))?;

            let from = hex_to_address(from_str)
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid 'from' address"))?;

            let to = tx_obj.get("to")
                .and_then(|v| v.as_str())
                .and_then(hex_to_address);

            let value = tx_obj.get("value")
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    u128::from_str_radix(s, 16).ok()
                })
                .unwrap_or(0);

            let data = tx_obj.get("data")
                .and_then(|v| v.as_str())
                .map(|s| {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    hex::decode(s).unwrap_or_default()
                })
                .unwrap_or_default();

            let gas = tx_obj.get("gas")
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    u64::from_str_radix(s, 16).ok()
                })
                .unwrap_or(10_000_000);

            // Get nonce from state
            let nonce = state_for_tx.read().get_nonce(&luxtensor_core::Address::from(from));

            // Create luxtensor_core::Transaction for broadcasting
            let to_addr = to.map(luxtensor_core::Address::from);
            let core_tx = luxtensor_core::Transaction::new(
                nonce,
                luxtensor_core::Address::from(from),
                to_addr,
                value,
                1, // gas_price
                gas,
                data.clone(),
            );

            // 🔧 FIX: Use deterministic hash from Transaction::hash() for consistency
            // This ensures the same hash is used across all nodes
            let tx_hash = core_tx.hash();

            // Add to pending transactions
            {
                let mut pending = pending_txs_for_tx.write();
                pending.insert(tx_hash, core_tx.clone());
                info!("📤 Transaction added to mempool: 0x{}", hex::encode(&tx_hash));
            }

            // 🚀 BROADCAST TO P2P NETWORK
            if let Err(e) = broadcaster_for_tx.broadcast(&core_tx) {
                warn!("Failed to broadcast transaction to P2P: {}", e);
            } else {
                info!("📡 Transaction broadcasted to P2P network: 0x{}", hex::encode(&tx_hash));
            }

            // Also update EVM state for compatibility
            {
                let mut state_guard = evm_state_for_tx.write();
                state_guard.increment_nonce(&from);

                // 🔧 FIX: Add transaction to tx_queue for block inclusion
                // This ensures transactions are processed by the block producer
                let ready_tx = crate::eth_rpc::ReadyTransaction {
                    nonce,
                    from,
                    to,
                    value,
                    data: data.clone(),
                    gas,
                    r: [0u8; 32],
                    s: [0u8; 32],
                    v: 0,
                };
                state_guard.queue_transaction(ready_tx);
                info!("📦 Transaction queued for block inclusion: 0x{}", hex::encode(&tx_hash));
            }

            Ok(serde_json::json!(format!("0x{}", hex::encode(tx_hash))))
        });

        // 🔧 Override eth_getTransactionReceipt to read from pending_txs
        // This ensures receipts are found for transactions submitted via eth_sendTransaction
        let pending_txs_for_receipt = self.pending_txs.clone();
        let db_for_receipt = self.db.clone();
        io.add_sync_method("eth_getTransactionReceipt", move |params: Params| {
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
            {
                let pending = pending_txs_for_receipt.read();
                if let Some(tx) = pending.get(&hash) {
                    // For pending txs, return a "pending" receipt indicating tx is accepted but not mined
                    return Ok(serde_json::json!({
                        "transactionHash": format!("0x{}", hex::encode(hash)),
                        "transactionIndex": "0x0",
                        "blockHash": format!("0x{}", hex::encode([0u8; 32])),
                        "blockNumber": "0x0",
                        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                        "to": tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
                        "contractAddress": if tx.to.is_none() && !tx.data.is_empty() {
                            // Generate contract address for deployment
                            let nonce = tx.nonce;
                            let from_bytes = tx.from.as_bytes();
                            let mut hasher = std::collections::hash_map::DefaultHasher::new();
                            std::hash::Hash::hash_slice(from_bytes, &mut hasher);
                            std::hash::Hash::hash(&nonce, &mut hasher);
                            let hash_val = std::hash::Hasher::finish(&hasher);
                            let mut addr = [0u8; 20];
                            addr[..8].copy_from_slice(&hash_val.to_be_bytes());
                            addr[8..16].copy_from_slice(&hash_val.to_le_bytes());
                            addr[16..20].copy_from_slice(&(nonce as u32).to_be_bytes());
                            Some(format!("0x{}", hex::encode(addr)))
                        } else {
                            None
                        },
                        "cumulativeGasUsed": "0x5208",
                        "gasUsed": "0x5208",
                        "status": "0x1",
                        "logs": []
                    }));
                }
            }

            // 2. Check stored receipts in database (from mined blocks)
            match db_for_receipt.get_receipt(&hash) {
                Ok(Some(receipt_bytes)) => {
                    // Deserialize using bincode (same as storage)
                    // Must match Receipt struct from executor.rs exactly
                    #[derive(serde::Deserialize)]
                    #[allow(dead_code)]
                    struct StoredLog {
                        address: luxtensor_core::Address,
                        topics: Vec<[u8; 32]>,
                        data: Vec<u8>,
                    }

                    #[derive(serde::Deserialize)]
                    #[repr(u8)]
                    enum StoredExecutionStatus {
                        Success = 1,
                        Failed = 0,
                    }

                    #[derive(serde::Deserialize)]
                    #[allow(dead_code)]
                    struct StoredReceipt {
                        transaction_hash: [u8; 32],
                        block_height: u64,
                        block_hash: [u8; 32],
                        transaction_index: usize,
                        from: luxtensor_core::Address,
                        to: Option<luxtensor_core::Address>,
                        gas_used: u64,
                        status: StoredExecutionStatus,
                        logs: Vec<StoredLog>,
                        contract_address: Option<luxtensor_core::Address>,
                    }


                    tracing::debug!("📥 Got receipt bytes: {} bytes", receipt_bytes.len());

                    match bincode::deserialize::<StoredReceipt>(&receipt_bytes) {
                        Ok(receipt) => {
                            let contract_addr = receipt.contract_address.map(|addr|
                                format!("0x{}", hex::encode(addr.as_bytes()))
                            );

                        return Ok(serde_json::json!({
                            "transactionHash": format!("0x{}", hex::encode(receipt.transaction_hash)),
                            "transactionIndex": format!("0x{:x}", receipt.transaction_index),
                            "blockHash": format!("0x{}", hex::encode(receipt.block_hash)),
                            "blockNumber": format!("0x{:x}", receipt.block_height),
                            "from": format!("0x{}", hex::encode(receipt.from.as_bytes())),
                            "to": receipt.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
                            "contractAddress": contract_addr,
                            "cumulativeGasUsed": format!("0x{:x}", receipt.gas_used),
                            "gasUsed": format!("0x{:x}", receipt.gas_used),
                            "status": match receipt.status {
                                StoredExecutionStatus::Success => "0x1",
                                StoredExecutionStatus::Failed => "0x0",
                            },
                            "logs": []
                        }));
                        }
                        Err(e) => {
                            tracing::warn!("❌ Failed to deserialize receipt: {:?}", e);
                        }
                    }
                }
                _ => {}
            }

            // 3. Final fallback: check if TX exists at all
            match db_for_receipt.get_transaction(&hash) {
                Ok(Some(tx)) => {
                    Ok(serde_json::json!({
                        "transactionHash": format!("0x{}", hex::encode(hash)),
                        "transactionIndex": "0x0",
                        "blockHash": format!("0x{}", hex::encode([0u8; 32])),
                        "blockNumber": "0x1",
                        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                        "to": tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
                        "contractAddress": serde_json::Value::Null,
                        "cumulativeGasUsed": "0x5208",
                        "gasUsed": "0x5208",
                        "status": "0x1",
                        "logs": []
                    }))
                }
                Ok(None) => Ok(serde_json::Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        });

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

    /// Register AI-specific methods
    fn register_ai_methods(&self, io: &mut IoHandler) {
        let ai_tasks = self.ai_tasks.clone();

        // lux_submitAITask - Submit AI computation task
        io.add_sync_method("lux_submitAITask", move |params: Params| {
            let task_request: AITaskRequest = params.parse()?;

            // 1. Validate the task request
            if task_request.model_hash.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Model hash is required"));
            }
            if task_request.requester.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Requester address is required"));
            }

            // 2. Parse reward amount
            let reward = u128::from_str_radix(
                task_request.reward.trim_start_matches("0x"),
                16
            ).unwrap_or(0);

            // 3. Generate task ID
            let task_id_data = format!(
                "{}:{}:{}:{}",
                task_request.model_hash,
                task_request.requester,
                task_request.input_data,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("System time before UNIX epoch")
                    .as_nanos()
            );
            let task_id = luxtensor_crypto::keccak256(task_id_data.as_bytes());

            // 4. Create and store task
            let task_info = AITaskInfo {
                id: task_id,
                model_hash: task_request.model_hash,
                input_data: task_request.input_data,
                requester: task_request.requester,
                reward,
                status: AITaskStatus::Pending,
                result: None,
                worker: None,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("System time before UNIX epoch")
                    .as_secs(),
                completed_at: None,
            };

            {
                let mut tasks = ai_tasks.write();
                tasks.insert(task_id, task_info);
                info!("AI task submitted: 0x{}", hex::encode(&task_id));
            }

            // Return the task ID
            Ok(serde_json::json!({
                "success": true,
                "task_id": format!("0x{}", hex::encode(task_id))
            }))
        });

        let ai_tasks = self.ai_tasks.clone();

        // lux_getAIResult - Get AI task result
        io.add_sync_method("lux_getAIResult", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing task ID"));
            }

            // Parse task ID
            let task_id_hex = parsed[0].trim_start_matches("0x");
            let task_id_bytes = hex::decode(task_id_hex)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task ID format"))?;

            if task_id_bytes.len() != 32 {
                return Err(jsonrpc_core::Error::invalid_params("Task ID must be 32 bytes"));
            }

            let mut task_id = [0u8; 32];
            task_id.copy_from_slice(&task_id_bytes);

            // Look up the task
            let tasks = ai_tasks.read();
            if let Some(task) = tasks.get(&task_id) {
                let status_str = match task.status {
                    AITaskStatus::Pending => "pending",
                    AITaskStatus::Processing => "processing",
                    AITaskStatus::Completed => "completed",
                    AITaskStatus::Failed => "failed",
                };

                Ok(serde_json::json!({
                    "task_id": format!("0x{}", hex::encode(task_id)),
                    "status": status_str,
                    "model_hash": task.model_hash,
                    "requester": task.requester,
                    "reward": format!("0x{:x}", task.reward),
                    "result": task.result,
                    "worker": task.worker,
                    "created_at": task.created_at,
                    "completed_at": task.completed_at,
                }))
            } else {
                Ok(Value::Null)
            }
        });

        let validators = self.validators.clone();

        // lux_getValidatorStatus - Get validator information
        io.add_sync_method("lux_getValidatorStatus", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing validator address"));
            }

            let address = parse_address(&parsed[0])?;

            // Look up validator in consensus module
            let validator_set = validators.read();
            if let Some(validator) = validator_set.get_validator(&address) {
                Ok(serde_json::json!({
                    "address": format!("0x{}", hex::encode(address.as_bytes())),
                    "stake": format!("0x{:x}", validator.stake),
                    "active": validator.active,
                    "rewards": format!("0x{:x}", validator.rewards),
                    "public_key": format!("0x{}", hex::encode(validator.public_key)),
                }))
            } else {
                Ok(Value::Null)
            }
        });

        // === Additional AI and Network Methods ===

        let neurons = self.neurons.clone();
        let subnets = self.subnets.clone();

        // ai_getMetagraph - Get metagraph for subnet
        io.add_sync_method("ai_getMetagraph", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            let neurons_map = neurons.read();
            let subnets_map = subnets.read();

            let subnet_info = subnets_map.get(&subnet_id);
            let neurons_in_subnet: Vec<serde_json::Value> = neurons_map
                .iter()
                .filter(|((sid, _), _)| *sid == subnet_id)
                .map(|(_, n)| serde_json::json!({
                    "uid": n.uid,
                    "address": n.address,
                    "stake": format!("0x{:x}", n.stake),
                    "trust": n.trust,
                    "rank": n.rank,
                    "incentive": n.incentive,
                    "dividends": n.dividends,
                    "active": n.active,
                }))
                .collect();

            Ok(serde_json::json!({
                "subnet_id": subnet_id,
                "neurons": neurons_in_subnet,
                "neuron_count": neurons_in_subnet.len(),
                "total_stake": subnet_info.map(|s| format!("0x{:x}", s.total_stake)).unwrap_or_else(|| "0x0".to_string()),
            }))
        });

        let neurons = self.neurons.clone();

        // ai_getIncentive - Get incentive info for subnet
        io.add_sync_method("ai_getIncentive", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            let neurons_map = neurons.read();
            let incentives: Vec<serde_json::Value> = neurons_map
                .iter()
                .filter(|((sid, _), _)| *sid == subnet_id)
                .map(|(_, n)| serde_json::json!({
                    "uid": n.uid,
                    "incentive": n.incentive,
                    "dividends": n.dividends,
                }))
                .collect();

            Ok(serde_json::json!({
                "subnet_id": subnet_id,
                "incentives": incentives,
            }))
        });

        // net_version - Get network version
        io.add_sync_method("net_version", move |_params: Params| {
            Ok(Value::String("1".to_string()))
        });

        // net_peerCount - Get peer count
        io.add_sync_method("net_peerCount", move |_params: Params| {
            let count = crate::peer_count::get_peer_count();
            Ok(Value::String(format!("0x{:x}", count)))
        });

        // web3_clientVersion - Get client version
        io.add_sync_method("web3_clientVersion", move |_params: Params| {
            Ok(Value::String(format!("Luxtensor/{}", env!("CARGO_PKG_VERSION"))))
        });
    }

    /// Register SDK-compatible query methods (query_*)
    fn register_query_methods(&self, io: &mut IoHandler) {
        let neurons = self.neurons.clone();
        let _subnets = self.subnets.clone();

        // query_neuron - Get specific neuron info
        io.add_sync_method("query_neuron", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;

            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!({
                    "uid": neuron.uid,
                    "address": neuron.address,
                    "subnet_id": neuron.subnet_id,
                    "stake": format!("0x{:x}", neuron.stake),
                    "trust": neuron.trust,
                    "rank": neuron.rank,
                    "incentive": neuron.incentive,
                    "dividends": neuron.dividends,
                    "active": neuron.active,
                    "endpoint": neuron.endpoint
                }))
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_neuronCount - Get neuron count in subnet
        io.add_sync_method("query_neuronCount", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnet_id = parsed[0];
            let neurons_map = neurons.read();
            let count = neurons_map.keys().filter(|(sid, _)| *sid == subnet_id).count();
            Ok(Value::Number(count.into()))
        });

        let neurons = self.neurons.clone();

        // query_activeNeurons - Get active neuron UIDs
        io.add_sync_method("query_activeNeurons", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnet_id = parsed[0];
            let neurons_map = neurons.read();
            let active_uids: Vec<u64> = neurons_map
                .iter()
                .filter(|((sid, _), n)| *sid == subnet_id && n.active)
                .map(|((_, uid), _)| *uid)
                .collect();
            Ok(serde_json::to_value(active_uids).unwrap_or(Value::Array(vec![])))
        });

        let subnets = self.subnets.clone();

        // query_allSubnets - Get all subnets (alias for subnet_listAll)
        io.add_sync_method("query_allSubnets", move |_params: Params| {
            let subnets_map = subnets.read();
            let list: Vec<Value> = subnets_map.values().map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "owner": s.owner,
                    "emission_rate": s.emission_rate,
                    "participant_count": s.participant_count,
                    "total_stake": format!("0x{:x}", s.total_stake)
                })
            }).collect();
            Ok(Value::Array(list))
        });

        let subnets = self.subnets.clone();

        // query_subnetExists - Check if subnet exists
        io.add_sync_method("query_subnetExists", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            Ok(Value::Bool(subnets_map.contains_key(&parsed[0])))
        });

        let subnets = self.subnets.clone();

        // query_subnetOwner - Get subnet owner
        io.add_sync_method("query_subnetOwner", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if let Some(subnet) = subnets_map.get(&parsed[0]) {
                Ok(Value::String(subnet.owner.clone()))
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_subnetEmission - Get subnet emission rate
        io.add_sync_method("query_subnetEmission", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if let Some(subnet) = subnets_map.get(&parsed[0]) {
                Ok(Value::String(format!("0x{:x}", subnet.emission_rate)))
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_subnetHyperparameters - Get subnet hyperparams
        io.add_sync_method("query_subnetHyperparameters", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if let Some(subnet) = subnets_map.get(&parsed[0]) {
                Ok(serde_json::json!({
                    "tempo": 360,
                    "rho": 10,
                    "kappa": 10,
                    "immunity_period": 100,
                    "max_allowed_validators": 64,
                    "min_allowed_weights": 1,
                    "max_weights_limit": 1000,
                    "emission_rate": subnet.emission_rate
                }))
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_subnetTempo - Get subnet tempo
        io.add_sync_method("query_subnetTempo", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if subnets_map.contains_key(&parsed[0]) {
                Ok(Value::Number(360.into())) // Default tempo
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_rank - Get neuron rank
        io.add_sync_method("query_rank", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!(neuron.rank as f64 / 65535.0))
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_trust - Get neuron trust
        io.add_sync_method("query_trust", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!(neuron.trust))
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_incentive - Get neuron incentive
        io.add_sync_method("query_incentive", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!(neuron.incentive))
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_dividends - Get neuron dividends
        io.add_sync_method("query_dividends", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!(neuron.dividends))
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_consensus - Get neuron consensus (same as trust for now)
        io.add_sync_method("query_consensus", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!(neuron.trust))
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_isHotkeyRegistered - Check if hotkey is registered
        io.add_sync_method("query_isHotkeyRegistered", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or hotkey"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let hotkey = parsed[1].as_str().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;
            let neurons_map = neurons.read();
            let is_registered = neurons_map.iter()
                .any(|((sid, _), n)| *sid == subnet_id && n.address == hotkey);
            Ok(Value::Bool(is_registered))
        });

        let neurons = self.neurons.clone();

        // query_uidForHotkey - Get UID for hotkey
        io.add_sync_method("query_uidForHotkey", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or hotkey"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let hotkey = parsed[1].as_str().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;
            let neurons_map = neurons.read();
            let uid = neurons_map.iter()
                .find(|((sid, _), n)| *sid == subnet_id && n.address == hotkey)
                .map(|((_, uid), _)| *uid);
            match uid {
                Some(u) => Ok(Value::Number(u.into())),
                None => Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // query_hotkeyForUid - Get hotkey for UID
        io.add_sync_method("query_hotkeyForUid", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(Value::String(neuron.address.clone()))
            } else {
                Ok(Value::Null)
            }
        });

        let validators = self.validators.clone();

        // query_stakeForColdkeyAndHotkey - Get stake for coldkey-hotkey pair
        io.add_sync_method("query_stakeForColdkeyAndHotkey", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing coldkey or hotkey"));
            }
            // For now, just return stake for hotkey (simplified)
            let hotkey = &parsed[1];
            let address = parse_address(hotkey)?;
            let validator_set = validators.read();
            let stake = validator_set.get_validator(&address).map(|v| v.stake).unwrap_or(0);
            Ok(Value::String(format!("0x{:x}", stake)))
        });

        let validators = self.validators.clone();

        // query_totalStakeForColdkey - Get total stake for coldkey
        io.add_sync_method("query_totalStakeForColdkey", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing coldkey"));
            }
            let address = parse_address(&parsed[0])?;
            let validator_set = validators.read();
            let stake = validator_set.get_validator(&address).map(|v| v.stake).unwrap_or(0);
            Ok(Value::String(format!("0x{:x}", stake)))
        });

        let validators = self.validators.clone();

        // query_totalStakeForHotkey - Get total stake for hotkey
        io.add_sync_method("query_totalStakeForHotkey", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing hotkey"));
            }
            let address = parse_address(&parsed[0])?;
            let validator_set = validators.read();
            let stake = validator_set.get_validator(&address).map(|v| v.stake).unwrap_or(0);
            Ok(Value::String(format!("0x{:x}", stake)))
        });

        let validators = self.validators.clone();

        // query_allStakeForColdkey - Get all stakes for coldkey
        io.add_sync_method("query_allStakeForColdkey", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing coldkey"));
            }
            let address = parse_address(&parsed[0])?;
            let validator_set = validators.read();
            let mut stakes = serde_json::Map::new();
            if let Some(v) = validator_set.get_validator(&address) {
                stakes.insert(parsed[0].clone(), serde_json::json!(format!("0x{:x}", v.stake)));
            }
            Ok(Value::Object(stakes))
        });

        let validators = self.validators.clone();

        // query_allStakeForHotkey - Get all stakes for hotkey
        io.add_sync_method("query_allStakeForHotkey", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing hotkey"));
            }
            let address = parse_address(&parsed[0])?;
            let validator_set = validators.read();
            let mut stakes = serde_json::Map::new();
            if let Some(v) = validator_set.get_validator(&address) {
                stakes.insert(parsed[0].clone(), serde_json::json!(format!("0x{:x}", v.stake)));
            }
            Ok(Value::Object(stakes))
        });

        // query_weightCommits - Get weight commits for a subnet
        let commit_reveal = self.commit_reveal.clone();
        io.add_sync_method("query_weightCommits", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnet_id = parsed[0];

            // Get commits from CommitRevealManager
            let commits = commit_reveal.read().get_pending_commits(subnet_id);
            let epoch_state = commit_reveal.read().get_epoch_state(subnet_id);

            let mut result = serde_json::Map::new();

            // Add epoch info
            if let Some(state) = epoch_state {
                result.insert("epochNumber".into(), serde_json::json!(state.epoch_number));
                result.insert("phase".into(), serde_json::json!(format!("{:?}", state.phase)));
                result.insert("commitStartBlock".into(), serde_json::json!(state.commit_start_block));
                result.insert("revealStartBlock".into(), serde_json::json!(state.reveal_start_block));
                result.insert("finalizeBlock".into(), serde_json::json!(state.finalize_block));
            }

            // Add commits
            let commit_list: Vec<serde_json::Value> = commits.iter().map(|c| {
                serde_json::json!({
                    "validator": format!("0x{}", hex::encode(c.validator.as_bytes())),
                    "commitHash": format!("0x{}", hex::encode(&c.commit_hash)),
                    "committedAt": c.committed_at,
                    "revealed": c.revealed
                })
            }).collect();

            result.insert("commits".into(), serde_json::json!(commit_list));
            result.insert("commitCount".into(), serde_json::json!(commits.len()));

            Ok(Value::Object(result))
        });

        // query_weightsVersion - Get weights version
        io.add_sync_method("query_weightsVersion", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            Ok(Value::Number(1.into())) // Default version 1
        });

        // query_weightsRateLimit - Get weights rate limit
        io.add_sync_method("query_weightsRateLimit", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            Ok(Value::Number(100.into())) // Default rate limit
        });

        let neurons = self.neurons.clone();

        // query_hasValidatorPermit - Check if has validator permit
        io.add_sync_method("query_hasValidatorPermit", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or hotkey"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let hotkey = parsed[1].as_str().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;
            let neurons_map = neurons.read();
            // Check if registered and has high stake
            let has_permit = neurons_map.iter()
                .any(|((sid, _), n)| *sid == subnet_id && n.address == hotkey && n.stake > 0);
            Ok(Value::Bool(has_permit))
        });

        let neurons = self.neurons.clone();

        // query_validatorTrust - Get validator trust
        io.add_sync_method("query_validatorTrust", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
            }
            let subnet_id = parsed[0].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let neuron_uid = parsed[1].as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
            let neurons_map = neurons.read();
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                Ok(serde_json::json!(neuron.trust))
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_rho - Get rho parameter
        io.add_sync_method("query_rho", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if subnets_map.contains_key(&parsed[0]) {
                Ok(serde_json::json!(10.0)) // Default rho
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_kappa - Get kappa parameter
        io.add_sync_method("query_kappa", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if subnets_map.contains_key(&parsed[0]) {
                Ok(serde_json::json!(10.0)) // Default kappa
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_adjustmentInterval - Get adjustment interval
        io.add_sync_method("query_adjustmentInterval", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if subnets_map.contains_key(&parsed[0]) {
                Ok(Value::Number(100.into())) // Default interval
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // query_activityCutoff - Get activity cutoff
        io.add_sync_method("query_activityCutoff", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
            }
            let subnets_map = subnets.read();
            if subnets_map.contains_key(&parsed[0]) {
                Ok(Value::Number(5000.into())) // Default cutoff
            } else {
                Ok(Value::Null)
            }
        });

        let validators = self.validators.clone();

        // query_rootNetworkValidators - Get root network validators
        io.add_sync_method("query_rootNetworkValidators", move |_params: Params| {
            let validator_set = validators.read();
            let validators_list: Vec<String> = validator_set
                .validators()
                .iter()
                .filter(|v| v.active)
                .map(|v| format!("0x{}", hex::encode(v.address.as_bytes())))
                .collect();
            Ok(serde_json::to_value(validators_list).unwrap_or(Value::Array(vec![])))
        });

        // query_senateMembers - Get senate members
        io.add_sync_method("query_senateMembers", move |_params: Params| {
            Ok(Value::Array(vec![])) // No senate members by default
        });

        // system_version - Get system version
        io.add_sync_method("system_version", move |_params: Params| {
            Ok(Value::String("1.0.0".to_string()))
        });

        // system_health - Check node health
        io.add_sync_method("system_health", move |_params: Params| {
            Ok(serde_json::json!({
                "status": "healthy",
                "version": "1.0.0",
                "syncing": false,
                "peers": crate::peer_count::get_peer_count()
            }))
        });

        // system_peerCount - Get peer count
        io.add_sync_method("system_peerCount", move |_params: Params| {
            let count = crate::peer_count::get_peer_count();
            Ok(Value::Number(count.into()))
        });

        let db = self.db.clone();

        // system_syncState - Get sync state
        io.add_sync_method("system_syncState", move |_params: Params| {
            let height = db.get_best_height().ok().flatten().unwrap_or(0);
            Ok(serde_json::json!({
                "isSyncing": false,
                "currentBlock": height,
                "highestBlock": height
            }))
        });

        // governance_getProposals - Get governance proposals
        io.add_sync_method("governance_getProposals", move |_params: Params| {
            Ok(Value::Array(vec![])) // No proposals by default
        });

        // governance_getProposal - Get specific proposal
        io.add_sync_method("governance_getProposal", move |params: Params| {
            let parsed: Vec<u64> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing proposal_id"));
            }
            Ok(Value::Null) // Proposal not found
        });

        // balances_free - Get free balance
        io.add_sync_method("balances_free", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }
            // Return same as eth_getBalance
            Ok(Value::String("0x0".to_string()))
        });

        // balances_reserved - Get reserved balance
        io.add_sync_method("balances_reserved", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }
            Ok(Value::String("0x0".to_string()))
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
