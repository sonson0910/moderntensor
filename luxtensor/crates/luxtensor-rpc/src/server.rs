use crate::{types::*, RpcError, Result};
use crate::websocket::BroadcastEvent;
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_core::{Address, StateDB, Transaction};
use luxtensor_storage::BlockchainDB;
use luxtensor_consensus::{ValidatorSet, Validator};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// JSON-RPC server for LuxTensor blockchain
/// 
/// The RPC server provides a JSON-RPC API for interacting with the blockchain.
/// It supports both synchronous HTTP requests and asynchronous WebSocket subscriptions.
/// 
/// # WebSocket Integration
/// 
/// When a `broadcast_tx` is provided via `with_broadcast()` or `with_all()`, the RPC server
/// will broadcast pending transactions to WebSocket subscribers. This allows clients to
/// receive real-time notifications when new transactions are submitted via `eth_sendRawTransaction`.
/// 
/// # Example
/// 
/// ```no_run
/// use luxtensor_rpc::{RpcServer, websocket::WebSocketServer};
/// use luxtensor_storage::BlockchainDB;
/// use luxtensor_core::StateDB;
/// use parking_lot::RwLock;
/// use std::sync::Arc;
/// 
/// # async fn example() {
/// let db = Arc::new(BlockchainDB::open("./data").unwrap());
/// let state = Arc::new(RwLock::new(StateDB::new()));
/// 
/// // Create WebSocket server and get broadcast sender
/// let ws_server = WebSocketServer::new();
/// let broadcast_tx = ws_server.get_broadcast_sender();
/// 
/// // Create RPC server with WebSocket broadcast support
/// let rpc_server = RpcServer::with_broadcast(db, state, broadcast_tx);
/// # }
/// ```
pub struct RpcServer {
    db: Arc<BlockchainDB>,
    state: Arc<RwLock<StateDB>>,
    validators: Arc<RwLock<ValidatorSet>>,
    subnets: Arc<RwLock<HashMap<u64, SubnetInfo>>>,
    neurons: Arc<RwLock<HashMap<(u64, u64), NeuronInfo>>>, // (subnet_id, neuron_uid) -> NeuronInfo
    weights: Arc<RwLock<HashMap<(u64, u64), Vec<WeightInfo>>>>, // (subnet_id, neuron_uid) -> weights
    broadcast_tx: Option<mpsc::UnboundedSender<BroadcastEvent>>,
}

impl RpcServer {
    /// Create a new RPC server
    pub fn new(db: Arc<BlockchainDB>, state: Arc<RwLock<StateDB>>) -> Self {
        Self { 
            db, 
            state,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx: None,
        }
    }

    /// Create a new RPC server with validator set
    pub fn with_validators(
        db: Arc<BlockchainDB>, 
        state: Arc<RwLock<StateDB>>,
        validators: Arc<RwLock<ValidatorSet>>
    ) -> Self {
        Self { 
            db, 
            state, 
            validators,
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx: None,
        }
    }

    /// Create a new RPC server with broadcast sender for WebSocket support
    /// 
    /// When transactions are submitted via `eth_sendRawTransaction`, they will be
    /// broadcast to WebSocket subscribers listening for pending transactions.
    /// 
    /// # Arguments
    /// 
    /// * `db` - Blockchain storage database
    /// * `state` - State database for account data
    /// * `broadcast_tx` - Channel sender for broadcasting events to WebSocket subscribers
    pub fn with_broadcast(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        broadcast_tx: mpsc::UnboundedSender<BroadcastEvent>,
    ) -> Self {
        Self {
            db,
            state,
            validators: Arc::new(RwLock::new(ValidatorSet::new())),
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx: Some(broadcast_tx),
        }
    }

    /// Create a new RPC server with all options
    /// 
    /// This constructor allows full customization including validator set and
    /// optional WebSocket broadcast functionality.
    /// 
    /// # Arguments
    /// 
    /// * `db` - Blockchain storage database
    /// * `state` - State database for account data
    /// * `validators` - Validator set for consensus
    /// * `broadcast_tx` - Optional channel sender for broadcasting events to WebSocket subscribers
    pub fn with_all(
        db: Arc<BlockchainDB>,
        state: Arc<RwLock<StateDB>>,
        validators: Arc<RwLock<ValidatorSet>>,
        broadcast_tx: Option<mpsc::UnboundedSender<BroadcastEvent>>,
    ) -> Self {
        Self {
            db,
            state,
            validators,
            subnets: Arc::new(RwLock::new(HashMap::new())),
            neurons: Arc::new(RwLock::new(HashMap::new())),
            weights: Arc::new(RwLock::new(HashMap::new())),
            broadcast_tx,
        }
    }

