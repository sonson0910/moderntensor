//! Bridge RPC Module — JSON-RPC methods for cross-chain bridge operations
//!
//! Provides endpoints to query bridge configuration, message status,
//! and list bridge messages with optional status filtering.

use jsonrpc_core::{IoHandler, Params};
use luxtensor_core::bridge::{BridgeMessageStatus, InMemoryBridge};
use std::sync::Arc;

/// Shared context for Bridge RPC handlers.
pub struct BridgeRpcContext {
    pub bridge: Arc<InMemoryBridge>,
}

impl BridgeRpcContext {
    pub fn new(bridge: Arc<InMemoryBridge>) -> Self {
        Self { bridge }
    }
}

/// Register all `bridge_*` RPC methods.
pub fn register_bridge_methods(ctx: &BridgeRpcContext, io: &mut IoHandler) {
    register_bridge_get_config(ctx, io);
    register_bridge_get_message(ctx, io);
    register_bridge_list_messages(ctx, io);
    register_bridge_get_stats(ctx, io);
}

// =============================================================================
// bridge_getConfig
// =============================================================================

fn register_bridge_get_config(ctx: &BridgeRpcContext, io: &mut IoHandler) {
    let bridge = ctx.bridge.clone();

    io.add_sync_method("bridge_getConfig", move |_params: Params| {
        let config = bridge.config();
        Ok(serde_json::json!({
            "min_transfer_amount": config.min_transfer_amount,
            "max_transfer_amount": config.max_transfer_amount,
            "confirmation_threshold": config.confirmation_threshold,
            "relayers": config.relayers.iter()
                .map(|r| format!("0x{}", hex::encode(r)))
                .collect::<Vec<_>>(),
            "supported_chains": config.supported_chains.iter()
                .map(|c| c.as_u64())
                .collect::<Vec<_>>(),
            "paused": config.paused,
        }))
    });
}

// =============================================================================
// bridge_getMessage
// =============================================================================

fn register_bridge_get_message(ctx: &BridgeRpcContext, io: &mut IoHandler) {
    let bridge = ctx.bridge.clone();

    io.add_sync_method("bridge_getMessage", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing message_hash"));
        }

        let hash = parse_hash(&parsed[0])?;

        match bridge.get_message(hash) {
            Ok(msg) => Ok(serde_json::json!({
                "hash": format!("0x{}", hex::encode(msg.hash)),
                "sender": format!("0x{}", hex::encode(msg.sender)),
                "recipient": format!("0x{}", hex::encode(msg.recipient)),
                "amount": msg.amount.to_string(),
                "source_chain": format!("{:?}", msg.source_chain),
                "target_chain": format!("{:?}", msg.target_chain),
                "direction": format!("{:?}", msg.direction),
                "status": format!("{:?}", msg.status),
                "nonce": msg.nonce,
                "block_number": msg.block_number,
                "timestamp": msg.timestamp,
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// bridge_listMessages
// =============================================================================

fn register_bridge_list_messages(ctx: &BridgeRpcContext, io: &mut IoHandler) {
    let bridge = ctx.bridge.clone();

    io.add_sync_method("bridge_listMessages", move |params: Params| {
        // Optional status filter: "Pending", "Confirmed", "Executed", "Failed"
        let status_filter: Option<BridgeMessageStatus> = match params.parse::<Vec<String>>() {
            Ok(args) if !args.is_empty() => match args[0].to_lowercase().as_str() {
                "pending" => Some(BridgeMessageStatus::Pending),
                "confirmed" => Some(BridgeMessageStatus::Confirmed),
                "executed" => Some(BridgeMessageStatus::Executed),
                "failed" => Some(BridgeMessageStatus::Failed),
                _ => None,
            },
            _ => None,
        };

        let messages = bridge.list_messages(status_filter);
        let result: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "hash": format!("0x{}", hex::encode(msg.hash)),
                    "sender": format!("0x{}", hex::encode(msg.sender)),
                    "recipient": format!("0x{}", hex::encode(msg.recipient)),
                    "amount": msg.amount.to_string(),
                    "status": format!("{:?}", msg.status),
                    "nonce": msg.nonce,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "messages": result,
            "count": result.len(),
        }))
    });
}

// =============================================================================
// bridge_getStats
// =============================================================================

fn register_bridge_get_stats(ctx: &BridgeRpcContext, io: &mut IoHandler) {
    let bridge = ctx.bridge.clone();

    io.add_sync_method("bridge_getStats", move |_params: Params| {
        let all = bridge.list_messages(None);
        let pending = all.iter().filter(|m| m.status == BridgeMessageStatus::Pending).count();
        let confirmed = all.iter().filter(|m| m.status == BridgeMessageStatus::Confirmed).count();
        let executed = all.iter().filter(|m| m.status == BridgeMessageStatus::Executed).count();
        let failed = all.iter().filter(|m| m.status == BridgeMessageStatus::Failed).count();

        Ok(serde_json::json!({
            "total_messages": all.len(),
            "pending": pending,
            "confirmed": confirmed,
            "executed": executed,
            "failed": failed,
        }))
    });
}

// =============================================================================
// Helpers
// =============================================================================

/// Parse a 32-byte hex string into `[u8; 32]` Hash.
fn parse_hash(hex_str: &str) -> std::result::Result<luxtensor_core::Hash, jsonrpc_core::Error> {
    let cleaned = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex format"))?;
    if bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params("Expected 32-byte hash"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::bridge::BridgeConfig;

    #[test]
    fn test_bridge_rpc_context_creation() {
        let bridge = Arc::new(InMemoryBridge::new(BridgeConfig::default()));
        let ctx = BridgeRpcContext::new(bridge.clone());
        assert!(Arc::strong_count(&ctx.bridge) >= 1);
    }

    #[test]
    fn test_parse_hash_valid() {
        let hex = "0x0101010101010101010101010101010101010101010101010101010101010101";
        let result = parse_hash(hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_hash_invalid_length() {
        let result = parse_hash("0x0102");
        assert!(result.is_err());
    }
}
