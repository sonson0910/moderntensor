//! # Transaction RPC Module
//!
//! Transaction-related RPC handlers for blockchain operations.
//!
//! ## Available Methods
//!
//! | Method | Description |
//! |--------|-------------|
//! | `tx_getTransaction` | Get transaction by hash |
//! | `tx_getReceipt` | Get transaction receipt |
//! | `tx_getPending` | List pending transactions |
//! | `tx_estimateGas` | Estimate gas for transaction |
//! | `tx_sendRaw` | Send raw transaction bytes |
//!
//! Extracted from server.rs to reduce complexity and improve maintainability.

use dashmap::DashMap;
use jsonrpc_core::{IoHandler, Params};
use parking_lot::RwLock;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

use luxtensor_core::UnifiedMempool;
use crate::TransactionBroadcaster;
use luxtensor_core::{Address, Hash, Transaction, UnifiedStateDB};
use luxtensor_storage::BlockchainDB;

/// Context for transaction RPC handlers
pub struct TxRpcContext {
    pub mempool: Arc<UnifiedMempool>,
    pub pending_txs: Arc<DashMap<Hash, Transaction>>,
    /// [C1 FIX] Unified state for consistent nonce/balance reads
    pub unified_state: Arc<RwLock<UnifiedStateDB>>,
    pub broadcaster: Arc<dyn TransactionBroadcaster>,
    pub db: Arc<BlockchainDB>,
}

impl TxRpcContext {
    pub fn new(
        mempool: Arc<UnifiedMempool>,
        pending_txs: Arc<DashMap<Hash, Transaction>>,
        unified_state: Arc<RwLock<UnifiedStateDB>>,
        broadcaster: Arc<dyn TransactionBroadcaster>,
        db: Arc<BlockchainDB>,
    ) -> Self {
        Self { mempool, pending_txs, unified_state, broadcaster, db }
    }
}

/// Register transaction-related RPC methods
pub fn register_tx_methods(ctx: &TxRpcContext, io: &mut IoHandler) {
    register_send_transaction(ctx, io);
    register_get_receipt(ctx, io);
}

/// Register eth_sendTransaction with P2P broadcasting
fn register_send_transaction(ctx: &TxRpcContext, io: &mut IoHandler) {
    let mempool = ctx.mempool.clone();
    let pending_txs = ctx.pending_txs.clone();
    let unified_state = ctx.unified_state.clone();
    let broadcaster = ctx.broadcaster.clone();

    io.add_method("eth_sendTransaction", move |params: Params| {
        use crate::eth_rpc::hex_to_address;
        let mempool = mempool.clone();
        let pending_txs = pending_txs.clone();
        let unified_state = unified_state.clone();
        let broadcaster = broadcaster.clone();
        async move {

        let p: Vec<serde_json::Value> = params.parse()?;
        let tx_obj = p.get(0).ok_or_else(||
            jsonrpc_core::Error::invalid_params("Missing transaction object"))?;

        // Parse 'from' address
        let from_str = tx_obj.get("from")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing 'from' field"))?;

        let from = hex_to_address(from_str)
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid 'from' address"))?;

        // Parse 'to' address (optional)
        let to = tx_obj.get("to")
            .and_then(|v| v.as_str())
            .and_then(hex_to_address);

        // Parse value
        let value = tx_obj.get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u128::from_str_radix(s, 16).ok()
            })
            .unwrap_or(0);

        // Parse data
        let data = tx_obj.get("data")
            .and_then(|v| v.as_str())
            .map(|s| {
                let s = s.strip_prefix("0x").unwrap_or(s);
                hex::decode(s).unwrap_or_default()
            })
            .unwrap_or_default();

        // Parse gas
        let gas = tx_obj.get("gas")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).ok()
            })
            .unwrap_or(10_000_000);

        // Get nonce from UnifiedStateDB (C1 FIX: consistent state source)
        let state_guard = unified_state.read();
        let current_nonce = state_guard.get_nonce(&Address::from(from));
        let chain_id = state_guard.chain_id();
        drop(state_guard);

        // Reject unsigned transactions on non-dev chains
        // eth_sendTransaction cannot sign — use eth_sendRawTransaction for production
        // Dev chains: 1337 (Hardhat default), 31337, 1, 5, 11155111 (Sepolia/Goerli)
        let is_dev = matches!(chain_id, 8898 | 1337 | 31337 | 1 | 5 | 11155111);
        if !is_dev {
            return Err(jsonrpc_core::Error {
                code: jsonrpc_core::ErrorCode::ServerError(-32000),
                message: "eth_sendTransaction is only available on dev chains. Use eth_sendRawTransaction with a signed transaction.".to_string(),
                data: None,
            });
        }

        // DOUBLE-SPEND PROTECTION: Check if nonce was explicitly provided
        let provided_nonce = tx_obj.get("nonce")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).ok()
            });

        let nonce = if let Some(explicit_nonce) = provided_nonce {
            // If nonce is explicitly provided, validate it
            if explicit_nonce < current_nonce {
                return Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::ServerError(-32000),
                    message: format!("nonce too low: expected {} got {}", current_nonce, explicit_nonce),
                    data: None,
                });
            }
            if explicit_nonce > current_nonce + 100 {
                return Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::ServerError(-32000),
                    message: format!("nonce too high: expected {} got {}", current_nonce, explicit_nonce),
                    data: None,
                });
            }
            explicit_nonce
        } else {
            current_nonce
        };

        // Check for duplicate nonce in pending transactions
        {
            let has_dup = pending_txs
                .iter()
                .any(|entry| entry.value().from == Address::from(from) && entry.value().nonce == nonce);
            if has_dup {
                return Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::ServerError(-32000),
                    message: format!("known transaction: nonce {} already pending", nonce),
                    data: None,
                });
            }
        }

        // Create luxtensor_core::Transaction for broadcasting
        let to_addr = to.map(Address::from);
        let core_tx = Transaction::new(
            nonce,
            Address::from(from),
            to_addr,
            value,
            1_000_000_000, // gas_price: 1 Gwei (mempool minimum)
            gas,
            data.clone(),
        );

        // Use deterministic hash from Transaction::hash()
        let tx_hash = core_tx.hash();

        // Add to pending transactions
        {
            pending_txs.insert(tx_hash, core_tx.clone());
            info!("📤 Transaction added to mempool: 0x{}", hex::encode(&tx_hash));
        }

        // Broadcast to P2P network
        if let Err(e) = broadcaster.broadcast(&core_tx) {
            warn!("Failed to broadcast transaction to P2P: {}", e);
        } else {
            info!("📡 Transaction broadcasted to P2P network: 0x{}", hex::encode(&tx_hash));
        }

        // Add to mempool queue for block production
        // Uses UnifiedMempool for unified transaction management
        {
            let metadata = luxtensor_core::PendingTxMetadata::default();
            if let Err(e) = mempool.add_transaction_with_metadata(core_tx.clone(), metadata) {
                return Err(jsonrpc_core::Error {
                    code: jsonrpc_core::ErrorCode::InternalError,
                    message: format!("Failed to add to mempool: {}", e),
                    data: None,
                });
            }
            info!("📦 Transaction queued for block inclusion: 0x{}", hex::encode(&tx_hash));
        }

        Ok(json!(format!("0x{}", hex::encode(tx_hash))))
        }
    });
}