    /// Start the RPC server on the given address
    pub fn start(self, addr: &str) -> Result<Server> {
        let mut io = IoHandler::new();

        // Register blockchain query methods
        self.register_blockchain_methods(&mut io);

        // Register account methods
        self.register_account_methods(&mut io);

        // Register staking methods
        self.register_staking_methods(&mut io);

        // Register AI-specific methods
        self.register_ai_methods(&mut io);

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
        let db = self.db.clone();

        // eth_blockNumber - Get current block height
        io.add_sync_method("eth_blockNumber", move |_params: Params| {
            let height = db
                .get_best_height()
                .map_err(|_| jsonrpc_core::Error::internal_error())?
                .unwrap_or(0);
            Ok(Value::String(format!("0x{:x}", height)))
        });

        let db = self.db.clone();

        // eth_getBlockByNumber - Get block by number
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

            match db.get_block_by_height(height) {
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

        // eth_getTransactionByHash - Get transaction by hash
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
        let db = self.db.clone();
        let state = self.state.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        
        io.add_sync_method("eth_sendRawTransaction", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing transaction data"));
            }

            let tx_hex = parsed[0].trim_start_matches("0x");
            let tx_bytes = hex::decode(tx_hex)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid transaction hex"))?;

            // Decode transaction
            let tx: Transaction = bincode::deserialize(&tx_bytes)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid transaction encoding"))?;

            // Calculate transaction hash
            let tx_hash = tx.hash();
            
            // Broadcast to WebSocket subscribers if broadcast sender is available
            if let Some(ref broadcaster) = broadcast_tx {
                let rpc_tx = RpcTransaction::from(tx.clone());
                // Ignore send errors - WebSocket is optional
                let _ = broadcaster.send(BroadcastEvent::NewTransaction(rpc_tx));
            }
            
            // In a real implementation:
            // 1. Verify the transaction signature
            // 2. Check nonce and balance
            // 3. Add to mempool
            // 4. Broadcast to peers
            
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
        // lux_submitAITask - Submit AI computation task
        io.add_sync_method("lux_submitAITask", move |params: Params| {
            let _task: AITaskRequest = params.parse()?;

            // In a real implementation, we would:
            // 1. Validate the task
            // 2. Store in task queue
            // 3. Return task ID

            // For now, return a placeholder task ID
            Ok(Value::String(format!(
                "0x{}",
                hex::encode([1u8; 32])
            )))
        });

        // lux_getAIResult - Get AI task result
        io.add_sync_method("lux_getAIResult", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing task ID"));
            }

            // In a real implementation, we would:
            // 1. Look up the task result
            // 2. Return the result with status

            // For now, return null (task not found)
            Ok(Value::Null)
        });

        // lux_getValidatorStatus - Get validator information
        io.add_sync_method("lux_getValidatorStatus", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing validator address"));
            }

            // In a real implementation, we would:
            // 1. Look up validator in consensus module
            // 2. Return validator status and stake

