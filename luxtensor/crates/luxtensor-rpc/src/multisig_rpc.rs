//! Multisig RPC Module — JSON-RPC methods for multisig wallet operations
//!
//! Provides endpoints to create wallets, propose/approve transactions,
//! and query pending multisig transactions.

use jsonrpc_core::{IoHandler, Params};
use luxtensor_core::multisig::MultisigManager;
use std::sync::Arc;

/// Shared context for Multisig RPC handlers.
pub struct MultisigRpcContext {
    pub manager: Arc<MultisigManager>,
}

impl MultisigRpcContext {
    pub fn new(manager: Arc<MultisigManager>) -> Self {
        Self { manager }
    }
}

/// Register all `multisig_*` RPC methods.
pub fn register_multisig_methods(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    register_multisig_create_wallet(ctx, io);
    register_multisig_get_wallet(ctx, io);
    register_multisig_propose_transaction(ctx, io);
    register_multisig_approve_transaction(ctx, io);
    register_multisig_get_transaction(ctx, io);
    register_multisig_get_pending(ctx, io);
}

// =============================================================================
// multisig_createWallet
// =============================================================================

fn register_multisig_create_wallet(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    let manager = ctx.manager.clone();

    io.add_sync_method("multisig_createWallet", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let signers_raw = p
            .get("signers")
            .and_then(|v| v.as_array())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing signers array"))?;

        let threshold = p
            .get("threshold")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing threshold"))?
            as u8;

        let name = p.get("name").and_then(|v| v.as_str()).map(String::from);

        let block_timestamp = p
            .get("block_timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });

        let mut signers = Vec::new();
        for s in signers_raw {
            let addr_hex = s
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Signer must be hex string"))?;
            signers.push(parse_address(addr_hex)?);
        }

        match manager.create_wallet(signers, threshold, name, block_timestamp) {
            Ok(wallet) => Ok(serde_json::json!({
                "wallet_id": wallet.id,
                "threshold": wallet.threshold,
                "signers": wallet.signers.iter()
                    .map(|s| format!("0x{}", hex::encode(s)))
                    .collect::<Vec<_>>(),
                "created_at": wallet.created_at,
                "name": wallet.name,
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// multisig_getWallet
// =============================================================================

fn register_multisig_get_wallet(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    let manager = ctx.manager.clone();

    io.add_sync_method("multisig_getWallet", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing wallet_id"));
        }

        match manager.get_wallet(&parsed[0]) {
            Some(wallet) => Ok(serde_json::json!({
                "wallet_id": wallet.id,
                "threshold": wallet.threshold,
                "signers": wallet.signers.iter()
                    .map(|s| format!("0x{}", hex::encode(s)))
                    .collect::<Vec<_>>(),
                "created_at": wallet.created_at,
                "name": wallet.name,
            })),
            None => Ok(serde_json::Value::Null),
        }
    });
}

// =============================================================================
// multisig_proposeTransaction
// =============================================================================

fn register_multisig_propose_transaction(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    let manager = ctx.manager.clone();

    io.add_sync_method("multisig_proposeTransaction", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let wallet_id = p
            .get("wallet_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing wallet_id"))?;

        let proposer_hex = p
            .get("proposer")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing proposer"))?;
        let proposer = parse_address(proposer_hex)?;

        let to_hex = p
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing to"))?;
        let to = parse_address(to_hex)?;

        let value = p
            .get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u128>().ok())
            .unwrap_or(0);

        let data = p
            .get("data")
            .and_then(|v| v.as_str())
            .map(|s| parse_bytes(s))
            .unwrap_or_default();

        let block_timestamp = p
            .get("block_timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });

        match manager.propose_transaction(wallet_id, &proposer, to, value, data, block_timestamp) {
            Ok(tx) => Ok(serde_json::json!({
                "tx_id": tx.id,
                "wallet_id": tx.wallet_id,
                "to": format!("0x{}", hex::encode(tx.to)),
                "value": tx.value.to_string(),
                "approval_count": tx.approval_count(),
                "proposed_at": tx.proposed_at,
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// multisig_approveTransaction
// =============================================================================

fn register_multisig_approve_transaction(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    let manager = ctx.manager.clone();

    io.add_sync_method("multisig_approveTransaction", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let tx_id = p
            .get("tx_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing tx_id"))?;

        let signer_hex = p
            .get("signer")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing signer"))?;
        let signer = parse_address(signer_hex)?;

        let current_timestamp = p
            .get("current_timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });

        match manager.approve_transaction(tx_id, &signer, current_timestamp) {
            Ok(tx) => Ok(serde_json::json!({
                "tx_id": tx.id,
                "approval_count": tx.approval_count(),
                "executed": tx.executed,
                "approvals": tx.approvals.iter()
                    .map(|a| format!("0x{}", hex::encode(a)))
                    .collect::<Vec<_>>(),
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// multisig_getTransaction
// =============================================================================

fn register_multisig_get_transaction(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    let manager = ctx.manager.clone();

    io.add_sync_method("multisig_getTransaction", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing tx_id"));
        }

        match manager.get_transaction(&parsed[0]) {
            Some(tx) => Ok(serde_json::json!({
                "tx_id": tx.id,
                "wallet_id": tx.wallet_id,
                "to": format!("0x{}", hex::encode(tx.to)),
                "value": tx.value.to_string(),
                "approval_count": tx.approval_count(),
                "executed": tx.executed,
                "proposed_at": tx.proposed_at,
                "expires_at": tx.expires_at,
                "approvals": tx.approvals.iter()
                    .map(|a| format!("0x{}", hex::encode(a)))
                    .collect::<Vec<_>>(),
            })),
            None => Ok(serde_json::Value::Null),
        }
    });
}

// =============================================================================
// multisig_getPendingForWallet
// =============================================================================

fn register_multisig_get_pending(ctx: &MultisigRpcContext, io: &mut IoHandler) {
    let manager = ctx.manager.clone();

    io.add_sync_method("multisig_getPendingForWallet", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let wallet_id = p
            .get("wallet_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing wallet_id"))?;

        let current_timestamp = p
            .get("current_timestamp")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });

        let txs = manager.get_pending_for_wallet(wallet_id, current_timestamp);
        let result: Vec<serde_json::Value> = txs
            .iter()
            .map(|tx| {
                serde_json::json!({
                    "tx_id": tx.id,
                    "to": format!("0x{}", hex::encode(tx.to)),
                    "value": tx.value.to_string(),
                    "approval_count": tx.approval_count(),
                    "proposed_at": tx.proposed_at,
                    "expires_at": tx.expires_at,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "wallet_id": wallet_id,
            "pending_transactions": result,
            "count": result.len(),
        }))
    });
}

// =============================================================================
// Helpers
// =============================================================================

/// Parse a 20-byte hex address string into `[u8; 20]`.
fn parse_address(hex_str: &str) -> std::result::Result<luxtensor_core::Address, jsonrpc_core::Error> {
    let cleaned = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex format"))?;
    if bytes.len() != 20 {
        return Err(jsonrpc_core::Error::invalid_params("Expected 20-byte address"));
    }
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(arr.into())
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
    fn test_multisig_rpc_context_creation() {
        let manager = Arc::new(MultisigManager::new());
        let ctx = MultisigRpcContext::new(manager.clone());
        assert!(Arc::strong_count(&ctx.manager) >= 1);
    }

    #[test]
    fn test_parse_address_valid() {
        let hex = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let result = parse_address(hex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), luxtensor_core::Address::from([0xAA; 20]));
    }

    #[test]
    fn test_parse_address_invalid_length() {
        let result = parse_address("0x0102");
        assert!(result.is_err());
    }
}
