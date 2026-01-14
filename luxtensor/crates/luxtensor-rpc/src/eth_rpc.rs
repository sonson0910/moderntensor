// Ethereum-compatible RPC Module
// Provides eth_* methods for EVM contract deployment and interaction

use jsonrpc_core::{IoHandler, Params, Error as RpcError, ErrorCode};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::info;

/// Address type (20 bytes)
pub type Address = [u8; 20];

/// Hash type (32 bytes)
pub type TxHash = [u8; 32];

/// Pending transaction storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTransaction {
    pub hash: TxHash,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub data: Vec<u8>,
    pub gas: u64,
    pub nonce: u64,
    pub executed: bool,
    pub contract_address: Option<Address>,
    pub status: bool,
    pub gas_used: u64,
}

/// Deployed contract info
#[derive(Debug, Clone)]
pub struct DeployedContract {
    pub address: Address,
    pub code: Vec<u8>,
    pub deployer: Address,
    pub deploy_block: u64,
}

/// Transaction ready for block inclusion (with signature for production)
#[derive(Debug, Clone)]
pub struct ReadyTransaction {
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: u128,
    pub data: Vec<u8>,
    pub gas: u64,
    /// Signature R component (32 bytes)
    pub r: [u8; 32],
    /// Signature S component (32 bytes)
    pub s: [u8; 32],
    /// Signature V component (recovery id)
    pub v: u8,
}

/// EVM state for contract management
pub struct EvmState {
    pub contracts: HashMap<Address, DeployedContract>,
    pub pending_txs: HashMap<TxHash, PendingTransaction>,
    pub nonces: HashMap<Address, u64>,
    pub balances: HashMap<Address, u128>,
    pub block_number: u64,
    pub chain_id: u64,
    /// Queue of transactions ready for block inclusion
    pub tx_queue: Arc<RwLock<Vec<ReadyTransaction>>>,
}

impl EvmState {
    pub fn new(chain_id: u64) -> Self {
        let mut balances = HashMap::new();
        // Pre-fund test accounts with 1000 ETH each
        let test_accounts = [
            // Hardhat default accounts
            hex_to_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
            hex_to_address("0x70997970C51812dc3A010C7d01b50e0d17dc79C8"),
            hex_to_address("0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
        ];

        for account in test_accounts.iter().flatten() {
            balances.insert(*account, 1_000_000_000_000_000_000_000u128); // 1000 ETH
        }

        Self {
            contracts: HashMap::new(),
            pending_txs: HashMap::new(),
            nonces: HashMap::new(),
            balances,
            block_number: 1,
            chain_id,
            tx_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get and clear pending transactions for block production
    pub fn drain_tx_queue(&self) -> Vec<ReadyTransaction> {
        let mut queue = self.tx_queue.write();
        std::mem::take(&mut *queue)
    }

    /// Add transaction to queue for block inclusion
    pub fn queue_transaction(&self, tx: ReadyTransaction) {
        self.tx_queue.write().push(tx);
    }

    pub fn get_nonce(&self, address: &Address) -> u64 {
        *self.nonces.get(address).unwrap_or(&0)
    }

    pub fn increment_nonce(&mut self, address: &Address) {
        let nonce = self.nonces.entry(*address).or_insert(0);
        *nonce += 1;
    }

    pub fn get_balance(&self, address: &Address) -> u128 {
        *self.balances.get(address).unwrap_or(&0)
    }

    pub fn deploy_contract(&mut self, deployer: Address, code: Vec<u8>) -> Address {
        let nonce = self.get_nonce(&deployer);
        let contract_address = generate_contract_address(&deployer, nonce);

        let contract = DeployedContract {
            address: contract_address,
            code,
            deployer,
            deploy_block: self.block_number,
        };

        self.contracts.insert(contract_address, contract);
        contract_address
    }

    pub fn contract_exists(&self, address: &Address) -> bool {
        self.contracts.contains_key(address)
    }
}

fn hex_to_address(s: &str) -> Option<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return None;
    }
    let bytes = hex::decode(s).ok()?;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Some(addr)
}

fn address_to_hex(addr: &Address) -> String {
    format!("0x{}", hex::encode(addr))
}

fn hash_to_hex(hash: &TxHash) -> String {
    format!("0x{}", hex::encode(hash))
}

fn generate_tx_hash(from: &Address, nonce: u64) -> TxHash {
    use std::hash::{Hash as StdHash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    from.hash(&mut hasher);
    nonce.hash(&mut hasher);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .hash(&mut hasher);

    let hash_value = hasher.finish();
    let mut result = [0u8; 32];
    result[..8].copy_from_slice(&hash_value.to_be_bytes());
    result[8..16].copy_from_slice(&hash_value.to_le_bytes());
    result[16..24].copy_from_slice(&nonce.to_be_bytes());
    result
}

fn generate_contract_address(deployer: &Address, nonce: u64) -> Address {
    use std::hash::{Hash as StdHash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    deployer.hash(&mut hasher);
    nonce.hash(&mut hasher);

    let hash_value = hasher.finish();
    let mut result = [0u8; 20];
    result[..8].copy_from_slice(&hash_value.to_be_bytes());
    result[8..16].copy_from_slice(&hash_value.to_le_bytes());
    result[16..20].copy_from_slice(&(nonce as u32).to_be_bytes());
    result
}

/// Register Ethereum-compatible RPC methods
pub fn register_eth_methods(
    io: &mut IoHandler,
    evm_state: Arc<RwLock<EvmState>>,
) {
    // eth_chainId
    let state = evm_state.clone();
    io.add_sync_method("eth_chainId", move |_params: Params| {
        let chain_id = state.read().chain_id;
        Ok(json!(format!("0x{:x}", chain_id)))
    });

    // eth_blockNumber
    let state = evm_state.clone();
    io.add_sync_method("eth_blockNumber", move |_params: Params| {
        let block = state.read().block_number;
        Ok(json!(format!("0x{:x}", block)))
    });

    // eth_getBalance
    let state = evm_state.clone();
    io.add_sync_method("eth_getBalance", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing address".to_string(),
                data: None,
            })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address".to_string(),
            data: None,
        })?;

