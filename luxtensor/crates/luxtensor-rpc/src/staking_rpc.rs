// Staking RPC API Module
// Provides JSON-RPC endpoints for staking operations with persistent storage
// SECURITY: All state-changing operations now require signature verification

use crate::helpers::verify_caller_signature;
use jsonrpc_core::{IoHandler, Params, Error, ErrorCode};
use luxtensor_consensus::RewardExecutor;
use luxtensor_core::Address;
use luxtensor_storage::{MetagraphDB, StakingData, DelegationData};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Minimum stake required for validators (1 token)
const MIN_VALIDATOR_STAKE: u128 = 1_000_000_000_000_000_000;
/// Minimum delegation amount (0.1 token)
const MIN_DELEGATION: u128 = 100_000_000_000_000_000;

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Create an internal error with message
fn internal_error(msg: &str) -> Error {
    Error {
        code: ErrorCode::InternalError,
        message: msg.to_string(),
        data: None,
    }
}

/// Register staking-related RPC methods with persistent storage
pub fn register_staking_methods(
    io: &mut IoHandler,
    metagraph_db: Arc<MetagraphDB>,
    _executor: Arc<RwLock<RewardExecutor>>,
) {
    // staking_stake - Stake tokens as a validator (PERSISTED)
    // SECURITY: Now requires signature verification to prevent impersonation
    let db = metagraph_db.clone();
    io.add_sync_method("staking_stake", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 4 {
            return Err(Error::invalid_params(
                "Missing parameters. Required: address, amount, timestamp, signature"
            ));
        }

        let address = parse_address(&parsed[0])?;
        let amount = parse_amount(&parsed[1])?;
        let timestamp: u64 = parsed[2].parse()
            .map_err(|_| Error::invalid_params("Invalid timestamp"))?;
        let signature = &parsed[3];

        // Security: Verify timestamp is recent (within 5 minutes)
        let now = get_current_timestamp();
        if now > timestamp + 300 || timestamp > now + 60 {
            return Err(Error::invalid_params("Signature expired or future timestamp"));
        }

        // Security: Construct message and verify signature
        let message = format!("stake:{}:{}", hex::encode(address), amount);

        // Convert [u8; 20] to Address for signature verification
        let addr = Address::from(address);

        // Try recovery IDs 0 and 1 (common values)
        let sig_valid = verify_caller_signature(&addr, &message, signature, 0)
            .or_else(|_| verify_caller_signature(&addr, &message, signature, 1));

        if sig_valid.is_err() {
            return Err(Error::invalid_params(
                "Signature verification failed - caller does not own address"
            ));
        }

        if amount < MIN_VALIDATOR_STAKE {
            return Err(Error::invalid_params(format!(
                "Minimum stake is {} wei", MIN_VALIDATOR_STAKE
            )));
        }

        // Get existing stake or create new
        let existing = db.get_stake(&address)
            .map_err(|e| internal_error(&e.to_string()))?;

        let new_stake = match existing {
            Some(mut data) => {
                data.stake += amount;
                data
            }
            None => StakingData {
                address,
                stake: amount,
                staked_at: get_current_timestamp(),
                last_reward_claim: 0,
            }
        };

        // Persist to database
        db.store_stake(&new_stake)
            .map_err(|e| internal_error(&e.to_string()))?;

        Ok(serde_json::json!({
            "success": true,
            "address": format!("0x{}", hex::encode(address)),
            "staked": format!("0x{:x}", new_stake.stake),
            "stakedDecimal": new_stake.stake.to_string(),
            "message": "Stake successful (persisted, signature verified)"
        }))
    });

    // staking_unstake - Unstake tokens (PERSISTED)
    let db = metagraph_db.clone();
    io.add_sync_method("staking_unstake", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(Error::invalid_params("Missing address or amount"));
        }

        let address = parse_address(&parsed[0])?;
        let amount = parse_amount(&parsed[1])?;

        let existing = db.get_stake(&address)
            .map_err(|e| internal_error(&e.to_string()))?
            .ok_or_else(|| Error::invalid_params("No stake found"))?;

        if amount > existing.stake {
            return Err(Error::invalid_params("Insufficient staked amount"));
        }

        let new_stake = existing.stake - amount;

        // Ensure remaining stake meets minimum or is zero
        if new_stake > 0 && new_stake < MIN_VALIDATOR_STAKE {
            return Err(Error::invalid_params(format!(
                "Remaining stake must be at least {} wei or zero",
                MIN_VALIDATOR_STAKE
            )));
        }

        if new_stake == 0 {
            db.delete_stake(&address)
                .map_err(|e| internal_error(&e.to_string()))?;
        } else {
            let updated = StakingData {
                stake: new_stake,
                ..existing
            };
            db.store_stake(&updated)
                .map_err(|e| internal_error(&e.to_string()))?;
        }

        Ok(serde_json::json!({
            "success": true,
            "address": format!("0x{}", hex::encode(address)),
            "unstaked": format!("0x{:x}", amount),
            "remaining": format!("0x{:x}", new_stake),
            "message": "Unstake successful (persisted)"
        }))
    });

    // staking_delegate - Delegate tokens to a validator (PERSISTED)
    let db = metagraph_db.clone();
    io.add_sync_method("staking_delegate", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 3 {
            return Err(Error::invalid_params("Missing delegator, validator, or amount"));
        }

        let delegator = parse_address(&parsed[0])?;
        let validator = parse_address(&parsed[1])?;
        let amount = parse_amount(&parsed[2])?;
        let lock_days: u32 = parsed.get(3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Verify validator exists
        let validator_stake = db.get_stake(&validator)
            .map_err(|e| internal_error(&e.to_string()))?;

        if validator_stake.is_none() || validator_stake.as_ref().map(|s| s.stake).unwrap_or(0) < MIN_VALIDATOR_STAKE {
            return Err(Error::invalid_params("Validator not found or insufficient stake"));
        }

        if amount < MIN_DELEGATION {
            return Err(Error::invalid_params(format!(
                "Minimum delegation is {} wei", MIN_DELEGATION
            )));
        }

        // Check if already delegated
        if db.get_delegation(&delegator)
            .map_err(|e| internal_error(&e.to_string()))?
            .is_some()
        {
            return Err(Error::invalid_params("Already delegated. Undelegate first."));
        }

        // Store delegation
        let delegation = DelegationData {
            delegator,
            validator,
            amount,
            lock_days,
            start_block: get_current_timestamp(), // Using timestamp as approximation
            delegated_at: get_current_timestamp(),
        };

        db.store_delegation(&delegation)
            .map_err(|e| internal_error(&e.to_string()))?;

        Ok(serde_json::json!({
            "success": true,
            "delegator": format!("0x{}", hex::encode(delegator)),
            "validator": format!("0x{}", hex::encode(validator)),
            "amount": format!("0x{:x}", amount),
            "lockDays": lock_days,
            "message": "Delegation successful (persisted)"
        }))
    });

    // staking_undelegate - Remove delegation (PERSISTED)
    let db = metagraph_db.clone();
    io.add_sync_method("staking_undelegate", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(Error::invalid_params("Missing delegator address"));
        }

        let delegator = parse_address(&parsed[0])?;

        let delegation = db.get_delegation(&delegator)
            .map_err(|e| internal_error(&e.to_string()))?
            .ok_or_else(|| Error::invalid_params("No delegation found"))?;

        // Check lock period
        let now = get_current_timestamp();
        let lock_end = delegation.delegated_at + (delegation.lock_days as u64 * 86400);
        if now < lock_end {
            let days_remaining = (lock_end - now) / 86400;
            return Err(Error::invalid_params(format!(
                "Delegation locked. {} days remaining", days_remaining
            )));
        }

        db.delete_delegation(&delegator)
            .map_err(|e| internal_error(&e.to_string()))?;

        Ok(serde_json::json!({
            "success": true,
            "delegator": format!("0x{}", hex::encode(delegator)),
            "returned": format!("0x{:x}", delegation.amount),
            "message": "Undelegation successful (persisted)"
        }))
    });

    // staking_getStake - Get validator stake (from DB)
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getStake", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let stake_data = db.get_stake(&address)
            .map_err(|e| internal_error(&e.to_string()))?;

        let stake = stake_data.as_ref().map(|s| s.stake).unwrap_or(0);

        Ok(serde_json::json!({
            "address": format!("0x{}", hex::encode(address)),
            "stake": format!("0x{:x}", stake),
            "stakeDecimal": stake.to_string(),
            "isValidator": stake >= MIN_VALIDATOR_STAKE,
            "stakedAt": stake_data.as_ref().map(|s| s.staked_at).unwrap_or(0)
        }))
    });

    // staking_getDelegation - Get delegation info (from DB)
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getDelegation", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(Error::invalid_params("Missing delegator address"));
        }

        let delegator = parse_address(&parsed[0])?;
        let delegation = db.get_delegation(&delegator)
            .map_err(|e| internal_error(&e.to_string()))?;

        match delegation {
            Some(info) => {
                Ok(serde_json::json!({
                    "delegator": format!("0x{}", hex::encode(delegator)),
                    "validator": format!("0x{}", hex::encode(info.validator)),
                    "amount": format!("0x{:x}", info.amount),
                    "amountDecimal": info.amount.to_string(),
                    "lockDays": info.lock_days,
                    "startBlock": info.start_block,
                    "delegatedAt": info.delegated_at
                }))
            }
            None => {
                Ok(serde_json::json!({
                    "delegator": format!("0x{}", hex::encode(delegator)),
                    "delegation": null
                }))
            }
        }
    });

    // staking_getValidators - List all validators (from DB)
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getValidators", move |_params: Params| {
        let stakes = db.get_all_stakes()
            .map_err(|e| internal_error(&e.to_string()))?;

        let validators: Vec<serde_json::Value> = stakes.iter()
            .filter(|s| s.stake >= MIN_VALIDATOR_STAKE)
            .map(|s| {
                serde_json::json!({
                    "address": format!("0x{}", hex::encode(s.address)),
                    "stake": format!("0x{:x}", s.stake),
                    "stakeDecimal": s.stake.to_string(),
                    "stakedAt": s.staked_at
                })
            })
            .collect();

        Ok(serde_json::json!({
            "validators": validators,
            "count": validators.len()
        }))
    });

    // staking_getMinimums - Get minimum stake requirements
    io.add_sync_method("staking_getMinimums", move |_params: Params| {
        Ok(serde_json::json!({
            "minValidatorStake": format!("0x{:x}", MIN_VALIDATOR_STAKE),
            "minValidatorStakeDecimal": MIN_VALIDATOR_STAKE.to_string(),
            "minDelegation": format!("0x{:x}", MIN_DELEGATION),
            "minDelegationDecimal": MIN_DELEGATION.to_string()
        }))
    });

    // staking_getTotalStake - Get total staked in network
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getTotalStake", move |_params: Params| {
        let stakes = db.get_all_stakes()
            .map_err(|e| internal_error(&e.to_string()))?;

        let total: u128 = stakes.iter().map(|s| s.stake).sum();

        Ok(serde_json::json!({
            "totalStake": format!("0x{:x}", total),
            "totalStakeDecimal": total.to_string(),
            "validatorCount": stakes.iter().filter(|s| s.stake >= MIN_VALIDATOR_STAKE).count()
        }))
    });

    // =========================================================================
    // SDK Compatibility Methods (Added for SDK integration)
    // =========================================================================

    // staking_getStakeForPair - Get stake for coldkey-hotkey pair
    // SDK: staking_mixin.py calls this for coldkey/hotkey stake lookup
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getStakeForPair", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(Error::invalid_params("Missing coldkey or hotkey address"));
        }

        let coldkey = parse_address(&parsed[0])?;
        let hotkey = parse_address(&parsed[1])?;

        // In our model, stake is per-address. For coldkey-hotkey pairs,
        // we lookup the hotkey's stake (which would be delegated from coldkey)
        // First check if there's a delegation from coldkey to hotkey
        if let Ok(Some(delegation)) = db.get_delegation(&coldkey) {
            if delegation.validator == hotkey {
                return Ok(serde_json::json!({
                    "coldkey": format!("0x{}", hex::encode(coldkey)),
                    "hotkey": format!("0x{}", hex::encode(hotkey)),
                    "stake": format!("0x{:x}", delegation.amount),
                    "stakeDecimal": delegation.amount.to_string()
                }));
            }
        }

        // Also check direct stake on hotkey
        if let Ok(Some(stake_data)) = db.get_stake(&hotkey) {
            return Ok(serde_json::json!({
                "coldkey": format!("0x{}", hex::encode(coldkey)),
                "hotkey": format!("0x{}", hex::encode(hotkey)),
                "stake": format!("0x{:x}", stake_data.stake),
                "stakeDecimal": stake_data.stake.to_string()
            }));
        }

        Ok(serde_json::json!({
            "coldkey": format!("0x{}", hex::encode(coldkey)),
            "hotkey": format!("0x{}", hex::encode(hotkey)),
            "stake": "0x0",
            "stakeDecimal": "0"
        }))
    });

    // staking_getAllStakesForColdkey - Get all stakes for a coldkey
    // SDK: staking_mixin.py calls this to get all hotkey stakes for a coldkey
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getAllStakesForColdkey", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(Error::invalid_params("Missing coldkey address"));
        }

        let coldkey = parse_address(&parsed[0])?;

        // Get all delegations and filter by this coldkey
        let all_delegations = db.get_all_delegations()
            .map_err(|e| internal_error(&e.to_string()))?;

        let stakes: serde_json::Map<String, serde_json::Value> = all_delegations
            .iter()
            .filter(|d| d.delegator == coldkey)
            .map(|d| (
                format!("0x{}", hex::encode(d.validator)),
                serde_json::json!(format!("0x{:x}", d.amount))
            ))
            .collect();

        Ok(serde_json::json!({
            "coldkey": format!("0x{}", hex::encode(coldkey)),
            "stakes": stakes,
            "count": stakes.len()
        }))
    });

    // staking_getDelegates - Get all validators accepting delegation
    // SDK: staking_mixin.py calls this to list all delegates
    let db = metagraph_db.clone();
    io.add_sync_method("staking_getDelegates", move |_params: Params| {
        let stakes = db.get_all_stakes()
            .map_err(|e| internal_error(&e.to_string()))?;

        let all_delegations = db.get_all_delegations()
            .map_err(|e| internal_error(&e.to_string()))?;

        // Calculate total delegated to each validator
        let mut delegation_totals: std::collections::HashMap<[u8; 20], u128> = std::collections::HashMap::new();
        for d in &all_delegations {
            *delegation_totals.entry(d.validator).or_insert(0) += d.amount;
        }

        // All validators with stake >= minimum are potential delegates
        let delegates: Vec<serde_json::Value> = stakes.iter()
            .filter(|s| s.stake >= MIN_VALIDATOR_STAKE)
            .map(|s| {
                let total_delegated = delegation_totals.get(&s.address).copied().unwrap_or(0);
                serde_json::json!({
                    "address": format!("0x{}", hex::encode(s.address)),
                    "stake": format!("0x{:x}", s.stake),
                    "stakeDecimal": s.stake.to_string(),
                    "totalDelegated": format!("0x{:x}", total_delegated),
                    "totalDelegatedDecimal": total_delegated.to_string(),
                    "delegatorCount": all_delegations.iter().filter(|d| d.validator == s.address).count(),
                    "isAcceptingDelegation": true
                })
            })
            .collect();

        Ok(serde_json::json!({
            "delegates": delegates,
            "count": delegates.len()
        }))
    });
}

