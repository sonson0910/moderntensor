//! Query RPC Module - SDK-compatible query methods (query_*)
//!
//! This module handles all query_* RPC methods for SDK compatibility.
//! Refactored from server.rs to follow clean-code principles (SRP, max 20 lines per function).

use crate::helpers::parse_address;
use crate::types::{NeuronInfo, SubnetInfo};
use dashmap::DashMap;
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_consensus::{CommitRevealManager, ValidatorSet};
use parking_lot::RwLock;
use std::sync::Arc;

/// Shared context for query RPC handlers
/// Contains all necessary state references for query operations
pub struct QueryRpcContext {
    pub neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
    pub subnets: Arc<DashMap<u64, SubnetInfo>>,
    pub validators: Arc<RwLock<ValidatorSet>>,
    pub commit_reveal: Arc<RwLock<CommitRevealManager>>,
}

impl QueryRpcContext {
    pub fn new(
        neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
        subnets: Arc<DashMap<u64, SubnetInfo>>,
        validators: Arc<RwLock<ValidatorSet>>,
        commit_reveal: Arc<RwLock<CommitRevealManager>>,
    ) -> Self {
        Self {
            neurons,
            subnets,
            validators,
            commit_reveal,
        }
    }
}

/// Register all SDK-compatible query methods
/// Delegates to category-specific registration functions
pub fn register_query_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    register_neuron_query_methods(ctx, io);
    register_subnet_query_methods(ctx, io);
    register_stake_query_methods(ctx, io);
    register_hotkey_query_methods(ctx, io);
    register_weight_query_methods(ctx, io);
}

// =============================================================================
// NEURON QUERY METHODS
// =============================================================================

fn register_neuron_query_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    let neurons = ctx.neurons.clone();

    // query_neuron - Get specific neuron info
    io.add_sync_method("query_neuron", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or neuron_uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;

        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(serde_json::json!({
                "uid": neuron.uid,
                "address": neuron.address,
                "subnet_id": neuron.subnet_id,
                "stake": format!("0x{:x}", neuron.stake),
                "trust": neuron.trust,
                "rank": neuron.rank,
                "incentive": neuron.incentive,
                "dividends": neuron.dividends,
                "active": neuron.active,
                "endpoint": neuron.endpoint
            }))
        } else {
            Ok(Value::Null)
        }
    });

    let neurons = ctx.neurons.clone();

    // query_neuronCount - Get neuron count in subnet
    io.add_sync_method("query_neuronCount", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0];
        let count = neurons
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .count();
        Ok(Value::Number(count.into()))
    });

    let neurons = ctx.neurons.clone();

    // query_activeNeurons - Get active neuron UIDs
    io.add_sync_method("query_activeNeurons", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0];
        let active_uids: Vec<u64> = neurons
            .iter()
            .filter(|entry| entry.key().0 == subnet_id && entry.value().active)
            .map(|entry| entry.key().1)
            .collect();
        Ok(serde_json::to_value(active_uids).unwrap_or(Value::Array(vec![])))
    });

    register_neuron_metrics_methods(ctx, io);
}

fn register_neuron_metrics_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    let neurons = ctx.neurons.clone();

    // query_rank - Get neuron rank
    io.add_sync_method("query_rank", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or neuron_uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(serde_json::json!(neuron.rank as f64 / 65535.0))
        } else {
            Ok(Value::Null)
        }
    });

    let neurons = ctx.neurons.clone();

    // query_trust - Get neuron trust
    io.add_sync_method("query_trust", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or neuron_uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(serde_json::json!(neuron.trust))
        } else {
            Ok(Value::Null)
        }
    });

    let neurons = ctx.neurons.clone();

    // query_incentive - Get neuron incentive
    io.add_sync_method("query_incentive", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or neuron_uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(serde_json::json!(neuron.incentive))
        } else {
            Ok(Value::Null)
        }
    });

    let neurons = ctx.neurons.clone();

    // query_dividends - Get neuron dividends
    io.add_sync_method("query_dividends", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or neuron_uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(serde_json::json!(neuron.dividends))
        } else {
            Ok(Value::Null)
        }
    });

    let neurons = ctx.neurons.clone();

    // query_consensus - Get neuron consensus score
    io.add_sync_method("query_consensus", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or neuron_uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(serde_json::json!(neuron.consensus))
        } else {
            Ok(Value::Null)
        }
    });
}

// =============================================================================
// SUBNET QUERY METHODS
// =============================================================================