        let balance = state.read().get_balance(&address);
        Ok(json!(format!("0x{:x}", balance)))
    });

    // eth_getTransactionCount (nonce)
    let state = evm_state.clone();
    io.add_sync_method("eth_getTransactionCount", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing address".to_string(),
                data: None,
            })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address".to_string(),
            data: None,
        })?;

        let nonce = state.read().get_nonce(&address);
        Ok(json!(format!("0x{:x}", nonce)))
    });

    // eth_gasPrice
    io.add_sync_method("eth_gasPrice", move |_params: Params| {
        // Fixed gas price: 1 gwei
        Ok(json!("0x3b9aca00"))
    });

    // eth_estimateGas
    io.add_sync_method("eth_estimateGas", move |_params: Params| {
        // Default estimation for contract deploy
        Ok(json!("0x7a120")) // 500000
    });

    // eth_sendTransaction
    let state = evm_state.clone();
    io.add_sync_method("eth_sendTransaction", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let tx_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing transaction object".to_string(),
            data: None,
        })?;

        let from_str = tx_obj.get("from")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing 'from' field".to_string(),
                data: None,
            })?;

        let from = hex_to_address(from_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid 'from' address".to_string(),
            data: None,
        })?;

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

        let mut state_guard = state.write();
        let nonce = state_guard.get_nonce(&from);
        let tx_hash = generate_tx_hash(&from, nonce);

        // Execute transaction
        let (contract_address, status, gas_used) = if to.is_none() && !data.is_empty() {
            // Contract deployment
            let contract_addr = state_guard.deploy_contract(from, data.clone());
            (Some(contract_addr), true, gas / 2)
        } else if let Some(_to_addr) = to {
            // Contract call or transfer
            (None, true, 21000)
        } else {
            (None, false, 21000)
        };

        state_guard.increment_nonce(&from);
        state_guard.block_number += 1;

        // Store pending transaction
        let pending_tx = PendingTransaction {
            hash: tx_hash,
            from,
            to,
            value,
            data: data.clone(),
            gas,
            nonce,
            executed: true,
            contract_address,
            status,
            gas_used,
        };

        state_guard.pending_txs.insert(tx_hash, pending_tx);

        // Queue transaction for block production
        // NOTE: eth_sendTransaction uses empty signature - for production, use eth_sendRawTransaction with pre-signed TX
        let ready_tx = ReadyTransaction {
            nonce,
            from,
            to,
            value,
            data,
            gas,
            r: [0u8; 32], // Empty signature - will fail verification if executor requires it
            s: [0u8; 32],
            v: 0,
        };
        state_guard.queue_transaction(ready_tx);

        Ok(json!(hash_to_hex(&tx_hash)))
    });

    // eth_getTransactionReceipt
    let state = evm_state.clone();
    io.add_sync_method("eth_getTransactionReceipt", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let hash_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing transaction hash".to_string(),
                data: None,
            })?;

        let hash_str = hash_str.strip_prefix("0x").unwrap_or(hash_str);
        let hash_bytes = hex::decode(hash_str).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hash".to_string(),
            data: None,
        })?;

        let mut hash = [0u8; 32];
        let len = std::cmp::min(hash_bytes.len(), 32);
        hash[..len].copy_from_slice(&hash_bytes[..len]);

        let state_guard = state.read();

        if let Some(tx) = state_guard.pending_txs.get(&hash) {
            Ok(json!({
                "transactionHash": hash_to_hex(&tx.hash),
                "transactionIndex": "0x0",
                "blockHash": hash_to_hex(&tx.hash),
                "blockNumber": format!("0x{:x}", state_guard.block_number),
                "from": address_to_hex(&tx.from),
                "to": tx.to.as_ref().map(address_to_hex),
                "contractAddress": tx.contract_address.as_ref().map(address_to_hex),
                "cumulativeGasUsed": format!("0x{:x}", tx.gas_used),
                "gasUsed": format!("0x{:x}", tx.gas_used),
                "status": if tx.status { "0x1" } else { "0x0" },
                "logs": []
            }))
        } else {
            Ok(json!(null))
        }
    });

    // eth_call
    io.add_sync_method("eth_call", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let _call_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing call object".to_string(),
            data: None,
        })?;

        // For now, return empty for read calls
        Ok(json!("0x"))
    });

    // eth_getCode
    let state = evm_state.clone();
    io.add_sync_method("eth_getCode", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing address".to_string(),
                data: None,
            })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address".to_string(),
            data: None,
        })?;

        if state.read().contract_exists(&address) {
            if let Some(contract) = state.read().contracts.get(&address) {
                return Ok(json!(format!("0x{}", hex::encode(&contract.code))));
            }
        }
        Ok(json!("0x"))
    });

    // eth_accounts
    io.add_sync_method("eth_accounts", move |_params: Params| {
        Ok(json!([
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
            "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
        ]))
    });

    // net_version
    let state = evm_state.clone();
    io.add_sync_method("net_version", move |_params: Params| {
        let chain_id = state.read().chain_id;
        Ok(json!(chain_id.to_string()))
    });

    // eth_sendRawTransaction - process pre-signed transactions (production method)
    let state = evm_state.clone();
    io.add_sync_method("eth_sendRawTransaction", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let raw_tx = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing raw transaction".to_string(),
                data: None,
            })?;

        // Decode hex raw transaction
        let raw_tx = raw_tx.strip_prefix("0x").unwrap_or(raw_tx);
        let tx_bytes = hex::decode(raw_tx).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hex format".to_string(),
            data: None,
        })?;

        // Parse signed transaction (simplified RLP format: nonce|gasPrice|gas|to|value|data|v|r|s)
        // For production, implement full RLP decoding
        if tx_bytes.len() < 65 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Transaction too short".to_string(),
                data: None,
            });
        }

        // Extract signature from end of transaction (last 65 bytes: v(1) + r(32) + s(32))
        let sig_start = tx_bytes.len() - 65;
        let v = tx_bytes[sig_start];
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&tx_bytes[sig_start + 1..sig_start + 33]);
        s.copy_from_slice(&tx_bytes[sig_start + 33..sig_start + 65]);

        // Parse transaction fields (simplified - in production, use proper RLP)
        // Format: from(20) + nonce(8) + to(20) + value(16) + gas(8) + data_len(4) + data + sig(65)
        let min_len = 20 + 8 + 20 + 16 + 8 + 4 + 65;
        if tx_bytes.len() < min_len {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Transaction too short: need at least {} bytes", min_len),
                data: None,
            });
        }

        let mut from = [0u8; 20];
        from.copy_from_slice(&tx_bytes[0..20]);

        let nonce = u64::from_be_bytes(tx_bytes[20..28].try_into().unwrap());

        let to_bytes: [u8; 20] = tx_bytes[28..48].try_into().unwrap();
        let to = if to_bytes == [0u8; 20] {
            None
        } else {
            Some(to_bytes)
        };

        let value = u128::from_be_bytes(tx_bytes[48..64].try_into().unwrap());
        let gas = u64::from_be_bytes(tx_bytes[64..72].try_into().unwrap());
        let data_len = u32::from_be_bytes(tx_bytes[72..76].try_into().unwrap()) as usize;

        let data = if data_len > 0 && tx_bytes.len() >= 76 + data_len + 65 {
            tx_bytes[76..76 + data_len].to_vec()
        } else {
            vec![]
        };

        // Generate transaction hash
        let tx_hash = generate_tx_hash(&from, nonce);

        // Create ReadyTransaction with signature
        let ready_tx = ReadyTransaction {
            nonce,
            from,
            to,
            value,
            data: data.clone(),
            gas,
            r,
            s,
            v,
        };

        let mut state_guard = state.write();

        // Store pending transaction
        let pending_tx = PendingTransaction {
            hash: tx_hash,
            from,
            to,
            value,
            data,
            gas,
            nonce,
            executed: false,
            contract_address: None,
            status: true,
            gas_used: 0,
        };
        state_guard.pending_txs.insert(tx_hash, pending_tx);

        // Queue for block production
        state_guard.queue_transaction(ready_tx);

        info!("📥 Received signed raw transaction: {}", hash_to_hex(&tx_hash));
        Ok(json!(hash_to_hex(&tx_hash)))
    });
}

