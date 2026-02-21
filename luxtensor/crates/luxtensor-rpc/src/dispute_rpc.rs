//! Dispute RPC Module — JSON-RPC methods for Optimistic AI dispute resolution
//!
//! Provides endpoints to submit fraud proofs, query dispute status,
//! and list pending optimistic results awaiting finalization.

use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_oracle::{DisputeManager, H256, EthBytes};
use std::sync::Arc;
use tracing::info;

/// Shared context for Dispute RPC handlers.
pub struct DisputeRpcContext {
    pub dispute_manager: Arc<DisputeManager>,
}

impl DisputeRpcContext {
    pub fn new(dispute_manager: Arc<DisputeManager>) -> Self {
        Self { dispute_manager }
    }
}

/// Register all `dispute_*` RPC methods.
pub fn register_dispute_methods(ctx: &DisputeRpcContext, io: &mut IoHandler) {
    register_dispute_submit(ctx, io);
    register_dispute_get_status(ctx, io);
    register_dispute_stats(ctx, io);
}

/// Helper to run an async future from within a sync RPC handler.
///
/// Uses `tokio::task::block_in_place` + `Handle::current().block_on()`
/// which is the recommended pattern for bridging sync → async within
/// a multi-threaded tokio runtime.
fn block_on_async<F: std::future::Future>(f: F) -> F::Output {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(f)
    })
}

// =============================================================================
// dispute_submit
// =============================================================================

fn register_dispute_submit(ctx: &DisputeRpcContext, io: &mut IoHandler) {
    let dm = ctx.dispute_manager.clone();

    io.add_sync_method("dispute_submit", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let request_id_hex = p
            .get("request_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing request_id"))?;
        let challenger_hex = p
            .get("challenger")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing challenger"))?;
        let correct_result_hex = p
            .get("correct_result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing correct_result"))?;
        let proof_hash_hex = p
            .get("proof_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing proof_hash"))?;
        let current_block = p
            .get("current_block")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let request_id = parse_h256(request_id_hex)?;
        let challenger = parse_20_bytes(challenger_hex)?;
        let correct_result = parse_bytes(correct_result_hex);
        let proof_hash = parse_h256(proof_hash_hex)?;

        match block_on_async(dm.submit_dispute(
            request_id,
            challenger,
            EthBytes::from(correct_result),
            proof_hash,
            current_block,
        )) {
            Ok(()) => {
                info!(
                    "Dispute submitted via RPC: request_id=0x{}",
                    hex::encode(request_id.as_bytes())
                );
                Ok(serde_json::json!({
                    "success": true,
                    "request_id": format!("0x{}", hex::encode(request_id.as_bytes())),
                }))
            }
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// dispute_getStatus
// =============================================================================

fn register_dispute_get_status(ctx: &DisputeRpcContext, io: &mut IoHandler) {
    let dm = ctx.dispute_manager.clone();

    io.add_sync_method("dispute_getStatus", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing request_id"));
        }

        let request_id = parse_h256(&parsed[0])?;

        match block_on_async(dm.get_dispute_status(&request_id)) {
            Some(status) => {
                let status_str = format!("{:?}", status);
                Ok(serde_json::json!({
                    "request_id": format!("0x{}", hex::encode(request_id.as_bytes())),
                    "status": status_str,
                }))
            }
            None => Ok(Value::Null),
        }
    });
}

// =============================================================================
// dispute_stats
// =============================================================================

fn register_dispute_stats(ctx: &DisputeRpcContext, io: &mut IoHandler) {
    let dm = ctx.dispute_manager.clone();

    io.add_sync_method("dispute_stats", move |_params: Params| {
        let pending = block_on_async(dm.pending_count());
        let active = block_on_async(dm.active_dispute_count());

        Ok(serde_json::json!({
            "pending_results": pending,
            "active_disputes": active,
        }))
    });
}

// =============================================================================
// Helpers
// =============================================================================

/// Parse a 32-byte hex string into `H256` (ethers type via luxtensor-oracle).
fn parse_h256(hex_str: &str) -> Result<H256, jsonrpc_core::Error> {
    let cleaned = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex format"))?;
    if bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Expected 32-byte hash",
        ));
    }
    Ok(H256::from_slice(&bytes))
}

/// Parse a 20-byte hex string into `[u8; 20]`.
fn parse_20_bytes(hex_str: &str) -> Result<[u8; 20], jsonrpc_core::Error> {
    let cleaned = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex format"))?;
    if bytes.len() != 20 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Expected 20-byte address",
        ));
    }
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Parse hex-encoded bytes (generic length).
fn parse_bytes(hex_str: &str) -> Vec<u8> {
    let cleaned = hex_str.trim_start_matches("0x");
    hex::decode(cleaned).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_h256_valid() {
        let hex = "0x0101010101010101010101010101010101010101010101010101010101010101";
        let result = parse_h256(hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_h256_invalid_length() {
        let result = parse_h256("0x0102");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_20_bytes_valid() {
        let hex = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let result = parse_20_bytes(hex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0xAA; 20]);
    }

    #[test]
    fn test_dispute_rpc_context_creation() {
        let dm = Arc::new(DisputeManager::default_config());
        let ctx = DisputeRpcContext::new(dm.clone());
        // pending_count is async, cannot be called directly in sync test
        assert!(Arc::strong_count(&ctx.dispute_manager) >= 1);
    }
}