fn register_subnet_query_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    let subnets = ctx.subnets.clone();

    // query_allSubnets - Get all subnets
    io.add_sync_method("query_allSubnets", move |_params: Params| {
        let list: Vec<Value> = subnets
            .iter()
            .map(|entry| {
                let s = entry.value();
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "owner": s.owner,
                    "emission_rate": s.emission_rate,
                    "participant_count": s.participant_count,
                    "total_stake": format!("0x{:x}", s.total_stake)
                })
            })
            .collect();
        Ok(Value::Array(list))
    });

    let subnets = ctx.subnets.clone();

    // query_subnetExists - Check if subnet exists
    io.add_sync_method("query_subnetExists", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        Ok(Value::Bool(subnets.contains_key(&parsed[0])))
    });

    let subnets = ctx.subnets.clone();

    // query_subnetOwner - Get subnet owner
    io.add_sync_method("query_subnetOwner", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        if let Some(subnet) = subnets.get(&parsed[0]) {
            Ok(Value::String(subnet.owner.clone()))
        } else {
            Ok(Value::Null)
        }
    });

    let subnets = ctx.subnets.clone();

    // query_subnetEmission - Get subnet emission rate
    io.add_sync_method("query_subnetEmission", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        if let Some(subnet) = subnets.get(&parsed[0]) {
            Ok(Value::String(format!("0x{:x}", subnet.emission_rate)))
        } else {
            Ok(Value::Null)
        }
    });

    let subnets = ctx.subnets.clone();

    // query_subnetHyperparameters - Get subnet hyperparams
    io.add_sync_method("query_subnetHyperparameters", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        if let Some(subnet) = subnets.get(&parsed[0]) {
            Ok(serde_json::json!({
                "tempo": 360,
                "rho": 10,
                "kappa": 10,
                "immunity_period": 100,
                "max_allowed_validators": 64,
                "min_allowed_weights": 1,
                "max_weights_limit": 1000,
                "emission_rate": subnet.emission_rate
            }))
        } else {
            Ok(Value::Null)
        }
    });

    let subnets = ctx.subnets.clone();

    // query_subnetTempo - Get subnet tempo
    io.add_sync_method("query_subnetTempo", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        if subnets.contains_key(&parsed[0]) {
            Ok(Value::Number(360.into())) // Default tempo
        } else {
            Ok(Value::Null)
        }
    });
}

// =============================================================================
// STAKE QUERY METHODS
// =============================================================================

fn register_stake_query_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    let validators = ctx.validators.clone();

    // query_stakeForColdkeyAndHotkey - Get stake for coldkey-hotkey pair
    io.add_sync_method("query_stakeForColdkeyAndHotkey", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing coldkey or hotkey",
            ));
        }
        let hotkey = &parsed[1];
        let address = parse_address(hotkey)?;
        let validator_set = validators.read();
        let stake = validator_set
            .get_validator(&address)
            .map(|v| v.stake)
            .unwrap_or(0);
        Ok(Value::String(format!("0x{:x}", stake)))
    });

    let validators = ctx.validators.clone();

    // query_totalStakeForColdkey - Get total stake for coldkey
    io.add_sync_method("query_totalStakeForColdkey", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing coldkey"));
        }
        let address = parse_address(&parsed[0])?;
        let validator_set = validators.read();
        let stake = validator_set
            .get_validator(&address)
            .map(|v| v.stake)
            .unwrap_or(0);
        Ok(Value::String(format!("0x{:x}", stake)))
    });

    let validators = ctx.validators.clone();

    // query_totalStakeForHotkey - Get total stake for hotkey
    io.add_sync_method("query_totalStakeForHotkey", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing hotkey"));
        }
        let address = parse_address(&parsed[0])?;
        let validator_set = validators.read();
        let stake = validator_set
            .get_validator(&address)
            .map(|v| v.stake)
            .unwrap_or(0);
        Ok(Value::String(format!("0x{:x}", stake)))
    });

    let validators = ctx.validators.clone();

    // query_allStakeForColdkey - Get all stakes for coldkey
    io.add_sync_method("query_allStakeForColdkey", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing coldkey"));
        }
        let address = parse_address(&parsed[0])?;
        let validator_set = validators.read();
        let mut stakes = serde_json::Map::new();
        if let Some(v) = validator_set.get_validator(&address) {
            stakes.insert(
                parsed[0].clone(),
                serde_json::json!(format!("0x{:x}", v.stake)),
            );
        }
        Ok(Value::Object(stakes))
    });

    let validators = ctx.validators.clone();

    // query_allStakeForHotkey - Get all stakes for hotkey
    io.add_sync_method("query_allStakeForHotkey", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing hotkey"));
        }
        let address = parse_address(&parsed[0])?;
        let validator_set = validators.read();
        let mut stakes = serde_json::Map::new();
        if let Some(v) = validator_set.get_validator(&address) {
            stakes.insert(
                parsed[0].clone(),
                serde_json::json!(format!("0x{:x}", v.stake)),
            );
        }
        Ok(Value::Object(stakes))
    });
}

