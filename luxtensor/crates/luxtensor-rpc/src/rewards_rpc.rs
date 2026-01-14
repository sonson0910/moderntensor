// Rewards RPC API Module
// Provides JSON-RPC endpoints for reward queries and claims

use jsonrpc_core::{IoHandler, Params};
use luxtensor_consensus::RewardExecutor;
use parking_lot::RwLock;
use std::sync::Arc;

/// Register reward-related RPC methods
pub fn register_reward_methods(io: &mut IoHandler, executor: Arc<RwLock<RewardExecutor>>) {
    // rewards_getPending - Get pending rewards for an address
    let exec = executor.clone();
    io.add_sync_method("rewards_getPending", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let pending = exec.read().get_pending_rewards(address);

        Ok(serde_json::json!({
            "address": format!("0x{}", hex::encode(address)),
            "pending": format!("0x{:x}", pending),
            "pendingDecimal": pending.to_string()
        }))
    });

    // rewards_getBalance - Get full balance info for an address
    let exec = executor.clone();
    io.add_sync_method("rewards_getBalance", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let balance = exec.read().get_balance(address);

        Ok(serde_json::json!({
            "address": format!("0x{}", hex::encode(address)),
            "available": format!("0x{:x}", balance.available),
            "availableDecimal": balance.available.to_string(),
            "pendingRewards": format!("0x{:x}", balance.pending_rewards),
            "staked": format!("0x{:x}", balance.staked),
            "lockedUntil": balance.locked_until
        }))
    });

    // rewards_claim - Claim pending rewards
    let exec = executor.clone();
    io.add_sync_method("rewards_claim", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let result = exec.read().claim_rewards(address);

        Ok(serde_json::json!({
            "success": result.success,
            "claimed": format!("0x{:x}", result.amount),
            "claimedDecimal": result.amount.to_string(),
            "newBalance": format!("0x{:x}", result.new_balance),
            "message": result.message
        }))
    });

    // rewards_getHistory - Get reward history for an address
    let exec = executor.clone();
    io.add_sync_method("rewards_getHistory", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address_str = parsed[0].as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?;
        let limit = parsed.get(1)
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let address = parse_address(address_str)?;
        let history = exec.read().get_reward_history(address, limit);

        let history_json: Vec<serde_json::Value> = history.iter().map(|entry| {
            serde_json::json!({
                "epoch": entry.epoch,
                "amount": format!("0x{:x}", entry.amount),
                "amountDecimal": entry.amount.to_string(),
                "type": format!("{:?}", entry.reward_type),
                "claimed": entry.claimed
            })
        }).collect();

        Ok(serde_json::json!({
            "address": format!("0x{}", hex::encode(address)),
            "history": history_json
        }))
    });

    // rewards_getStats - Get executor statistics
    let exec = executor.clone();
    io.add_sync_method("rewards_getStats", move |_params: Params| {
        let stats = exec.read().stats();

        Ok(serde_json::json!({
            "currentEpoch": stats.current_epoch,
            "totalPending": format!("0x{:x}", stats.total_pending),
            "totalPendingDecimal": stats.total_pending.to_string(),
            "totalAvailable": format!("0x{:x}", stats.total_available),
            "daoBalance": format!("0x{:x}", stats.dao_balance),
            "daoBalanceDecimal": stats.dao_balance.to_string(),
            "accountsWithPending": stats.accounts_with_pending,
            "totalAccounts": stats.total_accounts
        }))
    });

    // rewards_getBurnStats - Get burn statistics
    let exec = executor.clone();
    io.add_sync_method("rewards_getBurnStats", move |_params: Params| {
        let stats = exec.read().burn_manager().stats();

        Ok(serde_json::json!({
            "totalBurned": format!("0x{:x}", stats.total_burned),
            "totalBurnedDecimal": stats.total_burned.to_string(),
            "txFeeBurned": format!("0x{:x}", stats.tx_fee_burned),
            "subnetBurned": format!("0x{:x}", stats.subnet_burned),
            "quotaBurned": format!("0x{:x}", stats.quota_burned),
            "slashingBurned": format!("0x{:x}", stats.slashing_burned),
            "recycledToGrants": format!("0x{:x}", stats.recycled_to_grants)
        }))
    });

    // rewards_getDaoBalance - Get DAO treasury balance
    let exec = executor.clone();
    io.add_sync_method("rewards_getDaoBalance", move |_params: Params| {
        let balance = exec.read().get_dao_balance();

        Ok(serde_json::json!({
            "balance": format!("0x{:x}", balance),
            "balanceDecimal": balance.to_string()
        }))
    });
}

/// Parse hex address string to [u8; 20]
fn parse_address(addr_str: &str) -> Result<[u8; 20], jsonrpc_core::Error> {
    let addr_str = addr_str.strip_prefix("0x").unwrap_or(addr_str);

    if addr_str.len() != 40 {
        return Err(jsonrpc_core::Error::invalid_params("Address must be 40 hex characters"));
    }

    let bytes = hex::decode(addr_str)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hex address"))?;

    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_address() {
        let addr = "0x1234567890123456789012345678901234567890";
        let result = parse_address(addr);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed[0], 0x12);
        assert_eq!(parsed[19], 0x90);
    }

    #[test]
    fn test_parse_address_no_prefix() {
        let addr = "1234567890123456789012345678901234567890";
        let result = parse_address(addr);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_address_invalid_length() {
        let addr = "0x1234";
        let result = parse_address(addr);
        assert!(result.is_err());
    }
}
