use crate::{types::*, RpcError, Result};
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_core::{Address, StateDB, Transaction};
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;

/// JSON-RPC server for LuxTensor blockchain
pub struct RpcServer {
    db: Arc<BlockchainDB>,
    state: Arc<RwLock<StateDB>>,
    // In-memory storage for AI tasks and validator info
    // In a production system, these would be stored in a persistent database
    ai_tasks: Arc<RwLock<HashMap<String, AITaskResult>>>,
    mempool_txs: Arc<RwLock<HashMap<[u8; 32], Transaction>>>,
}

impl RpcServer {
    /// Create a new RPC server
    pub fn new(db: Arc<BlockchainDB>, state: Arc<RwLock<StateDB>>) -> Self {
        Self { 
            db, 
            state,
            ai_tasks: Arc::new(RwLock::new(HashMap::new())),
            mempool_txs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the RPC server on the given address
    pub fn start(self, addr: &str) -> Result<Server> {
        let mut io = IoHandler::new();

        // Register blockchain query methods
        self.register_blockchain_methods(&mut io);

        // Register account methods
        self.register_account_methods(&mut io);

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

        // lux_blockNumber - Get current block height
        io.add_sync_method("lux_blockNumber", move |_params: Params| {
            let height = db
                .get_best_height()
                .map_err(|_| jsonrpc_core::Error::internal_error())?
                .unwrap_or(0);
            Ok(Value::String(format!("0x{:x}", height)))
        });

        let db = self.db.clone();

        // lux_getBlockByNumber - Get block by number
        io.add_sync_method("lux_getBlockByNumber", move |params: Params| {
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

        // lux_getBlockByHash - Get block by hash
        io.add_sync_method("lux_getBlockByHash", move |params: Params| {
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

        // lux_getTransactionByHash - Get transaction by hash
        io.add_sync_method("lux_getTransactionByHash", move |params: Params| {
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

        // lux_getBalance - Get account balance
        io.add_sync_method("lux_getBalance", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;

            let balance = state.read().get_balance(&address);
            Ok(Value::String(format!("0x{:x}", balance)))
        });

        let state = self.state.clone();

        // lux_getTransactionCount - Get account nonce
        io.add_sync_method("lux_getTransactionCount", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;

            let nonce = state.read().get_nonce(&address);
            Ok(Value::String(format!("0x{:x}", nonce)))
        });

        let mempool_txs = self.mempool_txs.clone();
        let state = self.state.clone();

        // lux_sendRawTransaction - Submit raw signed transaction
        io.add_sync_method("lux_sendRawTransaction", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing transaction data"));
            }

            // Decode the raw transaction from hex
            let raw_tx_hex = parsed[0].trim_start_matches("0x");
            let raw_tx_bytes = hex::decode(raw_tx_hex)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex encoding"))?;

            // Deserialize transaction using bincode
            let tx: Transaction = bincode::deserialize(&raw_tx_bytes)
                .map_err(|e| jsonrpc_core::Error::invalid_params(
                    format!("Failed to decode transaction: {}", e)
                ))?;

            // Verify the signature
            tx.verify_signature()
                .map_err(|e| jsonrpc_core::Error::invalid_params(
                    format!("Invalid signature: {}", e)
                ))?;

            // Verify nonce matches expected nonce
            let expected_nonce = state.read().get_nonce(&tx.from);
            if tx.nonce != expected_nonce {
                return Err(jsonrpc_core::Error::invalid_params(
                    format!("Invalid nonce: expected {}, got {}", expected_nonce, tx.nonce)
                ));
            }

            // Verify sender has enough balance for value + gas
            let balance = state.read().get_balance(&tx.from);
            let total_cost = tx.value + (tx.gas_limit as u128 * tx.gas_price as u128);
            if balance < total_cost {
                return Err(jsonrpc_core::Error::invalid_params(
                    format!("Insufficient balance: have {}, need {}", balance, total_cost)
                ));
            }

            // Calculate transaction hash
            let tx_hash = tx.hash();

            // Add to mempool (in-memory for now)
            let mut mempool = mempool_txs.write();
            if mempool.contains_key(&tx_hash) {
                return Err(jsonrpc_core::Error::invalid_params("Duplicate transaction"));
            }
            
            mempool.insert(tx_hash, tx);

            // Return transaction hash
            Ok(Value::String(format!("0x{}", hex::encode(tx_hash))))
        });
    }

    /// Register AI-specific methods
    fn register_ai_methods(&self, io: &mut IoHandler) {
        let ai_tasks = self.ai_tasks.clone();
        let state = self.state.clone();

        // lux_submitAITask - Submit AI computation task
        io.add_sync_method("lux_submitAITask", move |params: Params| {
            let task: AITaskRequest = params.parse()?;

            // Validate the requester address
            let requester_address = parse_address(&task.requester)?;
            
            // Parse reward amount
            let reward_str = task.reward.trim_start_matches("0x");
            let reward = u128::from_str_radix(reward_str, 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid reward amount"))?;

            // Verify requester has sufficient balance
            let balance = state.read().get_balance(&requester_address);
            if balance < reward {
                return Err(jsonrpc_core::Error::invalid_params(
                    format!("Insufficient balance for reward: have {}, need {}", balance, reward)
                ));
            }

            // Generate task ID from task data
            let task_id = {
                use luxtensor_crypto::keccak256;
                let mut data = Vec::new();
                data.extend_from_slice(task.model_hash.as_bytes());
                data.extend_from_slice(task.input_data.as_bytes());
                data.extend_from_slice(task.requester.as_bytes());
                data.extend_from_slice(&reward.to_le_bytes());
                // Use a counter or nonce from the requester's account to ensure uniqueness
                // For now, we use the hash of all inputs which makes tasks with identical 
                // parameters have the same ID (idempotent)
                keccak256(&data)
            };

            // Store task with pending status
            let task_result = AITaskResult {
                task_id: format!("0x{}", hex::encode(task_id)),
                result_data: String::new(), // Empty until completed
                worker: String::new(), // No worker assigned yet
                status: "pending".to_string(),
            };

            let mut tasks = ai_tasks.write();
            tasks.insert(format!("0x{}", hex::encode(task_id)), task_result);

            // Return task ID
            Ok(Value::String(format!("0x{}", hex::encode(task_id))))
        });

        let ai_tasks = self.ai_tasks.clone();

        // lux_getAIResult - Get AI task result
        io.add_sync_method("lux_getAIResult", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing task ID"));
            }

            let task_id = &parsed[0];
            
            // Look up the task result
            let tasks = ai_tasks.read();
            match tasks.get(task_id) {
                Some(result) => {
                    serde_json::to_value(result)
                        .map_err(|_| jsonrpc_core::Error::internal_error())
                }
                None => Ok(Value::Null),
            }
        });

        let state = self.state.clone();

        // lux_getValidatorStatus - Get validator information
        io.add_sync_method("lux_getValidatorStatus", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing validator address"));
            }

            // Validate address format
            let validator_address = parse_address(&parsed[0])?;

            // Get validator stake from state
            // In a full implementation, this would query the consensus module
            // For now, we check if the account has sufficient balance to be a validator
            let balance = state.read().get_balance(&validator_address);
            
            // Minimum stake requirement (32 tokens with 18 decimals)
            let min_stake: u128 = 32_000_000_000_000_000_000;
            let is_active = balance >= min_stake;

            let status = ValidatorStatus {
                address: format!("0x{}", hex::encode(validator_address.as_bytes())),
                stake: format!("0x{:x}", balance),
                active: is_active,
            };

            serde_json::to_value(status)
                .map_err(|_| jsonrpc_core::Error::internal_error())
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
    use luxtensor_crypto::KeyPair;
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
    fn test_send_raw_transaction_encoding() {
        // Test that we can encode and decode a transaction
        let keypair = KeyPair::generate();
        let from = Address::from_slice(&keypair.address());
        
        let mut tx = Transaction::new(
            0,
            from,
            Some(Address::zero()),
            1000,
            1,
            21000,
            vec![],
        );

        // Sign the transaction
        let message = tx.signing_message();
        let message_hash = luxtensor_crypto::keccak256(&message);
        let signature = keypair.sign(&message_hash);
        
        tx.r.copy_from_slice(&signature[..32]);
        tx.s.copy_from_slice(&signature[32..]);
        // Note: In production, recovery ID (v) should be properly determined during signing.
        // For this encoding/decoding test, we use 0 as a placeholder.
        tx.v = 0;

        // Encode transaction
        let encoded = bincode::serialize(&tx).unwrap();
        let hex_encoded = hex::encode(&encoded);

        // Decode transaction
        let decoded_bytes = hex::decode(&hex_encoded).unwrap();
        let decoded_tx: Transaction = bincode::deserialize(&decoded_bytes).unwrap();

        // Verify the decoded transaction matches
        assert_eq!(decoded_tx.nonce, tx.nonce);
        assert_eq!(decoded_tx.value, tx.value);
        assert_eq!(decoded_tx.from.as_bytes(), tx.from.as_bytes());
    }

    #[test]
    fn test_mempool_transaction_storage() {
        let (_temp, db, state) = create_test_setup();
        let server = RpcServer::new(db, state);

        // Create a test transaction
        let tx = Transaction::new(
            0,
            Address::zero(),
            Some(Address::zero()),
            1000,
            1,
            21000,
            vec![],
        );

        let tx_hash = tx.hash();

        // Add to mempool
        server.mempool_txs.write().insert(tx_hash, tx.clone());

        // Verify it's stored
        let stored_tx = server.mempool_txs.read().get(&tx_hash).cloned();
        assert!(stored_tx.is_some());
        assert_eq!(stored_tx.unwrap().nonce, tx.nonce);
    }

    #[test]
    fn test_ai_task_storage() {
        let (_temp, db, state) = create_test_setup();
        let server = RpcServer::new(db, state);

        // Create a test task
        let task_id = "0x1234567890abcdef";
        let task_result = AITaskResult {
            task_id: task_id.to_string(),
            result_data: String::new(),
            worker: String::new(),
            status: "pending".to_string(),
        };

        // Store task
        server.ai_tasks.write().insert(task_id.to_string(), task_result.clone());

        // Verify it's stored
        let stored_task = server.ai_tasks.read().get(task_id).cloned();
        assert!(stored_task.is_some());
        assert_eq!(stored_task.unwrap().status, "pending");
    }

    #[test]
    fn test_validator_status_check() {
        let (_temp, db, state) = create_test_setup();
        let mut state_write = state.write();
        
        // Create an address with sufficient balance to be a validator
        let addr = Address::from_slice(&[1u8; 20]);
        let min_stake: u128 = 32_000_000_000_000_000_000;
        
        let account = luxtensor_core::Account::with_balance(min_stake);
        state_write.set_account(addr, account);
        drop(state_write);

        // Check balance is set correctly
        let balance = state.read().get_balance(&addr);
        assert_eq!(balance, min_stake);
        assert!(balance >= min_stake);
    }
}