            // For now, return null
            Ok(Value::Null)
        });
    }

    /// Register staking-related methods
    fn register_staking_methods(&self, io: &mut IoHandler) {
        let state = self.state.clone();
        let validators = self.validators.clone();

        // staking_getTotalStake - Get total stake in network
        io.add_sync_method("staking_getTotalStake", move |_params: Params| {
            let validator_set = validators.read();
            let total_stake = validator_set.total_stake();
            Ok(Value::String(format!("0x{:x}", total_stake)))
        });

        let state = self.state.clone();
        let validators = self.validators.clone();

        // staking_getStake - Get stake for specific address
        io.add_sync_method("staking_getStake", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;
            
            // Query stake from validator set
            let validator_set = validators.read();
            let stake = validator_set
                .get_validator(&address)
                .map(|v| v.stake)
                .unwrap_or(0);
                
            Ok(Value::String(format!("0x{:x}", stake)))
        });

        let validators = self.validators.clone();

        // staking_getValidators - Get list of validators
        io.add_sync_method("staking_getValidators", move |_params: Params| {
            let validator_set = validators.read();
            let validators_list: Vec<Value> = validator_set
                .validators()
                .iter()
                .map(|v| {
                    serde_json::json!({
                        "address": format!("0x{}", hex::encode(v.address.as_bytes())),
                        "stake": format!("0x{:x}", v.stake),
                        "active": v.active,
                        "rewards": format!("0x{:x}", v.rewards),
                        "publicKey": format!("0x{}", hex::encode(v.public_key)),
                    })
                })
                .collect();
                
            Ok(Value::Array(validators_list))
        });

        let validators = self.validators.clone();

        // staking_addStake - Add stake to validator
        io.add_sync_method("staking_addStake", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing address or amount",
                ));
            }

            let addr_str = parsed[0]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?;
            let address = parse_address(addr_str)?;

            let amount_str = parsed[1]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid amount"))?;
            let amount = u128::from_str_radix(amount_str.trim_start_matches("0x"), 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid amount format"))?;

            // Update stake in validator set
            let mut validator_set = validators.write();
            
            if let Some(validator) = validator_set.get_validator(&address) {
                let new_stake = validator.stake + amount;
                validator_set
                    .update_stake(&address, new_stake)
                    .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
            } else {
                // Create new validator with default public key
                let validator = Validator::new(address, amount, [0u8; 32]);
                validator_set
                    .add_validator(validator)
                    .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
            }

            Ok(Value::Bool(true))
        });

        let validators = self.validators.clone();

        // staking_removeStake - Remove stake from validator
        io.add_sync_method("staking_removeStake", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing address or amount",
                ));
            }

            let addr_str = parsed[0]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?;
            let address = parse_address(addr_str)?;

            let amount_str = parsed[1]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid amount"))?;
            let amount = u128::from_str_radix(amount_str.trim_start_matches("0x"), 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid amount format"))?;

            // Update stake in validator set
            let mut validator_set = validators.write();
            
            if let Some(validator) = validator_set.get_validator(&address) {
                if validator.stake < amount {
                    return Err(jsonrpc_core::Error::invalid_params("Insufficient stake"));
                }
                
                let new_stake = validator.stake - amount;
                if new_stake == 0 {
                    // Remove validator if stake becomes 0
                    validator_set
                        .remove_validator(&address)
                        .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
                } else {
                    validator_set
                        .update_stake(&address, new_stake)
                        .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
                }
            } else {
                return Err(jsonrpc_core::Error::invalid_params("Validator not found"));
            }

            Ok(Value::Bool(true))
        });

        let validators = self.validators.clone();

        // staking_claimRewards - Claim staking rewards
        io.add_sync_method("staking_claimRewards", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;
            
            // Get rewards from validator
            let mut validator_set = validators.write();
            
            if let Some(validator) = validator_set.get_validator(&address) {
                let rewards = validator.rewards;
                
                // Reset rewards to 0 after claiming
                validator_set.add_reward(&address, 0u128.wrapping_sub(rewards))
                    .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
                
                Ok(serde_json::json!({
                    "success": true,
                    "rewards": format!("0x{:x}", rewards)
                }))
            } else {
                Err(jsonrpc_core::Error::invalid_params("Validator not found"))
            }
        });

        let subnets = self.subnets.clone();

        // subnet_getInfo - Get subnet information
        io.add_sync_method("subnet_getInfo", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            // Query subnet from storage
            let subnets_map = subnets.read();
            
            if let Some(subnet) = subnets_map.get(&subnet_id) {
                let subnet_json = serde_json::json!({
                    "id": subnet.id,
                    "name": subnet.name,
                    "owner": subnet.owner,
                    "emission_rate": format!("0x{:x}", subnet.emission_rate),
                    "participant_count": subnet.participant_count,
                    "total_stake": format!("0x{:x}", subnet.total_stake),
                    "created_at": format!("0x{:x}", subnet.created_at),
                });
                Ok(subnet_json)
            } else {
                Ok(Value::Null)
            }
        });

        let subnets = self.subnets.clone();

        // subnet_listAll - List all subnets
        io.add_sync_method("subnet_listAll", move |_params: Params| {
            let subnets_map = subnets.read();
            
            let subnets_list: Vec<Value> = subnets_map
                .values()
                .map(|subnet| {
                    serde_json::json!({
                        "id": subnet.id,
                        "name": subnet.name,
                        "owner": subnet.owner,
                        "emission_rate": format!("0x{:x}", subnet.emission_rate),
                        "participant_count": subnet.participant_count,
                        "total_stake": format!("0x{:x}", subnet.total_stake),
                    })
                })
                .collect();
                
            Ok(Value::Array(subnets_list))
        });

        let subnets = self.subnets.clone();

        // subnet_create - Create a new subnet
        io.add_sync_method("subnet_create", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 3 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing subnet name, owner, or emission rate",
                ));
            }

            let name = parsed[0]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid name"))?
                .to_string();

            let owner = parsed[1]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid owner"))?
                .to_string();

            let emission_rate_str = parsed[2]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid emission rate"))?;
            let emission_rate = u128::from_str_radix(emission_rate_str.trim_start_matches("0x"), 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid emission rate format"))?;

            // Create new subnet
            let mut subnets_map = subnets.write();
            let subnet_id = subnets_map.len() as u64;
            
            let subnet = SubnetInfo {
                id: subnet_id,
                name,
                owner,
                emission_rate,
                participant_count: 0,
                total_stake: 0,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };

            subnets_map.insert(subnet_id, subnet);
            
            Ok(serde_json::json!({
                "success": true,
                "subnet_id": subnet_id
            }))
        });

        let neurons = self.neurons.clone();
        let subnets = self.subnets.clone();

        // neuron_getInfo - Get neuron information
        io.add_sync_method("neuron_getInfo", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing subnet ID or neuron UID",
                ));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            let neuron_uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron UID"))?;

            // Query neuron from storage
            let neurons_map = neurons.read();
            
            if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
                let neuron_json = serde_json::json!({
                    "uid": neuron.uid,
                    "address": neuron.address,
                    "subnet_id": neuron.subnet_id,
                    "stake": format!("0x{:x}", neuron.stake),
                    "trust": neuron.trust,
                    "rank": neuron.rank,
                    "incentive": neuron.incentive,
                    "dividends": neuron.dividends,
                    "active": neuron.active,
                    "endpoint": neuron.endpoint,
                });
                Ok(neuron_json)
            } else {
                Ok(Value::Null)
            }
        });

        let neurons = self.neurons.clone();

        // neuron_listBySubnet - List neurons in subnet
        io.add_sync_method("neuron_listBySubnet", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            // Query all neurons in subnet
            let neurons_map = neurons.read();
            
            let neurons_list: Vec<Value> = neurons_map
                .iter()
                .filter(|((sid, _), _)| *sid == subnet_id)
                .map(|(_, neuron)| {
                    serde_json::json!({
                        "uid": neuron.uid,
                        "address": neuron.address,
                        "stake": format!("0x{:x}", neuron.stake),
                        "active": neuron.active,
                        "rank": neuron.rank,
                    })
                })
                .collect();
                
            Ok(Value::Array(neurons_list))
        });

        let neurons = self.neurons.clone();
        let subnets = self.subnets.clone();

        // neuron_register - Register neuron on subnet
        io.add_sync_method("neuron_register", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 3 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing subnet ID, address, or stake",
                ));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            let address = parsed[1]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?
                .to_string();

            let stake_str = parsed[2]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid stake"))?;
            let stake = u128::from_str_radix(stake_str.trim_start_matches("0x"), 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid stake format"))?;

            let endpoint = parsed.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());

            // Register neuron
            let mut neurons_map = neurons.write();
            let mut subnets_map = subnets.write();
            
            // Check subnet exists
            if !subnets_map.contains_key(&subnet_id) {
                return Err(jsonrpc_core::Error::invalid_params("Subnet not found"));
            }

            // Find next UID for this subnet
            let neuron_uid = neurons_map
                .keys()
                .filter(|(sid, _)| *sid == subnet_id)
                .map(|(_, uid)| uid)
                .max()
                .map(|max_uid| max_uid + 1)
                .unwrap_or(0);

            let neuron = NeuronInfo {
                uid: neuron_uid,
                address,
                subnet_id,
                stake,
                trust: 0.0,
                rank: 0,
                incentive: 0.0,
                dividends: 0.0,
                active: true,
                endpoint,
            };

            neurons_map.insert((subnet_id, neuron_uid), neuron);
            
            // Update subnet participant count
            if let Some(subnet) = subnets_map.get_mut(&subnet_id) {
                subnet.participant_count += 1;
                subnet.total_stake += stake;
            }

            Ok(serde_json::json!({
                "success": true,
                "neuron_uid": neuron_uid
            }))
        });

        let weights = self.weights.clone();

        // weight_getWeights - Get weights for neuron
        io.add_sync_method("weight_getWeights", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing subnet ID or neuron UID",
                ));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            let neuron_uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron UID"))?;

            // Query weights from storage
            let weights_map = weights.read();
            
            if let Some(weight_list) = weights_map.get(&(subnet_id, neuron_uid)) {
                let weights_json: Vec<Value> = weight_list
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "neuron_uid": w.neuron_uid,
                            "weight": w.weight
                        })
                    })
                    .collect();
                Ok(Value::Array(weights_json))
            } else {
                Ok(Value::Array(vec![]))
            }
        });

        let weights = self.weights.clone();

        // weight_setWeights - Set weights for neuron
        io.add_sync_method("weight_setWeights", move |params: Params| {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 4 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing subnet ID, neuron UID, target UIDs, or weights",
                ));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

            let neuron_uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron UID"))?;

            let target_uids: Vec<u64> = parsed[2]
                .as_array()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid target UIDs array"))?
                .iter()
                .map(|v| v.as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid UID")))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            let weight_values: Vec<u32> = parsed[3]
                .as_array()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid weights array"))?
                .iter()
                .map(|v| v.as_u64().and_then(|n| n.try_into().ok())
                    .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid weight value")))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            if target_uids.len() != weight_values.len() {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Mismatched UIDs and weights arrays",
                ));
            }

            // Set weights
            let mut weights_map = weights.write();
            
            let weight_info: Vec<WeightInfo> = target_uids
                .into_iter()
                .zip(weight_values.into_iter())
                .map(|(uid, weight)| WeightInfo {
                    neuron_uid: uid,
                    weight,
                })
                .collect();

            weights_map.insert((subnet_id, neuron_uid), weight_info);

            Ok(serde_json::json!({
                "success": true
            }))
        });
    }
}