// =============================================================================
// HOTKEY QUERY METHODS
// =============================================================================

fn register_hotkey_query_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    let neurons = ctx.neurons.clone();

    // query_isHotkeyRegistered - Check if hotkey is registered
    io.add_sync_method("query_isHotkeyRegistered", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or hotkey",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let hotkey = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;
        let is_registered = neurons
            .iter()
            .any(|entry| entry.key().0 == subnet_id && entry.value().address == hotkey);
        Ok(Value::Bool(is_registered))
    });

    let neurons = ctx.neurons.clone();

    // query_uidForHotkey - Get UID for hotkey
    io.add_sync_method("query_uidForHotkey", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or hotkey",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let hotkey = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;
        let uid = neurons
            .iter()
            .find(|entry| entry.key().0 == subnet_id && entry.value().address == hotkey)
            .map(|entry| entry.key().1);
        match uid {
            Some(u) => Ok(Value::Number(u.into())),
            None => Ok(Value::Null),
        }
    });

    let neurons = ctx.neurons.clone();

    // query_hotkeyForUid - Get hotkey for UID
    io.add_sync_method("query_hotkeyForUid", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet_id or uid",
            ));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;
        if let Some(neuron) = neurons.get(&(subnet_id, neuron_uid)) {
            Ok(Value::String(neuron.address.clone()))
        } else {
            Ok(Value::Null)
        }
    });
}

// =============================================================================
// WEIGHT QUERY METHODS
// =============================================================================

fn register_weight_query_methods(ctx: &QueryRpcContext, io: &mut IoHandler) {
    let commit_reveal = ctx.commit_reveal.clone();

    // query_weightCommits - Get weight commits for a subnet
    io.add_sync_method("query_weightCommits", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0];

        let commits = commit_reveal.read().get_pending_commits(subnet_id);
        let epoch_state = commit_reveal.read().get_epoch_state(subnet_id);

        let mut result = serde_json::Map::new();

        if let Some(state) = epoch_state {
            result.insert("epochNumber".into(), serde_json::json!(state.epoch_number));
            result.insert("phase".into(), serde_json::json!(format!("{:?}", state.phase)));
            result.insert(
                "commitStartBlock".into(),
                serde_json::json!(state.commit_start_block),
            );
            result.insert(
                "revealStartBlock".into(),
                serde_json::json!(state.reveal_start_block),
            );
            result.insert(
                "finalizeBlock".into(),
                serde_json::json!(state.finalize_block),
            );
        }

        let commit_list: Vec<serde_json::Value> = commits
            .iter()
            .map(|c| {
                serde_json::json!({
                    "validator": format!("0x{}", hex::encode(c.validator.as_bytes())),
                    "commitHash": format!("0x{}", hex::encode(&c.commit_hash)),
                    "committedAt": c.committed_at,
                    "revealed": c.revealed
                })
            })
            .collect();

        result.insert("commits".into(), serde_json::json!(commit_list));
        result.insert("commitCount".into(), serde_json::json!(commits.len()));

        Ok(Value::Object(result))
    });

    // query_weightsVersion - Get weights version
    io.add_sync_method("query_weightsVersion", move |params: Params| {
        let parsed: Vec<u64> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        Ok(Value::Number(1.into())) // Version 1
    });
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

// Note: Helper functions for parsing params cannot work with jsonrpc_core
// because Params takes self by value in parse(). Each handler must inline parsing.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_rpc_context_creation() {
        // Test that QueryRpcContext can be created
        let neurons = Arc::new(DashMap::new());
        let subnets = Arc::new(DashMap::new());
        let validators = Arc::new(RwLock::new(ValidatorSet::new()));
        let commit_reveal = Arc::new(RwLock::new(CommitRevealManager::new(
            luxtensor_consensus::CommitRevealConfig::default(),
        )));

        let ctx = QueryRpcContext::new(neurons, subnets, validators, commit_reveal);
        assert!(ctx.neurons.is_empty());
        assert!(ctx.subnets.is_empty());
    }
}