/// Register eth_getLogs and filter-related RPC methods
pub fn register_log_methods(
    io: &mut IoHandler,
    log_store: Arc<RwLock<crate::logs::LogStore>>,
    evm_state: Arc<RwLock<EvmState>>,
) {

    // eth_getLogs - Query historical logs
    let store = log_store.clone();
    let state = evm_state.clone();
    io.add_sync_method("eth_getLogs", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number;
        let logs = store.read().get_logs(&filter, current_block);

        let rpc_logs: Vec<serde_json::Value> = logs.iter()
            .map(|log| log.to_rpc_log())
            .collect();

        Ok(json!(rpc_logs))
    });

    // eth_newFilter - Create a new filter
    let store = log_store.clone();
    let state = evm_state.clone();
    io.add_sync_method("eth_newFilter", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number;
        let filter_id = store.read().new_filter(filter, current_block);

        Ok(json!(filter_id))
    });

    // eth_getFilterChanges - Get logs since last poll
    let store = log_store.clone();
    let state = evm_state.clone();
    io.add_sync_method("eth_getFilterChanges", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing filter ID".to_string(),
                data: None,
            })?;

        let current_block = state.read().block_number;
        match store.read().get_filter_changes(filter_id, current_block) {
            Some(logs) => {
                let rpc_logs: Vec<serde_json::Value> = logs.iter()
                    .map(|log| log.to_rpc_log())
                    .collect();
                Ok(json!(rpc_logs))
            }
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Filter not found".to_string(),
                data: None,
            }),
        }
    });

    // eth_getFilterLogs - Get all logs for a filter
    let store = log_store.clone();
    let state = evm_state.clone();
    io.add_sync_method("eth_getFilterLogs", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing filter ID".to_string(),
                data: None,
            })?;

        let current_block = state.read().block_number;
        let store_read = store.read();

        // For eth_getFilterLogs, we return all logs matching the original filter
        // This requires access to the filter itself
        match store_read.get_filter_changes(filter_id, current_block) {
            Some(logs) => {
                let rpc_logs: Vec<serde_json::Value> = logs.iter()
                    .map(|log| log.to_rpc_log())
                    .collect();
                Ok(json!(rpc_logs))
            }
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Filter not found".to_string(),
                data: None,
            }),
        }
    });

    // eth_uninstallFilter - Remove a filter
    let store = log_store.clone();
    io.add_sync_method("eth_uninstallFilter", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing filter ID".to_string(),
                data: None,
            })?;

        let removed = store.read().uninstall_filter(filter_id);
        Ok(json!(removed))
    });

    info!("Registered eth_getLogs and filter methods");
}