/// Parse block number from JSON value
fn parse_block_number(value: &serde_json::Value) -> std::result::Result<u64, jsonrpc_core::Error> {
    match value {
        serde_json::Value::String(s) => {
            if s == "latest" || s == "pending" {
                // In real implementation, get latest block
                Ok(0)
            } else if s == "earliest" {
                Ok(0)
            } else {
                let s = s.trim_start_matches("0x");
                u64::from_str_radix(s, 16)
                    .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid block number"))
            }
        }
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid block number")),
        _ => Err(jsonrpc_core::Error::invalid_params(
            "Block number must be string or number",
        )),
    }
}

/// Parse address from hex string
fn parse_address(s: &str) -> std::result::Result<Address, jsonrpc_core::Error> {
    let s = s.trim_start_matches("0x");
    let bytes = hex::decode(s)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid address format"))?;

    if bytes.len() != 20 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Address must be 20 bytes",
        ));
    }

    Ok(Address::from_slice(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::{Block, BlockHeader, Transaction};
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
        let _server = RpcServer::new(db, state);
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

    #[test]
    fn test_rpc_server_with_broadcast() {
        use crate::websocket::BroadcastEvent;
        use tokio::sync::mpsc;

        let (_temp, db, state) = create_test_setup();
        let (tx, _rx) = mpsc::unbounded_channel::<BroadcastEvent>();
        
        // Create RPC server with broadcast sender
        let server = RpcServer::with_broadcast(db, state, tx);
        
        // Verify broadcast_tx is set
        assert!(server.broadcast_tx.is_some());
    }

    #[test]
    fn test_rpc_server_without_broadcast() {
        let (_temp, db, state) = create_test_setup();
        
        // Create RPC server without broadcast sender
        let server = RpcServer::new(db, state);
        
        // Verify broadcast_tx is None
        assert!(server.broadcast_tx.is_none());
    }
}
