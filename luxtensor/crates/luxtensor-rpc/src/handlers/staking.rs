// Staking RPC handlers
// Extracted from server.rs

use crate::helpers::parse_address;
use jsonrpc_core::{Params, Value};
use luxtensor_consensus::{ValidatorSet, Validator};
use parking_lot::RwLock;
use std::sync::Arc;

/// Register staking-related RPC methods
pub fn register_staking_handlers(
    io: &mut jsonrpc_core::IoHandler,
    validators: Arc<RwLock<ValidatorSet>>,
) {
    let validators_clone = validators.clone();

    // staking_getTotalStake - Get total stake in network
    io.add_sync_method("staking_getTotalStake", move |_params: Params| {
        let validator_set = validators_clone.read();
        let total_stake = validator_set.total_stake();
        Ok(Value::String(format!("0x{:x}", total_stake)))
    });

    let validators_clone = validators.clone();

    // staking_getStake - Get stake for specific address
    io.add_sync_method("staking_getStake", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;

        let validator_set = validators_clone.read();
        let stake = validator_set
            .get_validator(&address)
            .map(|v| v.stake)
            .unwrap_or(0);

        Ok(Value::String(format!("0x{:x}", stake)))
    });

    let validators_clone = validators.clone();

    // staking_getValidators - Get list of validators
    io.add_sync_method("staking_getValidators", move |_params: Params| {
        let validator_set = validators_clone.read();
        let validators_list: Vec<Value> = validator_set
            .validators()
            .iter()
            .map(|v| {
                serde_json::json!({
                    "address": format!("0x{}", hex::encode(v.address.as_bytes())),
                    "stake": format!("0x{:x}", v.stake),
                    "active": v.active,
                    "rewards": format!("0x{:x}", v.rewards),
                    "publicKey": format!("0x{}", hex::encode(v.public_key)),
                })
            })
            .collect();

        Ok(Value::Array(validators_list))
    });

    let validators_clone = validators.clone();

    // staking_addStake - Add stake to validator
    io.add_sync_method("staking_addStake", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing address or amount",
            ));
        }

        let addr_str = parsed[0]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?;
        let address = parse_address(addr_str)?;

        let amount_str = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid amount"))?;
        let amount = u128::from_str_radix(amount_str.trim_start_matches("0x"), 16)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid amount format"))?;

        let mut validator_set = validators_clone.write();

        if let Some(validator) = validator_set.get_validator(&address) {
            let new_stake = validator.stake + amount;
            validator_set
                .update_stake(&address, new_stake)
                .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
        } else {
            let validator = Validator::new(address, amount, [0u8; 32]);
            validator_set
                .add_validator(validator)
                .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
        }

        Ok(Value::Bool(true))
    });

    let validators_clone = validators.clone();

    // staking_removeStake - Remove stake from validator
    io.add_sync_method("staking_removeStake", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing address or amount",
            ));
        }

        let addr_str = parsed[0]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?;
        let address = parse_address(addr_str)?;

        let amount_str = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid amount"))?;
        let amount = u128::from_str_radix(amount_str.trim_start_matches("0x"), 16)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid amount format"))?;

        let mut validator_set = validators_clone.write();

        if let Some(validator) = validator_set.get_validator(&address) {
            if validator.stake < amount {
                return Err(jsonrpc_core::Error::invalid_params("Insufficient stake"));
            }

            let new_stake = validator.stake - amount;
            if new_stake == 0 {
                validator_set
                    .remove_validator(&address)
                    .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
            } else {
                validator_set
                    .update_stake(&address, new_stake)
                    .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
            }
        } else {
            return Err(jsonrpc_core::Error::invalid_params("Validator not found"));
        }

        Ok(Value::Bool(true))
    });

    let validators_clone = validators.clone();

    // staking_claimRewards - Claim staking rewards
    io.add_sync_method("staking_claimRewards", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;

        let mut validator_set = validators_clone.write();

        if let Some(validator) = validator_set.get_validator(&address) {
            let rewards = validator.rewards;

            validator_set.add_reward(&address, 0u128.wrapping_sub(rewards))
                .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;

            Ok(serde_json::json!({
                "success": true,
                "rewards": format!("0x{:x}", rewards)
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Validator not found"))
        }
    });
}