/// Parse a filter object from JSON
fn parse_log_filter(obj: &serde_json::Value) -> Result<crate::logs::LogFilter, RpcError> {
    use crate::logs::LogFilter;

    let mut filter = LogFilter::default();

    // Parse fromBlock
    if let Some(from) = obj.get("fromBlock").and_then(|v| v.as_str()) {
        filter.from_block = parse_block_number(from);
    }

    // Parse toBlock
    if let Some(to) = obj.get("toBlock").and_then(|v| v.as_str()) {
        filter.to_block = parse_block_number(to);
    }

    // Parse address (can be single address or array)
    if let Some(addr) = obj.get("address") {
        let addresses = if let Some(addr_str) = addr.as_str() {
            vec![parse_address(addr_str)?]
        } else if let Some(arr) = addr.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(parse_address)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };
        if !addresses.is_empty() {
            filter.address = Some(addresses);
        }
    }

    // Parse topics
    if let Some(topics) = obj.get("topics").and_then(|v| v.as_array()) {
        let mut topic_filters = Vec::new();
        for topic in topics {
            if topic.is_null() {
                topic_filters.push(None);
            } else if let Some(topic_str) = topic.as_str() {
                topic_filters.push(Some(vec![parse_hash(topic_str)?]));
            } else if let Some(arr) = topic.as_array() {
                let hashes: Vec<[u8; 32]> = arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| parse_hash(s).ok())
                    .collect();
                if hashes.is_empty() {
                    topic_filters.push(None);
                } else {
                    topic_filters.push(Some(hashes));
                }
            } else {
                topic_filters.push(None);
            }
        }
        if !topic_filters.is_empty() {
            filter.topics = Some(topic_filters);
        }
    }

    // Parse blockHash
    if let Some(hash) = obj.get("blockHash").and_then(|v| v.as_str()) {
        filter.block_hash = Some(parse_hash(hash)?);
    }

    Ok(filter)
}

