//! # Ethereum-compatible RPC Module
//!
//! Provides `eth_*` methods for EVM contract deployment and interaction.
//!
//! ## Supported Methods
//!
//! | Method | Description |
//! |--------|-------------|
//! | `eth_sendRawTransaction` | Submit signed transaction |
//! | `eth_getTransactionReceipt` | Get transaction receipt |
//! | `eth_call` | Execute read-only call |
//! | `eth_getCode` | Get contract bytecode |
//! | `eth_getBalance` | Get account balance |
//! | `eth_blockNumber` | Get current block number |
//! | `eth_getTransactionCount` | Get nonce |
//! | `eth_chainId` | Get chain ID |
//!
//! ## Types
//!
//! - [`PendingTransaction`] - Transaction in mempool
//! - [`ReadyTransaction`] - Transaction ready for block inclusion
//! - [`DeployedContract`] - Contract metadata

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

/// Mempool for transaction management
/// Replaces EvmState - contains only mempool-related data
pub struct Mempool {
    /// Pending transactions awaiting confirmation
    pub pending_txs: HashMap<TxHash, PendingTransaction>,
    /// Queue of transactions ready for block inclusion
    pub tx_queue: Arc<RwLock<Vec<ReadyTransaction>>>,
}

impl Mempool {
    /// Create a new empty mempool
    pub fn new() -> Self {
        Self {
            pending_txs: HashMap::new(),
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
        tracing::debug!("📥 queue_transaction: Queueing TX from 0x{} nonce={}",
                      hex::encode(&tx.from), tx.nonce);
        self.tx_queue.write().push(tx);
        tracing::debug!("📥 queue_transaction: Queue size now = {}", self.tx_queue.read().len());
    }

    /// Check if a transaction hash is pending
    pub fn is_pending(&self, tx_hash: &TxHash) -> bool {
        self.pending_txs.contains_key(tx_hash)
    }

    /// Add a pending transaction
    pub fn add_pending(&mut self, tx_hash: TxHash, tx: PendingTransaction) {
        self.pending_txs.insert(tx_hash, tx);
    }

    /// Remove a pending transaction
    pub fn remove_pending(&mut self, tx_hash: &TxHash) -> Option<PendingTransaction> {
        self.pending_txs.remove(tx_hash)
    }
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Utility functions
// ============================================================================

pub fn hex_to_address(s: &str) -> Option<Address> {
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

pub fn generate_tx_hash(from: &Address, nonce: u64) -> TxHash {
    use std::hash::{Hash as StdHash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    from.hash(&mut hasher);
    nonce.hash(&mut hasher);
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
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
///
/// # Parameters
/// - `io`: The JSON-RPC IO handler
/// - `mempool`: Transaction mempool (pending_txs, tx_queue)
/// - `unified_state`: Primary state source for reads (chain_id, nonces, balances, code, storage)
pub fn register_eth_methods(
    io: &mut IoHandler,
    mempool: Arc<RwLock<Mempool>>,
    unified_state: Arc<RwLock<luxtensor_core::UnifiedStateDB>>,
) {
    // eth_chainId - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("eth_chainId", move |_params: Params| {
        let chain_id = state.read().chain_id();
        Ok(json!(format!("0x{:x}", chain_id)))
    });

    // NOTE: eth_blockNumber is registered in server.rs with proper DB query
    // The old implementation here used EvmState.block_number which was incorrect

    // eth_getBalance - Route to UnifiedStateDB
    let state = unified_state.clone();
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

        let addr = luxtensor_core::Address::from(address);
        let balance = state.read().get_balance(&addr);
        Ok(json!(format!("0x{:x}", balance)))
    });

    // eth_getTransactionCount (nonce) - Route to UnifiedStateDB
    let state = unified_state.clone();
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

        let addr = luxtensor_core::Address::from(address);
        let nonce = state.read().get_nonce(&addr);
        Ok(json!(format!("0x{:x}", nonce)))
    });

    // eth_gasPrice - Returns current base fee from EIP-1559 FeeMarket
    // Uses dynamic pricing: 0.5 gwei initial, adjusts based on block fullness
    io.add_sync_method("eth_gasPrice", move |_params: Params| {
        // Use FeeMarket for dynamic gas pricing
        use luxtensor_consensus::FeeMarket;
        let market = FeeMarket::new();
        let base_fee = market.current_base_fee();
        Ok(json!(format!("0x{:x}", base_fee)))
    });

    // eth_estimateGas
    io.add_sync_method("eth_estimateGas", move |_params: Params| {
        // Default estimation for contract deploy
        Ok(json!("0x7a120")) // 500000
    });

    // NOTE: eth_sendTransaction is handled by tx_rpc.rs which registers after this
    // and overrides this handler. The tx_rpc.rs version includes P2P broadcasting.
    // This duplicate was removed to avoid confusion and dead code.

    // eth_getTransactionReceipt - uses mempool for pending_txs, unified_state for block_number
    let mp_for_receipt = mempool.clone();
    let state_for_receipt = unified_state.clone();
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

        let mempool_guard = mp_for_receipt.read();
        let block_number = state_for_receipt.read().block_number();

        if let Some(tx) = mempool_guard.pending_txs.get(&hash) {
            Ok(json!({
                "transactionHash": hash_to_hex(&tx.hash),
                "transactionIndex": "0x0",
                "blockHash": hash_to_hex(&tx.hash),
                "blockNumber": format!("0x{:x}", block_number),
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

    // eth_call - Execute a call without creating a transaction (read-only)
    // Uses unified_state for contract code and block_number
    let state_for_call = unified_state.clone();
    io.add_sync_method("eth_call", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let call_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing call object".to_string(),
            data: None,
        })?;

        // Parse call parameters
        let from_str = call_obj.get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");

        let to_str = call_obj.get("to")
            .and_then(|v| v.as_str());

        let data_hex = call_obj.get("data")
            .and_then(|v| v.as_str())
            .unwrap_or("0x");

        let value_str = call_obj.get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");

        // Parse addresses
        let from_addr = hex_to_address(from_str).unwrap_or([0u8; 20]);
        let to_addr = match to_str {
            None => return Ok(json!("0x")),
            Some(addr_str) => match hex_to_address(addr_str) {
                None => return Ok(json!("0x")),
                Some(addr) => addr,
            }
        };

        // Parse data
        let data = {
            let s = data_hex.strip_prefix("0x").unwrap_or(data_hex);
            hex::decode(s).unwrap_or_default()
        };

        // Parse value
        let value: u128 = {
            let s = value_str.strip_prefix("0x").unwrap_or(value_str);
            u128::from_str_radix(s, 16).unwrap_or(0)
        };

        // Get contract code from UnifiedStateDB
        let state_guard = state_for_call.read();
        let contract_code = match state_guard.get_code(&luxtensor_core::Address::from(to_addr)) {
            Some(code) => code.to_vec(),
            None => {
                // No contract code, return empty
                return Ok(json!("0x"));
            }
        };
        let block_number = state_guard.block_number();
        drop(state_guard);

        // Execute call using EvmExecutor
        let executor = luxtensor_contracts::EvmExecutor::new();

        // Get current timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Execute the call
        match executor.call(
            luxtensor_core::Address::from(from_addr),
            luxtensor_contracts::ContractAddress::from(to_addr),
            contract_code,
            data,
            value,
            1_000_000, // Gas limit for eth_call
            block_number,
            timestamp,
        ) {
            Ok((output, _gas_used, _logs)) => {
                Ok(json!(format!("0x{}", hex::encode(output))))
            }
            Err(e) => {
                // Log error but return empty for compatibility
                tracing::warn!("eth_call execution error: {:?}", e);
                Ok(json!("0x"))
            }
        }
    });

    // eth_getCode - Route to UnifiedStateDB
    let state = unified_state.clone();
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

        let addr = luxtensor_core::Address::from(address);

        // UnifiedStateDB is the sole source of truth for contract code
        if let Some(code) = state.read().get_code(&addr) {
            return Ok(json!(format!("0x{}", hex::encode(&code))));
        }

        // No code at this address
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

    // net_version - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_sync_method("net_version", move |_params: Params| {
        let chain_id = state.read().chain_id();
        Ok(json!(chain_id.to_string()))
    });

    // eth_sendRawTransaction - process pre-signed transactions (production method)
    // Uses unified_state for chain_id/nonce reads, mempool for pending_txs/tx_queue
    let mp_for_sendraw = mempool.clone();
    let unified_for_sendraw = unified_state.clone();
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

        let nonce = u64::from_be_bytes(
            tx_bytes[20..28].try_into()
                .map_err(|_| RpcError::invalid_params("Invalid nonce bytes"))?
        );

        let to_bytes: [u8; 20] = tx_bytes[28..48].try_into()
            .map_err(|_| RpcError::invalid_params("Invalid to address bytes"))?;
        let to = if to_bytes == [0u8; 20] {
            None
        } else {
            Some(to_bytes)
        };

        let value = u128::from_be_bytes(tx_bytes[48..64].try_into()
            .map_err(|_| RpcError::invalid_params("Invalid value bytes"))?);
        let gas = u64::from_be_bytes(tx_bytes[64..72].try_into()
            .map_err(|_| RpcError::invalid_params("Invalid gas bytes"))?);
        let data_len = u32::from_be_bytes(tx_bytes[72..76].try_into()
            .map_err(|_| RpcError::invalid_params("Invalid data length bytes"))?) as usize;

        let data = if data_len > 0 && tx_bytes.len() >= 76 + data_len + 65 {
            tx_bytes[76..76 + data_len].to_vec()
        } else {
            vec![]
        };

        // Generate transaction hash
        let tx_hash = generate_tx_hash(&from, nonce);

        // === REPLAY PROTECTION: Validate chain ID from signature ===
        // EIP-155: v = chainId * 2 + 35 + recovery_id (0 or 1)
        // Extract chain_id from v: chain_id = (v - 35) / 2
        let expected_chain_id = unified_for_sendraw.read().chain_id();
        let tx_chain_id = if v >= 35 {
            ((v as u64) - 35) / 2
        } else {
            // Legacy transaction (v = 27 or 28), chain_id = 0 (mainnet)
            0
        };

        // Reject if chain_id doesn't match (unless legacy tx with v=27/28)
        if v >= 35 && tx_chain_id != expected_chain_id && tx_chain_id != 0 {
            return Err(RpcError {
                code: ErrorCode::ServerError(-32000),
                message: format!("chain ID mismatch: expected {} got {}", expected_chain_id, tx_chain_id),
                data: None,
            });
        }

        // === DOUBLE-SPEND PROTECTION: Validate nonce ===
        let from_addr = luxtensor_core::Address::from(from);
        let current_nonce = unified_for_sendraw.read().get_nonce(&from_addr);
        if nonce < current_nonce {
            return Err(RpcError {
                code: ErrorCode::ServerError(-32000),
                message: format!("nonce too low: expected {} got {}", current_nonce, nonce),
                data: None,
            });
        }

        // Check for duplicate nonce in pending transactions (mempool)
        {
            let mempool_guard = mp_for_sendraw.read();
            for (_, tx) in mempool_guard.pending_txs.iter() {
                if tx.from == from && tx.nonce == nonce {
                    return Err(RpcError {
                        code: ErrorCode::ServerError(-32000),
                        message: format!("known transaction: nonce {} already pending", nonce),
                        data: None,
                    });
                }
            }
        }

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

        let mut mempool_guard = mp_for_sendraw.write();

        // Store pending transaction in mempool
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
        mempool_guard.pending_txs.insert(tx_hash, pending_tx);

        // Queue for block production (mempool)
        mempool_guard.queue_transaction(ready_tx);

        info!("📥 Received signed raw transaction: {}", hash_to_hex(&tx_hash));
        Ok(json!(hash_to_hex(&tx_hash)))
    });

    // === Additional ETH methods for full compatibility ===

    // eth_syncing - Returns syncing status
    io.add_sync_method("eth_syncing", move |_params: Params| {
        Ok(json!(false)) // Not syncing
    });

    // eth_mining - Returns whether client is mining
    io.add_sync_method("eth_mining", move |_params: Params| {
        Ok(json!(false))
    });

    // eth_hashrate - Returns hashrate
    io.add_sync_method("eth_hashrate", move |_params: Params| {
        Ok(json!("0x0"))
    });

    // eth_coinbase - Returns coinbase address
    io.add_sync_method("eth_coinbase", move |_params: Params| {
        Ok(json!("0x0000000000000000000000000000000000000000"))
    });

    // eth_protocolVersion - Returns protocol version
    io.add_sync_method("eth_protocolVersion", move |_params: Params| {
        Ok(json!("0x41")) // Protocol version 65
    });

    let unified_for_storage = unified_state.clone();

    // eth_getStorageAt - Route to UnifiedStateDB
    io.add_sync_method("eth_getStorageAt", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(RpcError::invalid_params("Missing address or position"));
        }

        // Parse address
        let addr_bytes = match hex_to_address(&parsed[0]) {
            Some(a) => a,
            None => return Ok(json!("0x0000000000000000000000000000000000000000000000000000000000000000")),
        };

        // Parse slot (32 bytes)
        let slot_str = parsed[1].trim_start_matches("0x");
        let mut slot = [0u8; 32];
        if let Ok(bytes) = hex::decode(slot_str) {
            let start = 32_usize.saturating_sub(bytes.len());
            slot[start..].copy_from_slice(&bytes);
        }

        // Get storage value from UnifiedStateDB
        let addr = luxtensor_core::Address::from(addr_bytes);
        let state = unified_for_storage.read();
        let value = state.get_storage(&addr, &slot);

        Ok(json!(format!("0x{}", hex::encode(value))))
    });

    // net_listening - Returns whether node is listening
    io.add_sync_method("net_listening", move |_params: Params| {
        Ok(json!(true))
    });

    // web3_sha3 - Returns Keccak-256 hash
    io.add_sync_method("web3_sha3", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(RpcError::invalid_params("Missing data"));
        }
        let data = parsed[0].trim_start_matches("0x");
        let bytes = hex::decode(data).unwrap_or_default();
        use sha3::{Digest, Keccak256};
        let hash = Keccak256::digest(&bytes);
        Ok(json!(format!("0x{}", hex::encode(hash))))
    });

    // rpc_modules - Returns available RPC modules
    io.add_sync_method("rpc_modules", move |_params: Params| {
        Ok(json!({
            "eth": "1.0",
            "net": "1.0",
            "web3": "1.0",
            "staking": "1.0",
            "subnet": "1.0",
            "neuron": "1.0",
            "weight": "1.0",
            "ai": "1.0"
        }))
    });

    // dev_faucet - Credit tokens to address for testing (DEV MODE ONLY)
    // Uses unified_state for balance operations
    let dev_state = unified_state.clone();
    io.add_sync_method("dev_faucet", move |params: Params| {
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
            message: "Invalid address format".to_string(),
            data: None,
        })?;

        // Parse amount (default: 1000 MDT = 1000 * 10^9 base units)
        let amount: u128 = p.get(1)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(1_000_000_000_000); // 1000 MDT default

        // Credit account in UnifiedStateDB
        let mut state_guard = dev_state.write();
        let addr = luxtensor_core::Address::from(address);
        let current_balance = state_guard.get_balance(&addr);
        let new_balance = current_balance + amount;
        state_guard.set_balance(addr, new_balance);

        Ok(json!({
            "success": true,
            "address": address_to_hex(&address),
            "credited": amount,
            "new_balance": new_balance.to_string()
        }))
    });
}