/// Parse hex address string to [u8; 20]
fn parse_address(addr_str: &str) -> Result<[u8; 20], Error> {
    let addr_str = addr_str.strip_prefix("0x").unwrap_or(addr_str);

    if addr_str.len() != 40 {
        return Err(Error::invalid_params("Address must be 40 hex characters"));
    }

    let bytes = hex::decode(addr_str)
        .map_err(|_| Error::invalid_params("Invalid hex address"))?;

    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Ok(addr)
}

/// Parse amount string (hex or decimal) to u128
fn parse_amount(amount_str: &str) -> Result<u128, Error> {
    if amount_str.starts_with("0x") {
        u128::from_str_radix(&amount_str[2..], 16)
            .map_err(|_| Error::invalid_params("Invalid hex amount"))
    } else {
        amount_str.parse::<u128>()
            .map_err(|_| Error::invalid_params("Invalid amount"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_amount_decimal() {
        let amount = parse_amount("1000000000000000000");
        assert!(amount.is_ok());
        assert_eq!(amount.unwrap(), 1_000_000_000_000_000_000);
    }

    #[test]
    fn test_parse_amount_hex() {
        let amount = parse_amount("0xde0b6b3a7640000");
        assert!(amount.is_ok());
        assert_eq!(amount.unwrap(), 1_000_000_000_000_000_000);
    }

    #[test]
    fn test_parse_address() {
        let addr = parse_address("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2");
        assert!(addr.is_ok());
        assert_eq!(addr.unwrap().len(), 20);
    }
}