fn parse_block_number(s: &str) -> Option<u64> {
    match s {
        "latest" | "pending" => None,
        "earliest" => Some(0),
        _ => {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u64::from_str_radix(s, 16).ok()
        }
    }
}

fn parse_address(s: &str) -> Result<luxtensor_core::types::Address, RpcError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address length".to_string(),
            data: None,
        });
    }
    let bytes = hex::decode(s).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid address hex".to_string(),
        data: None,
    })?;
    Ok(luxtensor_core::types::Address::from_slice(&bytes))
}

fn parse_hash(s: &str) -> Result<[u8; 32], RpcError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 64 {
        return Err(RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hash length".to_string(),
            data: None,
        });
    }
    let bytes = hex::decode(s).map_err(|_| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid hash hex".to_string(),
        data: None,
    })?;
    let mut result = [0u8; 32];
    result.copy_from_slice(&bytes);
    Ok(result)
}

/// Register ERC-4337 Account Abstraction RPC methods
pub fn register_aa_methods(
    io: &mut IoHandler,
    entry_point: Arc<RwLock<luxtensor_contracts::EntryPoint>>,
) {
    // eth_sendUserOperation - Submit a user operation
    let ep = entry_point.clone();
    io.add_sync_method("eth_sendUserOperation", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let user_op_json = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing user operation".to_string(),
            data: None,
        })?;

        let _entry_point_addr = p.get(1)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing entry point address".to_string(),
                data: None,
            })?;

        // Parse user operation
        let user_op = parse_user_operation(user_op_json)?;

        // Validate and queue
        let entry_point = ep.read();
        match entry_point.validate_user_op(&user_op) {
            Ok(()) => {
                let ep_addr = luxtensor_core::types::Address::from([0u8; 20]);
                let op_hash = user_op.hash(&ep_addr, 777);
                Ok(json!(format!("0x{}", hex::encode(op_hash))))
            }
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Validation failed: {}", e),
                data: None,
            }),
        }
    });

    // eth_estimateUserOperationGas - Estimate gas for user operation
    let ep = entry_point.clone();
    io.add_sync_method("eth_estimateUserOperationGas", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let user_op_json = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing user operation".to_string(),
            data: None,
        })?;

        let user_op = parse_user_operation(user_op_json)?;
        let entry_point = ep.read();

        match entry_point.estimate_user_op_gas(&user_op) {
            Ok(estimate) => Ok(json!({
                "preVerificationGas": format!("0x{:x}", estimate.pre_verification_gas),
                "verificationGasLimit": format!("0x{:x}", estimate.verification_gas),
                "callGasLimit": format!("0x{:x}", estimate.call_gas),
            })),
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Estimation failed: {}", e),
                data: None,
            }),
        }
    });

    // eth_getUserOperationReceipt - Get receipt for a user operation
    let ep = entry_point.clone();
    io.add_sync_method("eth_getUserOperationReceipt", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;

        let op_hash_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing operation hash".to_string(),
                data: None,
            })?;

        let op_hash = parse_hash(op_hash_str)?;
        let entry_point = ep.read();

        match entry_point.get_user_op_receipt(&op_hash) {
            Some(receipt) => Ok(json!({
                "userOpHash": format!("0x{}", hex::encode(receipt.user_op_hash)),
                "sender": format!("0x{}", hex::encode(receipt.sender.as_bytes())),
                "nonce": format!("0x{:x}", receipt.nonce),
                "paymaster": receipt.paymaster.map(|p| format!("0x{}", hex::encode(p.as_bytes()))),
                "actualGasUsed": format!("0x{:x}", receipt.actual_gas_used),
                "actualGasCost": format!("0x{:x}", receipt.actual_gas_cost),
                "success": receipt.success,
                "reason": receipt.reason,
                "receipt": {
                    "transactionHash": format!("0x{}", hex::encode(receipt.transaction_hash)),
                    "blockNumber": format!("0x{:x}", receipt.block_number),
                    "blockHash": format!("0x{}", hex::encode(receipt.block_hash)),
                }
            })),
            None => Ok(json!(null)),
        }
    });

    // eth_supportedEntryPoints - Get list of supported entry points
    let ep = entry_point.clone();
    io.add_sync_method("eth_supportedEntryPoints", move |_params: Params| {
        let entry_point = ep.read();
        let supported = entry_point.get_supported_entry_points();
        Ok(json!(supported))
    });

    info!("Registered ERC-4337 Account Abstraction RPC methods");
}