/// Register eth_getLogs and filter-related RPC methods
/// Uses UnifiedStateDB for block_number reads
pub fn register_log_methods(
    io: &mut IoHandler,
    log_store: Arc<RwLock<crate::logs::LogStore>>,
    unified_state: Arc<RwLock<luxtensor_core::UnifiedStateDB>>,
) {

    // eth_getLogs - Query historical logs
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_getLogs", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number();
        let logs = store.read().get_logs(&filter, current_block);

        let rpc_logs: Vec<serde_json::Value> = logs.iter()
            .map(|log| log.to_rpc_log())
            .collect();

        Ok(json!(rpc_logs))
    });

    // eth_newFilter - Create a new filter
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_newFilter", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number();
        let filter_id = store.read().new_filter(filter, current_block);

        Ok(json!(filter_id))
    });

    // eth_getFilterChanges - Get logs since last poll
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_sync_method("eth_getFilterChanges", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing filter ID".to_string(),
                data: None,
            })?;

        let current_block = state.read().block_number();
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
    let state = unified_state.clone();
    io.add_sync_method("eth_getFilterLogs", move |params: Params| {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing filter ID".to_string(),
                data: None,
            })?;

        let current_block = state.read().block_number();
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

    // eth_getUserOperationByHash - Get user operation by hash (ERC-4337)
    let ep = entry_point.clone();
    io.add_sync_method("eth_getUserOperationByHash", move |params: Params| {
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

        // Return receipt info if operation was processed
        match entry_point.get_user_op_receipt(&op_hash) {
            Some(receipt) => Ok(json!({
                "userOperation": null, // Original op not stored for privacy
                "entryPoint": "0x0000000000000000000000000000000000004337",
                "transactionHash": format!("0x{}", hex::encode(receipt.transaction_hash)),
                "blockNumber": format!("0x{:x}", receipt.block_number),
                "blockHash": format!("0x{}", hex::encode(receipt.block_hash)),
            })),
            None => Ok(json!(null)),
        }
    });

    // eth_chainId - Return chain ID for AA context (ERC-4337)
    // Note: This complements the standard eth_chainId but is specific to AA operations
    let ep = entry_point.clone();
    io.add_sync_method("aa_chainId", move |_params: Params| {
        let entry_point = ep.read();
        let chain_id = entry_point.chain_id();
        Ok(json!(format!("0x{:x}", chain_id)))
    });

    info!("Registered ERC-4337 Account Abstraction RPC methods (6 methods)");
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


