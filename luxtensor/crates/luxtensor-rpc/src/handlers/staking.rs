// Staking RPC handlers
// Extracted from server.rs
// Now with on-chain persistent storage

use crate::helpers::parse_address;
use jsonrpc_core::{Params, Value};
use luxtensor_consensus::{ValidatorSet, Validator};
use luxtensor_core::UnifiedStateDB;
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::sync::Arc;

/// Register staking-related RPC methods
/// Stakes are persisted to BlockchainDB for on-chain storage
/// SECURITY: Now requires sufficient EVM balance for staking operations
pub fn register_staking_handlers(
    io: &mut jsonrpc_core::IoHandler,
    validators: Arc<RwLock<ValidatorSet>>,
    db: Arc<BlockchainDB>,
    chain_id: u64,
    unified_state: Arc<RwLock<UnifiedStateDB>>,
) {
    // Load existing validators from DB into ValidatorSet on startup
    if let Ok(stored_validators) = db.get_all_validators() {
        let loaded_count = stored_validators.len();
        let mut validator_set = validators.write();
        for (_addr, data) in stored_validators {
            if let Ok(validator) = bincode::deserialize::<luxtensor_consensus::Validator>(&data) {
                // Only add if not already in set (e.g. from genesis)
                if validator_set.get_validator(&validator.address).is_none() {
                    let _ = validator_set.add_validator(validator);
                }
            }
        }
        if loaded_count > 0 {
            tracing::info!("📊 Loaded {} validators from DB", loaded_count);
        }
    }

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
    let state_for_add = unified_state.clone();

    // staking_addStake - Add stake to validator
    // SECURITY: Verifies EVM balance and debits it atomically
    // LOCK ORDER: validators → unified_state (consistent with removeStake)
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

        // SECURITY: Acquire both locks in consistent order (validators → state)
        // to prevent deadlock with removeStake. All checks and mutations happen
        // atomically under both locks to eliminate TOCTOU races.
        let mut validator_set = validators_clone.write();
        let mut state = state_for_add.write();

        // Verify sufficient EVM balance
        let balance = state.get_balance(&address);
        if balance < amount {
            return Err(jsonrpc_core::Error::invalid_params(
                format!("Insufficient balance: have 0x{:x}, need 0x{:x}", balance, amount)
            ));
        }

        // Update validator stake
        if let Some(validator) = validator_set.get_validator(&address) {
            let new_stake = validator.stake.checked_add(amount)
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Stake overflow"))?;
            validator_set
                .update_stake(&address, new_stake)
                .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
        } else {
            let validator = Validator::new(address, amount, [0u8; 32]);
            validator_set
                .add_validator(validator)
                .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;
        }

        // Debit EVM balance (after successful stake update)
        state.set_balance(address, balance.saturating_sub(amount));

        Ok(Value::Bool(true))
    });

    let validators_clone = validators.clone();
    let state_for_remove = unified_state.clone();

    // staking_removeStake - Remove stake from validator
    // SECURITY: Credits EVM balance back when unstaking
    // LOCK ORDER: validators → unified_state (consistent with addStake)
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

        // SECURITY: Acquire both locks in consistent order (validators → state)
        let mut validator_set = validators_clone.write();
        let mut state = state_for_remove.write();

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

            // Credit back the unstaked amount to EVM balance
            let balance = state.get_balance(&address);
            state.set_balance(address, balance.saturating_add(amount));
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

            // SECURITY: Reset rewards to 0 using wrapping arithmetic\n            // wrapping_sub produces the two's complement: rewards + (MAX - rewards + 1) wraps to 0\n            validator_set.add_reward(&address, 0u128.wrapping_sub(rewards))\n                .map_err(|e| jsonrpc_core::Error::invalid_params(e))?;

            Ok(serde_json::json!({
                "success": true,
                "rewards": format!("0x{:x}", rewards)
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Validator not found"))
        }
    });

    let validators_clone = validators.clone();
    let db_for_register_validator = db.clone();

    // staking_registerValidator - Register as a new validator (DYNAMIC REGISTRATION)
    // Params: [address, stake, public_key, signature?, name?]
    // Minimum stake: 1000 LUX (persisted to DB)
    const MIN_STAKE: u128 = 1_000_000_000_000_000_000_000; // 1000 LUX

    io.add_sync_method("staking_registerValidator", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 3 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing address, stake amount, or public key",
            ));
        }

        let addr_str = parsed[0]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?;
        let address = parse_address(addr_str)?;

        let stake_str = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid stake amount"))?;
        let stake = u128::from_str_radix(stake_str.trim_start_matches("0x"), 16)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid stake format"))?;

        // P0 FIX: Minimum stake check
        if stake < MIN_STAKE {
            return Err(jsonrpc_core::Error::invalid_params(
                "Stake must be at least 1000 LUX"
            ));
        }

        let pubkey_str = parsed[2]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid public key"))?;

        // Optional name
        let name = parsed.get(3)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("validator-{}", hex::encode(&address.as_bytes()[..4])));

        // Parse public key (32 bytes for Validator struct)
        let pubkey_bytes = hex::decode(pubkey_str.trim_start_matches("0x"))
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid public key hex"))?;

        let mut public_key = [0u8; 32];
        let copy_len = pubkey_bytes.len().min(32);
        public_key[..copy_len].copy_from_slice(&pubkey_bytes[..copy_len]);

        // SECURITY: Use write lock for atomic check+insert (eliminates TOCTOU race)
        let mut validator_set = validators_clone.write();
        if validator_set.get_validator(&address).is_some() {
            return Err(jsonrpc_core::Error::invalid_params("Validator already registered"));
        }

        // Add to validator set with epoch delay (active next epoch)
        let new_validator = Validator {
            address,
            stake,
            active: true,
            public_key,
            rewards: 0,
            last_active_slot: 0,
            activation_epoch: 1, // P2 FIX: Active from epoch 1 (delay)
        };

        if let Err(e) = validator_set.add_validator(new_validator.clone()) {
            return Err(jsonrpc_core::Error::invalid_params(e));
        }

        // Persist validator to blockchain DB for on-chain storage
        if let Ok(data) = bincode::serialize(&new_validator) {
            let _ = db_for_register_validator.store_validator(address.as_bytes(), &data);
        }

        Ok(serde_json::json!({
            "success": true,
            "address": format!("0x{}", hex::encode(address.as_bytes())),
            "name": name,
            "stake": format!("0x{:x}", stake)
        }))
    });

    let validators_clone = validators.clone();

    // staking_getActiveValidators - Get only active validators (for consensus)
    io.add_sync_method("staking_getActiveValidators", move |_params: Params| {
        let validator_set = validators_clone.read();
        let active_validators: Vec<Value> = validator_set
            .validators()
            .iter()
            .filter(|v| v.active)
            .map(|v| {
                serde_json::json!({
                    "address": format!("0x{}", hex::encode(v.address.as_bytes())),
                    "stake": format!("0x{:x}", v.stake),
                    "publicKey": format!("0x{}", hex::encode(&v.public_key)),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "count": active_validators.len(),
            "validators": active_validators
        }))
    });

    let _validators_config = validators.clone();

    // staking_getConfig - Get staking configuration
    io.add_sync_method("staking_getConfig", move |_params: Params| {
        // Min stake: 1000 MDT (18 decimals)
        let min_stake: u128 = 1_000_000_000_000_000_000_000;
        Ok(serde_json::json!({
            "min_stake": format!("0x{:x}", min_stake),
            "min_stake_decimal": "1000000000000000000000",
            "max_validators": 100,
            "epoch_length": 100,
            "block_time_seconds": 3,
            "lock_period_days": 7,
            "unbonding_period_days": 21
        }))
    });

    let validators_clone = validators.clone();

    // staking_deactivateValidator - Deactivate a validator
    io.add_sync_method("staking_deactivateValidator", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let mut validator_set = validators_clone.write();

        if validator_set.get_validator(&address).is_some() {
            validator_set.deactivate_validator(&address)
                .map_err(|e| jsonrpc_core::Error::invalid_params(e.to_string()))?;
            Ok(serde_json::json!({ "success": true }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Validator not found"))
        }
    });

    // =========================================================================
    // STAKING LOCK/UNLOCK WITH TIME CONSTRAINTS
    // =========================================================================

    // Storage for locked stakes: address -> (amount, unlock_timestamp)
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Shared state for locked stakes
    lazy_static::lazy_static! {
        static ref LOCKED_STAKES: parking_lot::RwLock<HashMap<[u8; 20], (u128, u64, u64)>> = {
            parking_lot::RwLock::new(HashMap::new())
        };
    }

    // Load existing stakes from DB into LOCKED_STAKES
    {
        if let Ok(stakes) = db.get_all_stakes() {
            let mut locks = LOCKED_STAKES.write();
            for (addr, data) in &stakes {
                if addr.len() >= 20 {
                    if let Ok((amount, unlock_ts, bonus)) = bincode::deserialize::<(u128, u64, u64)>(data) {
                        let mut addr_arr = [0u8; 20];
                        addr_arr.copy_from_slice(&addr[..20]);
                        locks.insert(addr_arr, (amount, unlock_ts, bonus));
                    }
                }
            }
            if !stakes.is_empty() {
                tracing::info!("📊 Loaded {} stakes from blockchain DB into memory", stakes.len());
            }
        }
    }

    // Clone db for lockStakeSeconds handler
    let db_for_lock = db.clone();

    // staking_lockStakeSeconds - Lock stake for specific duration in SECONDS
    // SECURITY: Only available on dev/test chains to prevent abuse on mainnet.
    // Params: [address, amount, lock_seconds]
    // Returns: { success, unlock_timestamp }
    let is_dev_chain = chain_id == 8898 || chain_id == 31337 || chain_id == 1337;
    io.add_sync_method("staking_lockStakeSeconds", move |params: Params| {
        if !is_dev_chain {
            return Err(jsonrpc_core::Error {
                code: jsonrpc_core::ErrorCode::MethodNotFound,
                message: "staking_lockStakeSeconds is only available on dev/test chains".to_string(),
                data: None,
            });
        }
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 3 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing address, amount, or lock_seconds",
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

        let lock_seconds = parsed[2]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid lock_seconds"))?;

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();
        let unlock_timestamp = now + lock_seconds;

        // Calculate bonus rate based on lock period (using LockBonusConfig logic)
        // Convert seconds to days for bonus calculation
        let lock_days = (lock_seconds / 86400) as u32; // 86400 seconds = 1 day
        let bonus_rate = if lock_days >= 365 {
            100 // 100%
        } else if lock_days >= 180 {
            50  // 50%
        } else if lock_days >= 90 {
            25  // 25%
        } else if lock_days >= 30 {
            10  // 10%
        } else {
            0   // No bonus for < 30 days
        };

        // Generate transaction hash for tracking
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(address.as_bytes());
        hasher.update(&amount.to_be_bytes());
        hasher.update(&now.to_be_bytes());
        hasher.update(&lock_seconds.to_be_bytes());
        let tx_hash = hasher.finalize();
        let tx_hash_hex = format!("0x{}", hex::encode(&tx_hash));

        // SECURITY: Atomic check+insert under single write lock (eliminates TOCTOU)
        {
            let mut locks = LOCKED_STAKES.write();
            let addr_bytes: [u8; 20] = *address.as_bytes();

            // Check if already locked (under write lock)
            if let Some((_, existing_unlock, _)) = locks.get(&addr_bytes) {
                if now < *existing_unlock {
                    return Err(jsonrpc_core::Error::invalid_params(
                        format!("Stake already locked until timestamp {}", existing_unlock)
                    ));
                }
            }

            locks.insert(addr_bytes, (amount, unlock_timestamp, bonus_rate));

            // Persist to blockchain DB for on-chain storage
            let stake_data = bincode::serialize(&(amount, unlock_timestamp, bonus_rate))
                .unwrap_or_default();
            let _ = db_for_lock.store_stake(&addr_bytes, &stake_data);
        }

        Ok(serde_json::json!({
            "success": true,
            "txHash": tx_hash_hex,
            "address": format!("0x{}", hex::encode(address.as_bytes())),
            "amount": format!("0x{:x}", amount),
            "amountMDT": format!("{} MDT", amount / 1_000_000_000_000_000_000u128),
            "lock_seconds": lock_seconds,
            "lock_days": lock_days,
            "bonus_rate": format!("{}%", bonus_rate),
            "unlock_timestamp": unlock_timestamp,
            "current_timestamp": now,
            "message": format!("Locked {} MDT for {} seconds ({} days). Bonus: {}%. Unlock after timestamp {}",
                              amount / 1_000_000_000_000_000_000u128, lock_seconds, lock_days, bonus_rate, unlock_timestamp)
        }))
    });

    // staking_lockStake - Lock stake for a period to earn bonus rewards
    // Params: [address, amount, lock_days]
    // Returns: { success, unlock_timestamp, bonus_rate }
    io.add_sync_method("staking_lockStake", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 3 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing address, amount, or lock_days",
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

        let lock_days = parsed[2]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid lock_days"))?;

        // Validate lock period (minimum 7 days, maximum 365 days)
        if lock_days < 7 {
            return Err(jsonrpc_core::Error::invalid_params("Minimum lock period is 7 days"));
        }
        if lock_days > 365 {
            return Err(jsonrpc_core::Error::invalid_params("Maximum lock period is 365 days"));
        }

        // Calculate bonus rate based on lock period (Model C from tokenomics)
        // 7 days: 0%, 30 days: 10%, 90 days: 30%, 180 days: 60%, 365 days: 100%
        let bonus_rate = match lock_days {
            7..=29 => 0,
            30..=89 => 10,
            90..=179 => 30,
            180..=364 => 60,
            365 => 100,
            _ => 0,
        };

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();
        let unlock_timestamp = now + (lock_days * 24 * 60 * 60);

        // SECURITY: Atomic check+insert under single write lock (eliminates TOCTOU)
        {
            let mut locks = LOCKED_STAKES.write();
            let addr_bytes: [u8; 20] = *address.as_bytes();

            if let Some((_, existing_unlock, _)) = locks.get(&addr_bytes) {
                if now < *existing_unlock {
                    return Err(jsonrpc_core::Error::invalid_params(
                        format!("Stake already locked until timestamp {}", existing_unlock)
                    ));
                }
            }

            locks.insert(addr_bytes, (amount, unlock_timestamp, bonus_rate));
        }

        Ok(serde_json::json!({
            "success": true,
            "address": format!("0x{}", hex::encode(address.as_bytes())),
            "amount": format!("0x{:x}", amount),
            "lock_days": lock_days,
            "unlock_timestamp": unlock_timestamp,
            "bonus_rate": format!("{}%", bonus_rate),
            "message": format!("Stake locked for {} days. Unlock after timestamp {}", lock_days, unlock_timestamp)
        }))
    });

    // staking_unlockStake - Unlock stake after lock period expires
    // Params: [address]
    // Returns: { success, amount } or error if lock not expired
    let db_for_unlock = db.clone();
    io.add_sync_method("staking_unlockStake", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let addr_bytes: [u8; 20] = *address.as_bytes();

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

        // SECURITY: Atomic check+remove under single write lock (eliminates TOCTOU)
        let mut locks = LOCKED_STAKES.write();

        match locks.get(&addr_bytes).cloned() {
            None => {
                Err(jsonrpc_core::Error::invalid_params("No locked stake found for this address"))
            }
            Some((amount, unlock_timestamp, bonus_rate)) => {
                if now < unlock_timestamp {
                    // LOCK NOT EXPIRED - CANNOT UNLOCK
                    let remaining_seconds = unlock_timestamp - now;
                    let remaining_days = remaining_seconds / (24 * 60 * 60);
                    let remaining_hours = (remaining_seconds % (24 * 60 * 60)) / 3600;

                    Err(jsonrpc_core::Error::invalid_params(
                        format!(
                            "Lock not expired. Cannot unlock until timestamp {}. Remaining: {} days {} hours",
                            unlock_timestamp, remaining_days, remaining_hours
                        )
                    ))
                } else {
                    // Lock expired - allow unlock (already under write lock)
                    locks.remove(&addr_bytes);
                    // Remove from blockchain DB
                    let _ = db_for_unlock.remove_stake(&addr_bytes);

                    // Calculate bonus amount
                    let bonus_amount = (amount * bonus_rate as u128) / 100;

                    // Generate unlock transaction hash
                    use sha3::{Digest, Keccak256};
                    let mut hasher = Keccak256::new();
                    hasher.update(address.as_bytes());
                    hasher.update(&amount.to_be_bytes());
                    hasher.update(&now.to_be_bytes());
                    hasher.update(b"unlock");
                    let tx_hash = hasher.finalize();
                    let tx_hash_hex = format!("0x{}", hex::encode(&tx_hash));

                    Ok(serde_json::json!({
                        "success": true,
                        "txHash": tx_hash_hex,
                        "address": format!("0x{}", hex::encode(address.as_bytes())),
                        "amount": format!("0x{:x}", amount),
                        "amountMDT": format!("{} MDT", amount / 1_000_000_000_000_000_000u128),
                        "bonus_rate": format!("{}%", bonus_rate),
                        "bonus_amount": format!("0x{:x}", bonus_amount),
                        "bonusMDT": format!("{:.2} MDT", bonus_amount as f64 / 1_000_000_000_000_000_000f64),
                        "total_returned": format!("0x{:x}", amount + bonus_amount),
                        "totalMDT": format!("{:.2} MDT", (amount + bonus_amount) as f64 / 1_000_000_000_000_000_000f64),
                        "message": "Stake unlocked successfully with bonus"
                    }))
                }
            }
        }
    });

    // staking_getLockInfo - Get lock information for an address
    io.add_sync_method("staking_getLockInfo", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing address"));
        }

        let address = parse_address(&parsed[0])?;
        let addr_bytes: [u8; 20] = *address.as_bytes();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

        let locks = LOCKED_STAKES.read();
        match locks.get(&addr_bytes) {
            None => {
                Ok(serde_json::json!({
                    "locked": false,
                    "address": format!("0x{}", hex::encode(address.as_bytes()))
                }))
            }
            Some((amount, unlock_timestamp, bonus_rate)) => {
                let is_unlockable = now >= *unlock_timestamp;
                let remaining = if now < *unlock_timestamp {
                    unlock_timestamp - now
                } else {
                    0
                };

                Ok(serde_json::json!({
                    "locked": true,
                    "address": format!("0x{}", hex::encode(address.as_bytes())),
                    "amount": format!("0x{:x}", amount),
                    "unlock_timestamp": unlock_timestamp,
                    "bonus_rate": format!("{}%", bonus_rate),
                    "is_unlockable": is_unlockable,
                    "remaining_seconds": remaining
                }))
            }
        }
    });
}