/// Parse a UserOperation from JSON
fn parse_user_operation(obj: &serde_json::Value) -> Result<luxtensor_contracts::UserOperation, RpcError> {
    use luxtensor_contracts::UserOperation;

    let sender = obj.get("sender")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing sender".to_string(),
            data: None,
        })?;

    let nonce = obj.get("nonce")
        .and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u128::from_str_radix(s, 16).ok()
        })
        .unwrap_or(0);

    let call_gas_limit = parse_gas_value(obj.get("callGasLimit"));
    let verification_gas_limit = parse_gas_value(obj.get("verificationGasLimit"));
    let pre_verification_gas = parse_gas_value(obj.get("preVerificationGas"));
    let max_fee_per_gas = parse_gas_value(obj.get("maxFeePerGas"));
    let max_priority_fee_per_gas = parse_gas_value(obj.get("maxPriorityFeePerGas"));

    let init_code = parse_hex_bytes(obj.get("initCode"));
    let call_data = parse_hex_bytes(obj.get("callData"));
    let paymaster_and_data = parse_hex_bytes(obj.get("paymasterAndData"));
    let signature = parse_hex_bytes(obj.get("signature"));

    Ok(UserOperation {
        sender: parse_address(sender)?,
        nonce,
        init_code,
        call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data,
        signature,
    })
}

fn parse_gas_value(val: Option<&serde_json::Value>) -> u64 {
    val.and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            u64::from_str_radix(s, 16).ok()
        })
        .unwrap_or(0)
}

fn parse_hex_bytes(val: Option<&serde_json::Value>) -> Vec<u8> {
    val.and_then(|v| v.as_str())
        .and_then(|s| {
            let s = s.strip_prefix("0x").unwrap_or(s);
            hex::decode(s).ok()
        })
        .unwrap_or_default()
}


