//! Blockchain query and account RPC methods extracted from `server.rs`.
//!
//! Contains:
//! - `register_blockchain_query_methods`: eth_blockNumber, eth_getBlockByNumber,
//!   eth_getBlockByHash, eth_getTransactionByHash, eth_pendingTransactions
//! - `register_account_query_methods`: eth_getBalance, tx_getReceipt

use crate::helpers::{parse_address, parse_block_number_with_latest};
use crate::types::*;
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_core::{Hash, Transaction, UnifiedStateDB};
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use dashmap::DashMap;

/// Context for blockchain query RPC methods.
pub struct BlockchainRpcContext {
    pub db: Arc<BlockchainDB>,
    pub unified_state: Arc<RwLock<UnifiedStateDB>>,
    pub cached_block_number: Arc<AtomicU64>,
    pub pending_txs: Arc<DashMap<Hash, Transaction>>,
    pub mempool: Arc<luxtensor_core::UnifiedMempool>,
}

/// Register blockchain query methods (eth_blockNumber, eth_getBlockBy*, eth_getTransactionByHash, eth_pendingTransactions).
pub fn register_blockchain_query_methods(ctx: &BlockchainRpcContext, io: &mut IoHandler) {
    // eth_blockNumber - Get current block height (OPTIMIZED: atomic with proper ordering)
    let cached_block_num = ctx.cached_block_number.clone();
    let unified_for_block_num = ctx.unified_state.clone();
    let db_for_block_num = ctx.db.clone();
    io.add_method("eth_blockNumber", move |_params: Params| {
        let unified_for_block_num = unified_for_block_num.clone();
        let cached_block_num = cached_block_num.clone();
        let db_for_block_num = db_for_block_num.clone();
        async move {
            // Get block number from UnifiedStateDB first (source of truth)
            let unified_block = unified_for_block_num.read().block_number();
            if unified_block > 0 {
                // Update cache atomically with Release ordering for visibility
                cached_block_num.store(unified_block, Ordering::Release);
                return Ok(Value::String(format!("0x{:x}", unified_block)));
            }

            // Fallback: Check atomic cache (with Acquire for proper visibility)
            let cached = cached_block_num.load(Ordering::Acquire);
            if cached > 0 {
                return Ok(Value::String(format!("0x{:x}", cached)));
            }

            // SLOW PATH: Initialize from DB (only at startup)
            // Check genesis first — if DB is unavailable or empty, height is 0
            match db_for_block_num.get_block_by_height(0) {
                Ok(None) => return Ok(Value::String("0x0".to_string())),
                // DB error during startup → return 0x0 gracefully (node is bootstrapping)
                Err(_) => return Ok(Value::String("0x0".to_string())),
                Ok(Some(_)) => {}
            }

            // Jump search to find ceiling
            let mut ceiling: u64 = 1;
            loop {
                match db_for_block_num.get_block_by_height(ceiling) {
                    Ok(Some(_)) => {
                        ceiling *= 2;
                        if ceiling > 1_000_000 {
                            break;
                        }
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

            // Cache the result
            cached_block_num.store(low, Ordering::Relaxed);
            Ok(Value::String(format!("0x{:x}", low)))
        }
    });

    // eth_getBlockByNumber - Get block by number
    let db_for_get_block = ctx.db.clone();
    let cached_for_get_block = ctx.cached_block_number.clone();
    let unified_for_get_block = ctx.unified_state.clone();
    io.add_method("eth_getBlockByNumber", move |params: Params| {
        let db_for_get_block = db_for_get_block.clone();
        let unified_for_get_block = unified_for_get_block.clone();
        let cached_for_get_block = cached_for_get_block.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing block number"));
            }

            // Resolve "latest"/"pending" to the actual chain tip height
            let latest = {
                let ub = unified_for_get_block.read().block_number();
                if ub > 0 {
                    ub
                } else {
                    cached_for_get_block.load(Ordering::Acquire)
                }
            };
            let height = parse_block_number_with_latest(&parsed[0], latest)?;
            let _include_txs = parsed.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

            match db_for_get_block.get_block_by_height(height) {
                Ok(Some(block)) => {
                    let rpc_block = RpcBlock::from(block);
                    serde_json::to_value(rpc_block)
                        .map_err(|_| jsonrpc_core::Error::internal_error())
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        }
    });

    let db = ctx.db.clone();

    // eth_getBlockByHash - Get block by hash
    io.add_method("eth_getBlockByHash", move |params: Params| {
        let db = db.clone();
        async move {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing block hash"));
            }

            let hash_str = parsed[0].trim_start_matches("0x");
            let hash_bytes = hex::decode(hash_str)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hash format"))?;

            if hash_bytes.len() != 32 {
                return Err(jsonrpc_core::Error::invalid_params("Hash must be 32 bytes"));
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
        }
    });

    let db = ctx.db.clone();
    let pending_txs_query = ctx.pending_txs.clone();
    let mempool_for_tx_query = ctx.mempool.clone();

    // eth_getTransactionByHash - Get transaction by hash
    // 3-tier lookup: pending_txs (hot cache) → mempool → confirmed DB
    io.add_method("eth_getTransactionByHash", move |params: Params| {
        let db = db.clone();
        let pending_txs_query = pending_txs_query.clone();
        let mempool_for_tx_query = mempool_for_tx_query.clone();
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

            // 1. Check pending_txs hot cache first
            if let Some(tx) = pending_txs_query.get(&hash) {
                let rpc_tx = RpcTransaction::from(tx.value().clone());
                return serde_json::to_value(rpc_tx)
                    .map_err(|_| jsonrpc_core::Error::internal_error());
            }

            // 2. Check mempool pending transactions
            if let Some(tx) = mempool_for_tx_query.get_transaction(&hash) {
                let rpc_tx = RpcTransaction::from(tx);
                return serde_json::to_value(rpc_tx)
                    .map_err(|_| jsonrpc_core::Error::internal_error());
            }

            // 3. Fallback to confirmed transactions in database
            match db.get_transaction(&hash) {
                Ok(Some(tx)) => {
                    let rpc_tx = RpcTransaction::from(tx);
                    serde_json::to_value(rpc_tx).map_err(|_| jsonrpc_core::Error::internal_error())
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        }
    });

    // eth_pendingTransactions - Get all pending transactions from mempool
    let pending_txs_for_list = ctx.pending_txs.clone();
    io.add_method("eth_pendingTransactions", move |_params: Params| {
        let pending_txs_for_list = pending_txs_for_list.clone();
        async move {
            let rpc_txs: Vec<RpcTransaction> = pending_txs_for_list
                .iter()
                .map(|entry| RpcTransaction::from(entry.value().clone()))
                .collect();
            serde_json::to_value(rpc_txs).map_err(|_| jsonrpc_core::Error::internal_error())
        }
    });
}

/// Register account query methods (eth_getBalance, tx_getReceipt).
pub fn register_account_query_methods(
    unified_state: Arc<RwLock<UnifiedStateDB>>,
    db: Arc<BlockchainDB>,
    io: &mut IoHandler,
) {
    // eth_getBalance - Get account balance
    let unified_for_balance = unified_state.clone();
    io.add_method("eth_getBalance", move |params: Params| {
        let unified_state = unified_for_balance.clone();
        async move {
            let parsed: Vec<String> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing address"));
            }

            let address = parse_address(&parsed[0])?;

            let balance = unified_state.read().get_balance(&address);
            Ok(Value::String(format!("0x{:x}", balance)))
        }
    });

    // NOTE: eth_getTransactionCount and eth_sendRawTransaction are registered
    // in eth_rpc::register_eth_methods() with proper RLP decoding.
    // Do NOT duplicate them here — the eth_rpc versions are canonical.

    // tx_getReceipt - Get transaction receipt
    let db_for_receipt = db.clone();

    io.add_method("tx_getReceipt", move |params: Params| {
        let db = db_for_receipt.clone();
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

            // Query transaction from database
            match db.get_transaction(&hash) {
                Ok(Some(tx)) => {
                    // Try to get the real receipt from DB (bincode-serialized)
                    let (status, gas_used, logs_json, contract_addr, block_height) = match db.get_receipt(&hash) {
                        Ok(Some(receipt_bytes)) => {
                            match bincode::deserialize::<luxtensor_core::receipt::Receipt>(&receipt_bytes) {
                                Ok(r) => {
                                    let status_hex = match r.status {
                                        luxtensor_core::receipt::ExecutionStatus::Success => "0x1",
                                        luxtensor_core::receipt::ExecutionStatus::Failed => "0x0",
                                    };
                                    let logs: Vec<serde_json::Value> = r.logs.iter().map(|log| {
                                        serde_json::json!({
                                            "address": format!("0x{}", hex::encode(log.address.as_bytes())),
                                            "topics": log.topics.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
                                            "data": format!("0x{}", hex::encode(&log.data)),
                                        })
                                    }).collect();
                                    let ca = r.contract_address.map(|a| format!("0x{}", hex::encode(a.as_bytes())));
                                    (status_hex.to_string(), r.gas_used, serde_json::json!(logs), ca, r.block_height)
                                }
                                Err(_) => ("0x1".to_string(), 21000u64, serde_json::json!([]), None, 0u64),
                            }
                        }
                        _ => ("0x1".to_string(), 21000u64, serde_json::json!([]), None, 0u64),
                    };

                    let receipt = serde_json::json!({
                        "transactionHash": format!("0x{}", hex::encode(hash)),
                        "status": status,
                        "blockNumber": format!("0x{:x}", block_height),
                        "gasUsed": format!("0x{:x}", gas_used),
                        "cumulativeGasUsed": format!("0x{:x}", gas_used),
                        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                        "to": tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
                        "logs": logs_json,
                        "contractAddress": contract_addr,
                    });
                    Ok(receipt)
                }
                Ok(None) => Ok(Value::Null),
                Err(_) => Err(jsonrpc_core::Error::internal_error()),
            }
        }
    });

    // NOTE: dev_faucet is registered in eth_rpc.rs register_eth_methods()
    // which updates EvmState.balances - the source queried by eth_getBalance
}
