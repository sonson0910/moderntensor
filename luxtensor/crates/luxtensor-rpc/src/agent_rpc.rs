//! Agent RPC Module — JSON-RPC methods for Agentic EVM interaction
//!
//! Provides endpoints to register, manage, and query AI agents that run
//! autonomously on-chain via Token Bound Accounts (ERC-6551 inspired).

use crate::helpers::parse_address;
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_contracts::{AgentRegistry, AgentTriggerConfig};
use std::sync::Arc;
use tracing::info;

/// Shared context for Agent RPC handlers.
pub struct AgentRpcContext {
    pub agent_registry: Arc<AgentRegistry>,
}

impl AgentRpcContext {
    pub fn new(agent_registry: Arc<AgentRegistry>) -> Self {
        Self { agent_registry }
    }
}

/// Register all `agent_*` RPC methods.
pub fn register_agent_methods(ctx: &AgentRpcContext, io: &mut IoHandler) {
    register_agent_register(ctx, io);
    register_agent_deregister(ctx, io);
    register_agent_deposit_gas(ctx, io);
    register_agent_withdraw_gas(ctx, io);
    register_agent_get_info(ctx, io);
    register_agent_list_all(ctx, io);
}

// =============================================================================
// agent_register
// =============================================================================

fn register_agent_register(ctx: &AgentRpcContext, io: &mut IoHandler) {
    let registry = ctx.agent_registry.clone();

    io.add_sync_method("agent_register", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let agent_id_hex = p
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing agent_id"))?;
        let owner_hex = p
            .get("owner")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing owner"))?;
        let wallet_hex = p
            .get("wallet_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing wallet_address"))?;
        let contract_hex = p
            .get("contract_address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing contract_address"))?;
        let gas_deposit_hex = p
            .get("gas_deposit")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");
        let block_interval = p
            .get("block_interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let agent_id = parse_32_bytes(agent_id_hex)?;
        let owner = parse_address(owner_hex)?;
        let wallet = parse_address(wallet_hex)?;
        let contract = parse_address(contract_hex)?;
        let gas_deposit =
            u128::from_str_radix(gas_deposit_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        let trigger_config = AgentTriggerConfig {
            block_interval,
            gas_limit_per_trigger: p
                .get("gas_limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(500_000),
            trigger_calldata: p
                .get("calldata")
                .and_then(|v| v.as_str())
                .map(|s| hex::decode(s.trim_start_matches("0x")).unwrap_or_default())
                .unwrap_or_default(),
            enabled: block_interval > 0,
        };

        // Use current time as block approximation (actual block comes from consensus)
        let current_block = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / 6; // rough block estimate

        match registry.register_agent(
            agent_id, owner, wallet, contract, trigger_config, gas_deposit, current_block,
        ) {
            Ok(()) => {
                info!(
                    "Agent registered via RPC: 0x{}",
                    hex::encode(agent_id)
                );
                Ok(serde_json::json!({
                    "success": true,
                    "agent_id": format!("0x{}", hex::encode(agent_id)),
                }))
            }
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// agent_deregister
// =============================================================================

fn register_agent_deregister(ctx: &AgentRpcContext, io: &mut IoHandler) {
    let registry = ctx.agent_registry.clone();

    io.add_sync_method("agent_deregister", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let agent_id_hex = p
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing agent_id"))?;
        let caller_hex = p
            .get("caller")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing caller"))?;

        let agent_id = parse_32_bytes(agent_id_hex)?;
        let caller = parse_address(caller_hex)?;

        match registry.deregister_agent(&agent_id, &caller) {
            Ok(removed) => Ok(serde_json::json!({
                "success": true,
                "refunded_gas_deposit": format!("0x{:x}", removed.gas_deposit),
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// agent_depositGas
// =============================================================================

fn register_agent_deposit_gas(ctx: &AgentRpcContext, io: &mut IoHandler) {
    let registry = ctx.agent_registry.clone();

    io.add_sync_method("agent_depositGas", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let agent_id_hex = p
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing agent_id"))?;
        let amount_hex = p
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing amount"))?;

        let agent_id = parse_32_bytes(agent_id_hex)?;
        let amount =
            u128::from_str_radix(amount_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        match registry.deposit_gas(&agent_id, amount) {
            Ok(new_balance) => Ok(serde_json::json!({
                "success": true,
                "new_balance": format!("0x{:x}", new_balance),
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// agent_withdrawGas
// =============================================================================

fn register_agent_withdraw_gas(ctx: &AgentRpcContext, io: &mut IoHandler) {
    let registry = ctx.agent_registry.clone();

    io.add_sync_method("agent_withdrawGas", move |params: Params| {
        let p: serde_json::Value = params.parse()?;

        let agent_id_hex = p
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing agent_id"))?;
        let caller_hex = p
            .get("caller")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing caller"))?;
        let amount_hex = p
            .get("amount")
            .and_then(|v| v.as_str())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing amount"))?;

        let agent_id = parse_32_bytes(agent_id_hex)?;
        let caller = parse_address(caller_hex)?;
        let amount =
            u128::from_str_radix(amount_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        match registry.withdraw_gas(&agent_id, amount, &caller) {
            Ok(new_balance) => Ok(serde_json::json!({
                "success": true,
                "new_balance": format!("0x{:x}", new_balance),
            })),
            Err(e) => Err(jsonrpc_core::Error::invalid_params(e.to_string())),
        }
    });
}

// =============================================================================
// agent_getInfo
// =============================================================================

fn register_agent_get_info(ctx: &AgentRpcContext, io: &mut IoHandler) {
    let registry = ctx.agent_registry.clone();

    io.add_sync_method("agent_getInfo", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing agent_id"));
        }

        let agent_id = parse_32_bytes(&parsed[0])?;

        match registry.get_agent(&agent_id) {
            Some(agent) => Ok(agent_to_json(&agent)),
            None => Ok(Value::Null),
        }
    });
}

// =============================================================================
// agent_listAll
// =============================================================================

fn register_agent_list_all(ctx: &AgentRpcContext, io: &mut IoHandler) {
    let registry = ctx.agent_registry.clone();

    io.add_sync_method("agent_listAll", move |_params: Params| {
        let agents = registry.list_agents();
        let total = agents.len();

        let agent_list: Vec<serde_json::Value> = agents
            .iter()
            .map(|a| agent_to_json(a))
            .collect();

        Ok(serde_json::json!({
            "total": total,
            "agents": agent_list,
        }))
    });
}

// =============================================================================
// Helpers
// =============================================================================

fn agent_to_json(agent: &luxtensor_contracts::AgentAccount) -> serde_json::Value {
    serde_json::json!({
        "agent_id": format!("0x{}", hex::encode(agent.agent_id)),
        "owner": format!("0x{}", hex::encode(agent.owner.as_bytes())),
        "wallet_address": format!("0x{}", hex::encode(agent.wallet_address.as_bytes())),
        "contract_address": format!("0x{}", hex::encode(agent.contract_address.as_bytes())),
        "registered_at": agent.registered_at,
        "total_gas_used": agent.total_gas_used,
        "gas_deposit": format!("0x{:x}", agent.gas_deposit),
        "last_triggered_block": agent.last_triggered_block,
        "trigger_config": {
            "block_interval": agent.trigger_config.block_interval,
            "gas_limit_per_trigger": agent.trigger_config.gas_limit_per_trigger,
            "enabled": agent.trigger_config.enabled,
            "calldata": format!("0x{}", hex::encode(&agent.trigger_config.trigger_calldata)),
        },
    })
}

fn parse_32_bytes(hex_str: &str) -> Result<[u8; 32], jsonrpc_core::Error> {
    let cleaned = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(cleaned)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex format"))?;
    if bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params("Expected 32-byte ID"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_32_bytes_valid() {
        let hex = "0x0101010101010101010101010101010101010101010101010101010101010101";
        let result = parse_32_bytes(hex);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0x01; 32]);
    }

    #[test]
    fn test_parse_32_bytes_invalid_length() {
        let hex = "0x0102";
        let result = parse_32_bytes(hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_rpc_context_creation() {
        let registry = Arc::new(AgentRegistry::with_defaults());
        let ctx = AgentRpcContext::new(registry.clone());
        assert_eq!(ctx.agent_registry.agent_count(), 0);
    }
}
