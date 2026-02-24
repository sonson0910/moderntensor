//! # Ethereum RPC Method Registration
//!
//! Contains `register_eth_methods`, `register_log_methods`, and `register_aa_methods`.
//! These functions register Ethereum-compatible JSON-RPC handlers.

use std::sync::Arc;
use parking_lot::{Mutex, RwLock};
use jsonrpc_core::{IoHandler, Params, Error as RpcError, ErrorCode};
use serde_json::json;
use tracing::info;

use super::{hex_to_address, address_to_hex, hash_to_hex};
use super::rlp_decoder::decode_rlp_transaction;
use super::faucet::{FaucetRpcConfig, FaucetRateLimiter};
/// Register Ethereum-compatible RPC methods
///
/// # Parameters
/// - `io`: The JSON-RPC IO handler
/// - `mempool`: Transaction mempool (pending_txs, tx_queue)
/// - `unified_state`: Primary state source for reads (chain_id, nonces, balances, code, storage)
/// - `broadcaster`: P2P transaction broadcaster for relaying RPC-submitted transactions
/// - `evm_executor`: Shared EVM executor from block execution (for eth_call storage reads)
pub fn register_eth_methods(
    io: &mut IoHandler,
    mempool: Arc<luxtensor_core::UnifiedMempool>,
    unified_state: Arc<RwLock<luxtensor_core::UnifiedStateDB>>,
    db: Arc<luxtensor_storage::BlockchainDB>,
    broadcaster: Arc<dyn crate::TransactionBroadcaster>,
    evm_executor: Option<luxtensor_contracts::EvmExecutor>,
    faucet_config: FaucetRpcConfig,
) {
    // eth_chainId - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_method("eth_chainId", move |_params: Params| {
        let state = state.clone();
        async move {
            let chain_id = state.read().chain_id();
            Ok(json!(format!("0x{:x}", chain_id)))
        }
    });

    // NOTE: eth_blockNumber is registered in server.rs with proper DB query
    // The old implementation here used EvmState.block_number which was incorrect

    // eth_getBalance - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_method("eth_getBalance", move |params: Params| {
        let state = state.clone();
        async move {
            let p: Vec<serde_json::Value> = params.parse()?;
            let address_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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
        }
    });

    // eth_getTransactionCount (nonce) - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_method("eth_getTransactionCount", move |params: Params| {
        let state = state.clone();
        async move {
            let p: Vec<serde_json::Value> = params.parse()?;
            let address_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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
        }
    });

    // eth_gasPrice - Returns current base fee from EIP-1559 FeeMarket
    // Uses dynamic pricing: 0.5 gwei initial, adjusts based on block fullness
    io.add_method("eth_gasPrice", move |_params: Params| { async move {
        // Use FeeMarket for dynamic gas pricing
        use luxtensor_consensus::FeeMarket;
        let market = FeeMarket::new();
        let base_fee = market.current_base_fee();
        Ok(json!(format!("0x{:x}", base_fee)))
    }});

    // eth_estimateGas — real EVM dry-run simulation (non-committing)
    let state_for_estimate = unified_state.clone();
    io.add_method("eth_estimateGas", move |params: Params| {
        let state_for_estimate = state_for_estimate.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;

        // Parse call params (same format as eth_call)
        let call_obj = match p.get(0) {
            Some(obj) => obj,
            None => {
                // No params → simple transfer estimate
                return Ok(json!("0x5208")); // 21000
            }
        };

        let from_str = call_obj
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");
        let to_str = call_obj.get("to").and_then(|v| v.as_str());
        let data_hex = call_obj.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
        let value_str = call_obj.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");

        let from_addr = hex_to_address(from_str).ok_or_else(|| {
            jsonrpc_core::Error::invalid_params(format!(
                "Invalid 'from' address format: {}",
                from_str
            ))
        })?;
        let data = {
            let s = data_hex.strip_prefix("0x").unwrap_or(data_hex);
            hex::decode(s).unwrap_or_default()
        };
        let value: u128 = {
            let s = value_str.strip_prefix("0x").unwrap_or(value_str);
            u128::from_str_radix(s, 16).unwrap_or(0)
        };

        // Simple transfer (no data, has to address)
        if data.is_empty() && to_str.is_some() {
            return Ok(json!("0x5208")); // 21_000
        }

        // Contract creation or call — use gas estimation formula
        let calldata_gas = luxtensor_contracts::revm_integration::estimate_calldata_gas(&data);
        let is_create = to_str.is_none();

        let base_gas: u64 = 21_000;
        let create_gas: u64 = if is_create { 32_000 } else { 0 };

        // If we have contract code, try dry-run via EvmExecutor
        if let Some(to_addr_str) = to_str {
            if let Some(to_addr) = hex_to_address(to_addr_str) {
                let state_guard = state_for_estimate.read();
                let code = state_guard.get_code(&luxtensor_core::Address::from(to_addr));
                let block_number = state_guard.block_number();
                drop(state_guard);

                if let Some(contract_code) = code {
                    // Create executor seeded with state from UnifiedStateDB
                    let executor = luxtensor_contracts::EvmExecutor::default();
                    // Fund the caller and deploy code so the EVM sees the correct state
                    {
                        let state_r = state_for_estimate.read();
                        let caller_balance =
                            state_r.get_balance(&luxtensor_core::Address::from(from_addr));
                        executor.fund_account(
                            &luxtensor_core::Address::from(from_addr),
                            caller_balance,
                        );
                        executor.deploy_code(
                            &luxtensor_core::Address::from(to_addr),
                            contract_code.clone(),
                        );
                    }
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    match executor.call(
                        luxtensor_core::Address::from(from_addr),
                        luxtensor_contracts::ContractAddress::from(to_addr),
                        contract_code.to_vec(),
                        data.clone(),
                        value,
                        30_000_000, // High gas limit for estimation
                        block_number,
                        timestamp,
                        1, // gas_price for estimation
                    ) {
                        Ok((_output, gas_used, _logs)) => {
                            // Add 15% safety margin (Geth-style)
                            let estimated = (gas_used as f64 * 1.15) as u64;
                            let estimated = estimated.max(21_000);
                            return Ok(json!(format!("0x{:x}", estimated)));
                        }
                        Err(_) => {
                            // Execution failed — return generous estimate
                            return Ok(json!(format!("0x{:x}", base_gas + calldata_gas + 100_000)));
                        }
                    }
                }
            }
        }

        // Fallback: analytic estimate
        let estimated = base_gas
            + create_gas
            + calldata_gas
            + if is_create { data.len() as u64 * 200 } else { 50_000 };
        Ok(json!(format!("0x{:x}", estimated)))
    }});

    // NOTE: eth_sendTransaction is handled by tx_rpc.rs which registers after this
    // and overrides this handler. The tx_rpc.rs version includes P2P broadcasting.
    // This duplicate was removed to avoid confusion and dead code.

    // eth_getTransactionReceipt - uses UnifiedMempool for pending txs, falls back to DB
    let mp_for_receipt = mempool.clone();
    let state_for_receipt = unified_state.clone();
    let db_for_receipt = db.clone();
    io.add_method("eth_getTransactionReceipt", move |params: Params| {
        let mp_for_receipt = mp_for_receipt.clone();
        let state_for_receipt = state_for_receipt.clone();
        let db_for_receipt = db_for_receipt.clone();
        async move {
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

        let block_number = state_for_receipt.read().block_number();

        // Check UnifiedMempool for pending/recently-confirmed transactions
        if let Some(core_tx) = mp_for_receipt.get_transaction(&hash) {
            let meta = mp_for_receipt.get_pending_metadata(&hash)
                .unwrap_or_default();
            let from_bytes: [u8; 20] = *core_tx.from.as_bytes();
            let to_bytes: Option<[u8; 20]> = core_tx.to.map(|a| *a.as_bytes());
            let contract_bytes: Option<[u8; 20]> = meta.contract_address.map(|a| *a.as_bytes());
            return Ok(json!({
                "transactionHash": format!("0x{}", hex::encode(hash)),
                "transactionIndex": "0x0",
                "blockHash": format!("0x{}", hex::encode(hash)),
                "blockNumber": format!("0x{:x}", block_number),
                "from": format!("0x{}", hex::encode(from_bytes)),
                "to": to_bytes.map(|b| format!("0x{}", hex::encode(b))),
                "contractAddress": contract_bytes.map(|b| format!("0x{}", hex::encode(b))),
                "cumulativeGasUsed": format!("0x{:x}", meta.gas_used),
                "gasUsed": format!("0x{:x}", meta.gas_used),
                "status": if meta.status { "0x1" } else { "0x0" },
                "logs": []
            }));
        }

        // DB fallback: look up the stored receipt for confirmed transactions
        if let Ok(Some(receipt_bytes)) = db_for_receipt.get_receipt(&hash) {
            if let Ok(r) = bincode::deserialize::<luxtensor_core::receipt::Receipt>(&receipt_bytes) {
                let status_hex = match r.status {
                    luxtensor_core::receipt::ExecutionStatus::Success => "0x1",
                    luxtensor_core::receipt::ExecutionStatus::Failed => "0x0",
                };
                let logs: Vec<serde_json::Value> = r.logs.iter().map(|log| {
                    json!({
                        "address": format!("0x{}", hex::encode(log.address.as_bytes())),
                        "topics": log.topics.iter().map(|t| format!("0x{}", hex::encode(t))).collect::<Vec<_>>(),
                        "data": format!("0x{}", hex::encode(&log.data)),
                    })
                }).collect();

                return Ok(json!({
                    "transactionHash": format!("0x{}", hex::encode(hash)),
                    "transactionIndex": format!("0x{:x}", r.transaction_index),
                    "blockHash": format!("0x{}", hex::encode(r.block_hash)),
                    "blockNumber": format!("0x{:x}", r.block_height),
                    "from": format!("0x{}", hex::encode(r.from.as_bytes())),
                    "to": r.to.map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                    "contractAddress": r.contract_address.map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                    "cumulativeGasUsed": format!("0x{:x}", r.gas_used),
                    "gasUsed": format!("0x{:x}", r.gas_used),
                    "status": status_hex,
                    "logs": logs
                }));
            }
        }

        Ok(json!(null))
    }});

    // eth_call - Execute a call without creating a transaction (read-only)
    // Uses shared EvmExecutor.static_call() to read REAL contract storage
    let state_for_call = unified_state.clone();
    let evm_for_call = evm_executor.clone();
    io.add_method("eth_call", move |params: Params| {
        let state_for_call = state_for_call.clone();
        let evm_for_call = evm_for_call.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let call_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing call object".to_string(),
            data: None,
        })?;

        // Parse call parameters
        let from_str = call_obj
            .get("from")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0000000000000000000000000000000000000000");

        let to_str = call_obj.get("to").and_then(|v| v.as_str());

        let data_hex = call_obj.get("data").and_then(|v| v.as_str()).unwrap_or("0x");

        // Parse gas limit from call object (default: 1M)
        let gas_limit: u64 = call_obj
            .get("gas")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).ok()
            })
            .unwrap_or(1_000_000);

        // Parse addresses
        let from_addr = hex_to_address(from_str).unwrap_or([0u8; 20]);
        let to_addr = match to_str {
            None => return Ok(json!("0x")),
            Some(addr_str) => match hex_to_address(addr_str) {
                None => return Ok(json!("0x")),
                Some(addr) => addr,
            },
        };

        // Parse data
        let data = {
            let s = data_hex.strip_prefix("0x").unwrap_or(data_hex);
            hex::decode(s).unwrap_or_default()
        };

        // Get contract code and block number from UnifiedStateDB
        let state_guard = state_for_call.read();
        let contract_code = match state_guard.get_code(&luxtensor_core::Address::from(to_addr)) {
            Some(code) => code.to_vec(),
            None => {
                return Ok(json!("0x"));
            }
        };
        let block_number = state_guard.block_number();
        drop(state_guard);

        // Get current timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Use shared EvmExecutor.static_call() which deep-clones the real EVM state
        // (accounts, storage, block_hashes) and executes WITHOUT committing changes.
        // This ensures eth_call reads the actual contract storage from executed TXs.
        if let Some(ref executor) = evm_for_call {
            match executor.static_call(
                luxtensor_core::Address::from(from_addr),
                luxtensor_contracts::ContractAddress::from(to_addr),
                contract_code,
                data,
                gas_limit,
                block_number,
                timestamp,
                1, // gas_price for eth_call
            ) {
                Ok((output, _gas_used, _logs)) => Ok(json!(format!("0x{}", hex::encode(output)))),
                Err(e) => {
                    tracing::warn!("eth_call execution error: {:?}", e);
                    Ok(json!("0x"))
                }
            }
        } else {
            // Fallback: no shared executor, create a fresh one (no storage — legacy behavior)
            let fallback = luxtensor_contracts::EvmExecutor::default();
            fallback.deploy_code(&luxtensor_core::Address::from(to_addr), contract_code.clone());
            match fallback.static_call(
                luxtensor_core::Address::from(from_addr),
                luxtensor_contracts::ContractAddress::from(to_addr),
                contract_code,
                data,
                gas_limit,
                block_number,
                timestamp,
                1,
            ) {
                Ok((output, _gas_used, _logs)) => Ok(json!(format!("0x{}", hex::encode(output)))),
                Err(e) => {
                    tracing::warn!("eth_call fallback execution error: {:?}", e);
                    Ok(json!("0x"))
                }
            }
        }
    }});

    // eth_getCode - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_method("eth_getCode", move |params: Params| {
        let state = state.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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
    }});

    // eth_accounts
    // SECURITY: Returns empty array. Previously returned hardcoded Hardhat default
    // addresses with publicly-known private keys, which would allow anyone to
    // steal funds sent to those addresses.
    io.add_method("eth_accounts", move |_params: Params| { async move { Ok(json!([])) }});

    // net_version - Route to UnifiedStateDB
    let state = unified_state.clone();
    io.add_method("net_version", move |_params: Params| {
        let state = state.clone();
        async move {
            let chain_id = state.read().chain_id();
            Ok(json!(chain_id.to_string()))
        }
    });

    // eth_sendRawTransaction - Standard Ethereum RLP-encoded signed transactions
    // Supports Legacy (type 0), EIP-2930 (type 1), EIP-1559 (type 2)
    // Full MetaMask / ethers.js / web3.js compatibility
    let mp_for_sendraw = mempool.clone();
    let unified_for_sendraw = unified_state.clone();
    let broadcaster_for_sendraw = broadcaster.clone();
    io.add_method("eth_sendRawTransaction", move |params: Params| {
        let mp_for_sendraw = mp_for_sendraw.clone();
        let unified_for_sendraw = unified_for_sendraw.clone();
        let broadcaster_for_sendraw = broadcaster_for_sendraw.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let raw_tx = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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

        if tx_bytes.len() < 10 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Transaction too short".to_string(),
                data: None,
            });
        }

        // ========== RLP DECODE (standard Ethereum wire format) ==========
        let decoded = decode_rlp_transaction(&tx_bytes).map_err(|e| {
            tracing::warn!("RLP decode failed: {}", e);
            RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Failed to decode RLP transaction: {}", e),
                data: None,
            }
        })?;

        let from = decoded.from;
        let nonce = decoded.nonce;
        let to = decoded.to;
        let value = decoded.value;
        let gas = decoded.gas_limit;
        let data = decoded.data.clone();
        let r = decoded.r;
        let s = decoded.s;
        let v = decoded.v as u8;
        let tx_hash = decoded.signing_hash; // keccak256 of full raw bytes

        info!(
            "📝 RLP decoded TX type={} from=0x{} nonce={} to={} value={} gas={}",
            decoded.tx_type,
            hex::encode(&from),
            nonce,
            to.map(|a| format!("0x{}", hex::encode(a))).unwrap_or_else(|| "CREATE".into()),
            value,
            gas,
        );

        // === REPLAY PROTECTION: Validate chain ID ===
        let expected_chain_id = unified_for_sendraw.read().chain_id();
        if decoded.chain_id != 0 && decoded.chain_id != expected_chain_id {
            return Err(RpcError {
                code: ErrorCode::ServerError(-32000),
                message: format!(
                    "chain ID mismatch: expected {} got {}",
                    expected_chain_id, decoded.chain_id
                ),
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

        // Check for duplicate nonce in pending transactions (UnifiedMempool)
        {
            let pending = mp_for_sendraw.get_pending_transactions();
            for ptx in &pending {
                let ptx_from: [u8; 20] = *ptx.from.as_bytes();
                if ptx_from == from && ptx.nonce == nonce {
                    return Err(RpcError {
                        code: ErrorCode::ServerError(-32000),
                        message: format!("known transaction: nonce {} already pending", nonce),
                        data: None,
                    });
                }
            }
        }

        // Create core Transaction with signature (single object — no more dual structs)
        let gas_price = decoded.gas_price;
        let to_addr = to.map(luxtensor_core::Address::from);
        let mut core_tx = luxtensor_core::Transaction::with_chain_id(
            expected_chain_id,
            nonce,
            luxtensor_core::Address::from(from),
            to_addr,
            value,
            gas_price,
            gas,
            data,
        );
        // Preserve original ECDSA signature from the signed transaction
        core_tx.v = v;
        core_tx.r = r;
        core_tx.s = s;

        // Create lightweight pending metadata for RPC queries
        let metadata = luxtensor_core::PendingTxMetadata::default();

        // Add to UnifiedMempool (single add — replaces both add_pending + queue_transaction)
        if let Err(e) = mp_for_sendraw.add_transaction_with_metadata(core_tx.clone(), metadata) {
            return Err(RpcError {
                code: ErrorCode::InternalError,
                message: format!("Mempool rejected transaction: {}", e),
                data: None,
            });
        }

        // Broadcast to P2P network for multi-node propagation
        if let Err(e) = broadcaster_for_sendraw.broadcast(&core_tx) {
            tracing::warn!("Failed to broadcast raw transaction to P2P: {}", e);
        } else {
            info!("📡 Raw transaction broadcasted to P2P network: {}", hash_to_hex(&tx_hash));
        }

        info!("📥 Received signed raw transaction: {}", hash_to_hex(&tx_hash));
        Ok(json!(hash_to_hex(&tx_hash)))
    }});

    // === Additional ETH methods for full compatibility ===

    // eth_syncing - Returns syncing status
    io.add_method("eth_syncing", move |_params: Params| { async move {
        Ok(json!(false)) // Not syncing
    }});

    // eth_mining - Returns whether client is mining
    io.add_method("eth_mining", move |_params: Params| { async move { Ok(json!(false)) }});

    // eth_hashrate - Returns hashrate
    io.add_method("eth_hashrate", move |_params: Params| { async move { Ok(json!("0x0")) }});

    // eth_coinbase - Returns coinbase address
    io.add_method("eth_coinbase", move |_params: Params| { async move {
        Ok(json!("0x0000000000000000000000000000000000000000"))
    }});

    // eth_protocolVersion - Returns protocol version
    io.add_method("eth_protocolVersion", move |_params: Params| { async move {
        Ok(json!("0x41")) // Protocol version 65
    }});

    let unified_for_storage = unified_state.clone();

    // eth_getStorageAt - Route to UnifiedStateDB
    io.add_method("eth_getStorageAt", move |params: Params| {
        let unified_for_storage = unified_for_storage.clone();
        async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(RpcError::invalid_params("Missing address or position"));
        }

        // Parse address
        let addr_bytes = match hex_to_address(&parsed[0]) {
            Some(a) => a,
            None => {
                return Ok(json!(
                    "0x0000000000000000000000000000000000000000000000000000000000000000"
                ))
            }
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
    }});

    // net_listening - Returns whether node is listening
    io.add_method("net_listening", move |_params: Params| { async move { Ok(json!(true)) }});

    // web3_sha3 - Returns Keccak-256 hash
    io.add_method("web3_sha3", move |params: Params| { async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(RpcError::invalid_params("Missing data"));
        }
        let data = parsed[0].trim_start_matches("0x");
        let bytes = hex::decode(data).unwrap_or_default();
        use sha3::{Digest, Keccak256};
        let hash = Keccak256::digest(&bytes);
        Ok(json!(format!("0x{}", hex::encode(hash))))
    }});

    // rpc_modules - Returns available RPC modules
    io.add_method("rpc_modules", move |_params: Params| { async move {
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
    }});

    // dev_faucet - Credit tokens to address for testing (DEV/TEST NETWORKS ONLY)
    // Features: rate limiting, per-address cooldown, daily drip limit, configurable amount
    // SECURITY: Only works on chain_id 8898/9999/1337/31337
    // 🔧 FIX: Now creates a real on-chain Transaction (from Address::zero())
    // and submits to mempool + broadcast so all nodes see the same state change.
    let dev_state = unified_state.clone();
    let faucet_limiter = Arc::new(Mutex::new(FaucetRateLimiter::new(faucet_config.clone())));
    let faucet_cfg = faucet_config;
    let faucet_mempool = mempool.clone();
    let faucet_broadcaster = broadcaster.clone();
    io.add_method("dev_faucet", move |params: Params| {
        let dev_state = dev_state.clone();
        let faucet_limiter = faucet_limiter.clone();
        let faucet_cfg = faucet_cfg.clone();
        let faucet_mempool = faucet_mempool.clone();
        let faucet_broadcaster = faucet_broadcaster.clone();
        async move {
        // Guard: check if faucet is enabled
        if !faucet_cfg.enabled {
            return Err(RpcError {
                code: ErrorCode::MethodNotFound,
                message: "Faucet is disabled".to_string(),
                data: None,
            });
        }

        // Guard: only allow faucet on dev/test chain IDs
        // Chain ID 8898 = LuxTensor devnet, 9999 = LuxTensor testnet, 1337 = local dev, 31337 = Hardhat
        let chain_id = dev_state.read().chain_id();
        let allowed_chains: [u64; 4] = [8898, 9999, 1337, 31337];
        if !allowed_chains.contains(&chain_id) {
            return Err(RpcError {
                code: ErrorCode::MethodNotFound,
                message: "dev_faucet is only available on dev/test networks (chain_id 8898, 9999, 1337, 31337)".to_string(),
                data: None,
            });
        }

        let p: Vec<serde_json::Value> = params.parse()?;
        let address_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing address parameter. Usage: dev_faucet(address, [amount])".to_string(),
                data: None,
            })?;

        let address = hex_to_address(address_str).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid address format. Expected 0x-prefixed hex (20 bytes)".to_string(),
            data: None,
        })?;

        // Rate limiting: check cooldown and daily limit
        let rate_check = {
            let mut limiter = faucet_limiter.lock();
            limiter.check_and_record(&address)
        };

        let (drips_remaining, next_cooldown) = rate_check.map_err(|msg| RpcError {
            code: ErrorCode::ServerError(-32005), // Rate limit error code
            message: msg,
            data: None,
        })?;

        // Parse amount (default: use config drip_amount)
        let amount: u128 = p.get(1)
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(faucet_cfg.drip_amount);

        // Cap maximum single drip to 10x the default drip amount
        let max_drip = faucet_cfg.drip_amount.saturating_mul(10);
        if amount > max_drip {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Amount exceeds maximum drip (max: {})", max_drip),
                data: None,
            });
        }

        // 🔧 FIX: Create a real on-chain Transaction instead of direct set_balance.
        // From: Address::zero() (special mint address — executor skips sig/nonce/balance)
        // To: target address
        // Value: faucet amount
        // This TX gets mined into a block and executed by ALL nodes, ensuring
        // consistent state across the network.
        let faucet_tx = luxtensor_core::Transaction::with_chain_id(
            chain_id,
            0,  // nonce=0 always (executor skips nonce check for zero-addr)
            luxtensor_core::Address::zero(), // special faucet "mint" address
            Some(luxtensor_core::Address::from(address)),
            amount,
            1,      // gas_price (minimum)
            21000,  // gas_limit (simple transfer)
            vec![], // no data
        );

        let tx_hash = faucet_tx.hash();
        let tx_hash_hex = format!("0x{}", hex::encode(&tx_hash));

        // Submit to mempool — will be picked up by block production
        let _ = faucet_mempool.add_transaction(faucet_tx.clone());

        // Broadcast to peers so they also add it to their mempools
        let _ = faucet_broadcaster.broadcast(&faucet_tx);

        tracing::info!("💰 Faucet TX submitted: {} → 0x{} amount={}",
            tx_hash_hex, hex::encode(address), amount);

        Ok(json!({
            "success": true,
            "address": address_to_hex(&address),
            "credited": amount.to_string(),
            "tx_hash": tx_hash_hex,
            "drips_remaining_today": drips_remaining,
            "next_available_in_secs": next_cooldown,
            "chain_id": chain_id,
            "note": "Faucet TX submitted to mempool. Balance updates after block is mined."
        }))
    }});

    // ========================================================================
    // NOTE: eth_getTransactionByHash is registered in server.rs::register_blockchain_methods()
    // with proper 3-tier lookup: pending_txs (DashMap) → mempool → confirmed DB.
    // Do NOT register it here — this register_eth_methods() is called AFTER
    // register_blockchain_methods(), so any handler here would OVERRIDE the
    // correct server.rs version. The server.rs version queries shared_pending_txs
    // which includes transactions received via P2P gossipsub.
    // ========================================================================

    // eth_getBlockByNumber — Returns block info
    let unified_for_block = unified_state.clone();
    io.add_method("eth_getBlockByNumber", move |params: Params| {
        let unified_for_block = unified_for_block.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let block_tag = p.get(0).and_then(|v| v.as_str()).unwrap_or("latest");

        let state = unified_for_block.read();
        let block_number = if block_tag == "latest" || block_tag == "pending" {
            state.block_number()
        } else {
            let s = block_tag.strip_prefix("0x").unwrap_or(block_tag);
            u64::from_str_radix(s, 16).unwrap_or(state.block_number())
        };

        // Return a minimal but valid block object
        Ok(json!({
            "number": format!("0x{:x}", block_number),
            "hash": format!("0x{}", hex::encode([0u8; 32])),
            "parentHash": format!("0x{}", hex::encode([0u8; 32])),
            "nonce": "0x0000000000000000",
            "sha3Uncles": format!("0x{}", hex::encode([0u8; 32])),
            "logsBloom": format!("0x{}", hex::encode([0u8; 256])),
            "transactionsRoot": format!("0x{}", hex::encode([0u8; 32])),
            "stateRoot": format!("0x{}", hex::encode([0u8; 32])),
            "receiptsRoot": format!("0x{}", hex::encode([0u8; 32])),
            "miner": "0x0000000000000000000000000000000000000000",
            "difficulty": "0x0",
            "totalDifficulty": "0x0",
            "extraData": "0x",
            "size": "0x0",
            "gasLimit": format!("0x{:x}", 30_000_000u64),
            "gasUsed": "0x0",
            "timestamp": format!("0x{:x}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs()).unwrap_or(0)),
            "transactions": [],
            "uncles": [],
            "baseFeePerGas": format!("0x{:x}", 1_000_000_000u64)
        }))
    }});

    // ========================================================================
    // eth_feeHistory — EIP-1559 fee history (critical for MetaMask)
    // ========================================================================
    let db_for_fee = db.clone();
    let unified_for_fee = unified_state.clone();
    io.add_method("eth_feeHistory", move |params: Params| {
        let db_for_fee = db_for_fee.clone();
        let unified_for_fee = unified_for_fee.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;

        // Parse block_count (first param)
        let block_count = p
            .get(0)
            .and_then(|v| {
                if let Some(n) = v.as_u64() {
                    Some(n)
                } else if let Some(s) = v.as_str() {
                    let s = s.strip_prefix("0x").unwrap_or(s);
                    u64::from_str_radix(s, 16).ok()
                } else {
                    None
                }
            })
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing or invalid block_count".to_string(),
                data: None,
            })?;

        // Clamp block_count to 1024 (Ethereum standard limit)
        let block_count = block_count.min(1024);
        if block_count == 0 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "block_count must be > 0".to_string(),
                data: None,
            });
        }

        // Parse newest_block (second param)
        let newest_block_tag = p.get(1).and_then(|v| v.as_str()).unwrap_or("latest");

        let state_guard = unified_for_fee.read();
        let current_block = state_guard.block_number();
        drop(state_guard);

        let newest_block = match newest_block_tag {
            "latest" | "pending" => current_block,
            "earliest" => 0,
            s => {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).unwrap_or(current_block)
            }
        };

        // reward_percentiles (third param) — optional array of floats
        let _reward_percentiles: Vec<f64> = p
            .get(2)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect())
            .unwrap_or_default();

        // Calculate the oldest block we'll report
        let oldest_block = newest_block.saturating_sub(block_count - 1);

        let mut base_fee_per_gas: Vec<String> = Vec::new();
        let mut gas_used_ratio: Vec<f64> = Vec::new();
        let mut reward: Vec<Vec<String>> = Vec::new();

        // Use FeeMarket for base fee calculations
        use luxtensor_consensus::FeeMarket;
        let market = FeeMarket::new();
        let default_base_fee = market.current_base_fee();

        // Iterate from oldest_block to newest_block
        for height in oldest_block..=newest_block {
            if let Ok(Some(block)) = db_for_fee.get_block_by_height(height) {
                let gas_used = block.header.gas_used;
                let gas_limit = block.header.gas_limit;
                let ratio = if gas_limit > 0 { gas_used as f64 / gas_limit as f64 } else { 0.0 };
                gas_used_ratio.push(ratio);
                // Use default_base_fee since we don't persist per-block base fee
                base_fee_per_gas.push(format!("0x{:x}", default_base_fee));
                // Reward: empty inner array per block (we don't track per-tx priority fees)
                reward.push(_reward_percentiles.iter().map(|_| "0x0".to_string()).collect());
            } else {
                // Block not found in DB, use defaults
                gas_used_ratio.push(0.0);
                base_fee_per_gas.push(format!("0x{:x}", default_base_fee));
                reward.push(_reward_percentiles.iter().map(|_| "0x0".to_string()).collect());
            }
        }

        // EIP-1559 spec: baseFeePerGas has block_count + 1 entries
        // (includes the predicted next base fee)
        base_fee_per_gas.push(format!("0x{:x}", default_base_fee));

        info!(
            "eth_feeHistory: block_count={}, oldest=0x{:x}, newest=0x{:x}",
            block_count, oldest_block, newest_block
        );

        Ok(json!({
            "oldestBlock": format!("0x{:x}", oldest_block),
            "baseFeePerGas": base_fee_per_gas,
            "gasUsedRatio": gas_used_ratio,
            "reward": if _reward_percentiles.is_empty() { None } else { Some(reward) }
        }))
    }});

    // ========================================================================
    // eth_getBlockByHash — Standard block lookup by hash
    // ========================================================================
    let db_for_bbh = db.clone();
    let unified_for_bbh = unified_state.clone();
    io.add_method("eth_getBlockByHash", move |params: Params| {
        let db_for_bbh = db_for_bbh.clone();
        let unified_for_bbh = unified_for_bbh.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;

        let hash_str = p.get(0)
            .and_then(|v| v.as_str())
            .ok_or_else(|| RpcError {
                code: ErrorCode::InvalidParams,
                message: "Missing block hash".to_string(),
                data: None,
            })?;

        let hash_str = hash_str.strip_prefix("0x").unwrap_or(hash_str);
        let hash_bytes = hex::decode(hash_str).map_err(|_| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Invalid hex hash".to_string(),
            data: None,
        })?;

        if hash_bytes.len() != 32 {
            return Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Hash must be 32 bytes".to_string(),
                data: None,
            });
        }

        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes);

        let full_transactions = p.get(1)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Look up block by hash in DB
        match db_for_bbh.get_block(&hash) {
            Ok(Some(block)) => {
                let block_hash = block.hash();
                let chain_id = unified_for_bbh.read().chain_id();

                let transactions = if full_transactions {
                    // Return full transaction objects
                    block.transactions.iter().enumerate().map(|(idx, tx)| {
                        json!({
                            "hash": format!("0x{}", hex::encode(tx.hash())),
                            "nonce": format!("0x{:x}", tx.nonce),
                            "blockHash": format!("0x{}", hex::encode(block_hash)),
                            "blockNumber": format!("0x{:x}", block.header.height),
                            "transactionIndex": format!("0x{:x}", idx),
                            "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
                            "to": tx.to.as_ref().map(|a| format!("0x{}", hex::encode(a.as_bytes()))),
                            "value": format!("0x{:x}", tx.value),
                            "gas": format!("0x{:x}", tx.gas_limit),
                            "gasPrice": format!("0x{:x}", tx.gas_price),
                            "input": format!("0x{}", hex::encode(&tx.data)),
                            "v": format!("0x{:x}", tx.v as u64),
                            "r": format!("0x{}", hex::encode(tx.r)),
                            "s": format!("0x{}", hex::encode(tx.s)),
                            "chainId": format!("0x{:x}", chain_id),
                            "type": "0x0"
                        })
                    }).collect::<Vec<_>>()
                } else {
                    // Return only transaction hashes
                    block.transactions.iter().map(|tx| {
                        json!(format!("0x{}", hex::encode(tx.hash())))
                    }).collect::<Vec<_>>()
                };

                info!("eth_getBlockByHash: found block height={} txs={}",
                    block.header.height, block.transactions.len());

                Ok(json!({
                    "number": format!("0x{:x}", block.header.height),
                    "hash": format!("0x{}", hex::encode(block_hash)),
                    "parentHash": format!("0x{}", hex::encode(block.header.previous_hash)),
                    "nonce": "0x0000000000000000",
                    "sha3Uncles": format!("0x{}", hex::encode([0u8; 32])),
                    "logsBloom": format!("0x{}", hex::encode([0u8; 256])),
                    "transactionsRoot": format!("0x{}", hex::encode(block.header.txs_root)),
                    "stateRoot": format!("0x{}", hex::encode(block.header.state_root)),
                    "receiptsRoot": format!("0x{}", hex::encode(block.header.receipts_root)),
                    "miner": format!("0x{}", hex::encode(&block.header.validator[..20])),
                    "difficulty": "0x0",
                    "totalDifficulty": "0x0",
                    "extraData": format!("0x{}", hex::encode(&block.header.extra_data)),
                    "size": "0x0",
                    "gasLimit": format!("0x{:x}", block.header.gas_limit),
                    "gasUsed": format!("0x{:x}", block.header.gas_used),
                    "timestamp": format!("0x{:x}", block.header.timestamp),
                    "transactions": transactions,
                    "uncles": [],
                    "baseFeePerGas": format!("0x{:x}", 1_000_000_000u64)
                }))
            }
            Ok(None) => Ok(json!(null)),
            Err(e) => {
                tracing::warn!("eth_getBlockByHash DB error: {:?}", e);
                Ok(json!(null))
            }
        }
    }});

    // ========================================================================
    // eth_getBlockTransactionCountByNumber — Transaction count in a block
    // ========================================================================
    let db_for_txcount = db.clone();
    let unified_for_txcount = unified_state.clone();
    io.add_method("eth_getBlockTransactionCountByNumber", move |params: Params| {
        let db_for_txcount = db_for_txcount.clone();
        let unified_for_txcount = unified_for_txcount.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;

        let block_tag = p.get(0).and_then(|v| v.as_str()).unwrap_or("latest");

        let state_guard = unified_for_txcount.read();
        let current_block = state_guard.block_number();
        drop(state_guard);

        let block_number = match block_tag {
            "latest" | "pending" => current_block,
            "earliest" => 0,
            s => {
                let s = s.strip_prefix("0x").unwrap_or(s);
                u64::from_str_radix(s, 16).unwrap_or(current_block)
            }
        };

        match db_for_txcount.get_block_by_height(block_number) {
            Ok(Some(block)) => {
                let count = block.transactions.len();
                info!(
                    "eth_getBlockTransactionCountByNumber: block={} count={}",
                    block_number, count
                );
                Ok(json!(format!("0x{:x}", count)))
            }
            Ok(None) => {
                // Block not found — return 0 (matches common Ethereum node behavior)
                Ok(json!("0x0"))
            }
            Err(e) => {
                tracing::warn!("eth_getBlockTransactionCountByNumber DB error: {:?}", e);
                Ok(json!("0x0"))
            }
        }
    }});
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
    io.add_method("eth_getLogs", move |params: Params| {
        let store = store.clone();
        let state = state.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_obj = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter object".to_string(),
            data: None,
        })?;

        let filter = parse_log_filter(filter_obj)?;
        let current_block = state.read().block_number();
        let logs = store.read().get_logs(&filter, current_block);

        let rpc_logs: Vec<serde_json::Value> = logs.iter().map(|log| log.to_rpc_log()).collect();

        Ok(json!(rpc_logs))
    }});

    // eth_newFilter - Create a new filter
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_method("eth_newFilter", move |params: Params| {
        let store = store.clone();
        let state = state.clone();
        async move {
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
    }});

    // eth_getFilterChanges - Get logs since last poll
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_method("eth_getFilterChanges", move |params: Params| {
        let store = store.clone();
        let state = state.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter ID".to_string(),
            data: None,
        })?;

        let current_block = state.read().block_number();
        match store.read().get_filter_changes(filter_id, current_block) {
            Some(logs) => {
                let rpc_logs: Vec<serde_json::Value> =
                    logs.iter().map(|log| log.to_rpc_log()).collect();
                Ok(json!(rpc_logs))
            }
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Filter not found".to_string(),
                data: None,
            }),
        }
    }});

    // eth_getFilterLogs - Get all logs for a filter
    let store = log_store.clone();
    let state = unified_state.clone();
    io.add_method("eth_getFilterLogs", move |params: Params| {
        let store = store.clone();
        let state = state.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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
                let rpc_logs: Vec<serde_json::Value> =
                    logs.iter().map(|log| log.to_rpc_log()).collect();
                Ok(json!(rpc_logs))
            }
            None => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: "Filter not found".to_string(),
                data: None,
            }),
        }
    }});

    // eth_uninstallFilter - Remove a filter
    let store = log_store.clone();
    io.add_method("eth_uninstallFilter", move |params: Params| {
        let store = store.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let filter_id = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing filter ID".to_string(),
            data: None,
        })?;

        let removed = store.read().uninstall_filter(filter_id);
        Ok(json!(removed))
    }});

    // ========================================================================
    // eth_newBlockFilter — Create a filter for new blocks
    // ========================================================================
    let store_for_bf = log_store.clone();
    let state_for_bf = unified_state.clone();
    io.add_method("eth_newBlockFilter", move |_params: Params| {
        let store_for_bf = store_for_bf.clone();
        let state_for_bf = state_for_bf.clone();
        async move {
        let current_block = state_for_bf.read().block_number();
        // Create a log filter that tracks new blocks (empty filter = all logs)
        let filter =
            crate::logs::LogFilter { from_block: Some(current_block + 1), ..Default::default() };
        let filter_id = store_for_bf.read().new_filter(filter, current_block);
        info!("eth_newBlockFilter: created filter_id={} at block={}", filter_id, current_block);
        Ok(json!(filter_id))
    }});

    // ========================================================================
    // eth_newPendingTransactionFilter — Create a filter for pending txs
    // ========================================================================
    let store_for_ptf = log_store.clone();
    let state_for_ptf = unified_state.clone();
    io.add_method("eth_newPendingTransactionFilter", move |_params: Params| {
        let store_for_ptf = store_for_ptf.clone();
        let state_for_ptf = state_for_ptf.clone();
        async move {
        let current_block = state_for_ptf.read().block_number();
        // Create a filter tracking from current block onward
        let filter =
            crate::logs::LogFilter { from_block: Some(current_block), ..Default::default() };
        let filter_id = store_for_ptf.read().new_filter(filter, current_block);
        info!(
            "eth_newPendingTransactionFilter: created filter_id={} at block={}",
            filter_id, current_block
        );
        Ok(json!(filter_id))
    }});

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
                let hashes: Vec<[u8; 32]> = arr
                    .iter()
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
    Ok(luxtensor_core::types::Address::try_from_slice(&bytes).ok_or_else(|| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Invalid address length".to_string(),
        data: None,
    })?)
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
    io.add_method("eth_sendUserOperation", move |params: Params| {
        let ep = ep.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;

        let user_op_json = p.get(0).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing user operation".to_string(),
            data: None,
        })?;

        let _entry_point_addr = p.get(1).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
            code: ErrorCode::InvalidParams,
            message: "Missing entry point address".to_string(),
            data: None,
        })?;

        // Parse user operation
        let user_op = parse_user_operation(user_op_json)?;

        // Validate and queue the operation for block inclusion
        let entry_point = ep.read();
        match entry_point.validate_user_op(&user_op) {
            Ok(()) => {
                // Queue in EntryPoint's pending pool — will be drained during block production
                let op_hash = entry_point.queue_user_op(user_op);
                Ok(json!(format!("0x{}", hex::encode(op_hash))))
            }
            Err(e) => Err(RpcError {
                code: ErrorCode::InvalidParams,
                message: format!("Validation failed: {}", e),
                data: None,
            }),
        }
        }
    });

    // eth_estimateUserOperationGas - Estimate gas for user operation
    let ep = entry_point.clone();
    io.add_method("eth_estimateUserOperationGas", move |params: Params| {
        let ep = ep.clone();
        async move {
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
    }});

    // eth_getUserOperationReceipt - Get receipt for a user operation
    let ep = entry_point.clone();
    io.add_method("eth_getUserOperationReceipt", move |params: Params| {
        let ep = ep.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;

        let op_hash_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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
    }});

    // eth_supportedEntryPoints - Get list of supported entry points
    let ep = entry_point.clone();
    io.add_method("eth_supportedEntryPoints", move |_params: Params| {
        let ep = ep.clone();
        async move {
        let entry_point = ep.read();
        let supported = entry_point.get_supported_entry_points();
        Ok(json!(supported))
    }});

    // eth_getUserOperationByHash - Get user operation by hash (ERC-4337)
    let ep = entry_point.clone();
    io.add_method("eth_getUserOperationByHash", move |params: Params| {
        let ep = ep.clone();
        async move {
        let p: Vec<serde_json::Value> = params.parse()?;
        let op_hash_str = p.get(0).and_then(|v| v.as_str()).ok_or_else(|| RpcError {
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
    }});

    // eth_chainId - Return chain ID for AA context (ERC-4337)
    // Note: This complements the standard eth_chainId but is specific to AA operations
    let ep = entry_point.clone();
    io.add_method("aa_chainId", move |_params: Params| {
        let ep = ep.clone();
        async move {
        let entry_point = ep.read();
        let chain_id = entry_point.chain_id();
        Ok(json!(format!("0x{:x}", chain_id)))
    }});

    info!("Registered ERC-4337 Account Abstraction RPC methods (6 methods)");
}

/// Parse a UserOperation from JSON
fn parse_user_operation(
    obj: &serde_json::Value,
) -> Result<luxtensor_contracts::UserOperation, RpcError> {
    use luxtensor_contracts::UserOperation;

    let sender = obj.get("sender").and_then(|v| v.as_str()).ok_or_else(|| RpcError {
        code: ErrorCode::InvalidParams,
        message: "Missing sender".to_string(),
        data: None,
    })?;

    let nonce = obj
        .get("nonce")
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
