use crate::{types::*, RpcError, Result};
use jsonrpc_core::{IoHandler, Params, Value};
use jsonrpc_http_server::{Server, ServerBuilder};
use luxtensor_core::{Address, StateDB};
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;

/// JSON-RPC server for LuxTensor blockchain
pub struct RpcServer {
    db: Arc<BlockchainDB>,
    state: Arc<RwLock<StateDB>>,
}

impl RpcServer {
    /// Create a new RPC server
    pub fn new(db: Arc<BlockchainDB>, state: Arc<RwLock<StateDB>>) -> Self {
        Self { db, state }
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
        io.add_sync_method("eth_sendRawTransaction", move |params: Params| {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing transaction data"));
            }

            // In a real implementation, we would:
            // 1. Decode the raw transaction
            // 2. Verify the signature
            // 3. Add to mempool
            // 4. Return transaction hash

            // For now, return a placeholder
            Ok(Value::String(format!(
                "0x{}",
                hex::encode([0u8; 32])
            )))
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
}
