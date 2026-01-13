// Ethereum-compatible RPC Module
// Provides eth_* methods for EVM contract deployment and interaction

use jsonrpc_core::{IoHandler, Params, Error as RpcError, ErrorCode};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

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

/// EVM state for contract management
pub struct EvmState {
    pub contracts: HashMap<Address, DeployedContract>,
    pub pending_txs: HashMap<TxHash, PendingTransaction>,
    pub nonces: HashMap<Address, u64>,
    pub balances: HashMap<Address, u128>,
    pub block_number: u64,
    pub chain_id: u64,
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
        }
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
            data,
            gas,
            nonce,
            executed: true,
            contract_address,
            status,
            gas_used,
        };

        state_guard.pending_txs.insert(tx_hash, pending_tx);

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
}