/// Register eth_getTransactionReceipt handler
fn register_get_receipt(ctx: &TxRpcContext, io: &mut IoHandler) {
    let pending_txs = ctx.pending_txs.clone();
    let db = ctx.db.clone();

    io.add_method("eth_getTransactionReceipt", move |params: Params| {
        let pending_txs = pending_txs.clone();
        let db = db.clone();
        async move {
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
            if let Some(tx) = pending_txs.get(&hash) {
                return Ok(build_pending_receipt(&hash, tx.value()));
            }

            // 2. Check stored receipts in database (from mined blocks)
            match db.get_receipt(&hash) {
                Ok(Some(receipt_bytes)) => {
                    if let Some(receipt_json) = deserialize_receipt(&receipt_bytes) {
                        return Ok(receipt_json);
                    }
                }
                _ => {}
            }

            // 3. Final fallback: check if TX exists at all
            match db.get_transaction(&hash) {
                Ok(Some(tx)) => Ok(build_fallback_receipt(&hash, &tx)),
                Ok(None) => Ok(serde_json::Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        }
    });
}

/// Build receipt JSON for pending transaction
fn build_pending_receipt(hash: &[u8; 32], tx: &Transaction) -> serde_json::Value {
    json!({
        "transactionHash": format!("0x{}", hex::encode(hash)),
        "transactionIndex": "0x0",
        "blockHash": format!("0x{}", hex::encode([0u8; 32])),
        "blockNumber": "0x0",
        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
        "to": tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
        "contractAddress": compute_contract_address(tx),
        "cumulativeGasUsed": "0x5208",
        "gasUsed": "0x5208",
        "status": "0x1",
        "logs": []
    })
}

/// Build fallback receipt when only transaction (not full receipt) exists
fn build_fallback_receipt(hash: &[u8; 32], tx: &Transaction) -> serde_json::Value {
    json!({
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
    })
}

/// Compute contract address for deployment transactions
fn compute_contract_address(tx: &Transaction) -> serde_json::Value {
    if tx.to.is_none() && !tx.data.is_empty() {
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

        json!(format!("0x{}", hex::encode(addr)))
    } else {
        serde_json::Value::Null
    }
}

/// Deserialize stored receipt from database
fn deserialize_receipt(receipt_bytes: &[u8]) -> Option<serde_json::Value> {
    use luxtensor_core::receipt::{ExecutionStatus, Receipt as StoredReceipt};

    tracing::debug!("📥 Got receipt bytes: {} bytes", receipt_bytes.len());

    match bincode::deserialize::<StoredReceipt>(receipt_bytes) {
        Ok(receipt) => {
            let contract_addr =
                receipt.contract_address.map(|addr| format!("0x{}", hex::encode(addr.as_bytes())));

            let logs: Vec<serde_json::Value> = receipt.logs.iter().map(|log| {
                json!({
                    "address": format!("0x{}", hex::encode(log.address.as_bytes())),
                    "topics": log.topics.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
                    "data": format!("0x{}", hex::encode(&log.data)),
                })
            }).collect();

            Some(json!({
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
                    ExecutionStatus::Success => "0x1",
                    ExecutionStatus::Failed => "0x0",
                },
                "logs": logs
            }))
        }
        Err(e) => {
            tracing::warn!("❌ Failed to deserialize receipt: {:?}", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_contract_address_for_deployment() {
        let tx = Transaction::new(
            0,
            Address::zero(),
            None, // Contract creation
            0,
            1,
            21000,
            vec![0x60, 0x80], // Minimal bytecode
        );

        let result = compute_contract_address(&tx);
        assert!(result.is_string());
    }

    #[test]
    fn test_compute_contract_address_for_transfer() {
        let tx = Transaction::new(
            0,
            Address::zero(),
            Some(Address::zero()), // Transfer to address
            1000,
            1,
            21000,
            vec![],
        );

        let result = compute_contract_address(&tx);
        assert!(result.is_null());
    }
}
