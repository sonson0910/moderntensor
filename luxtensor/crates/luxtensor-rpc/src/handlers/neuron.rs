// Neuron RPC handlers
// Extracted from server.rs
// Now with on-chain persistent storage

use crate::types::{NeuronInfo, SubnetInfo};
use dashmap::DashMap;
use jsonrpc_core::{Params, Value};
use luxtensor_storage::BlockchainDB;
use std::sync::Arc;

/// Register neuron-related RPC methods
/// Neurons are persisted to BlockchainDB for on-chain storage
pub fn register_neuron_handlers(
    io: &mut jsonrpc_core::IoHandler,
    neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
    subnets: Arc<DashMap<u64, SubnetInfo>>,
    db: Arc<BlockchainDB>,
) {
    // Load existing neurons from DB into memory on startup
    if let Ok(stored_neurons) = db.get_all_neurons() {
        for ((subnet_id, uid), data) in stored_neurons {
            if let Ok(neuron) = bincode::deserialize::<NeuronInfo>(&data) {
                neurons.insert((subnet_id, uid), neuron);
            }
        }
        if !neurons.is_empty() {
            tracing::info!("📊 Loaded {} neurons from blockchain DB", neurons.len());
        }
    }

    let neurons_clone = neurons.clone();

    // neuron_getInfo - Get neuron information
    io.add_method("neuron_getInfo", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet ID or neuron UID",
            ));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron UID"))?;

        if let Some(neuron) = neurons_clone.get(&(subnet_id, neuron_uid)) {
            let neuron_json = serde_json::json!({
                "uid": neuron.uid,
                "address": neuron.address,
                "subnet_id": neuron.subnet_id,
                "stake": format!("0x{:x}", neuron.stake),
                "trust": neuron.trust,
                "rank": neuron.rank,
                "incentive": neuron.incentive,
                "dividends": neuron.dividends,
                "active": neuron.active,
                "endpoint": neuron.endpoint,
            });
            Ok(neuron_json)
        } else {
            Ok(Value::Null)
        }
        }
    });

    let neurons_clone = neurons.clone();

    // neuron_listBySubnet - List neurons in subnet
    io.add_method("neuron_listBySubnet", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let neurons_list: Vec<Value> = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let neuron = entry.value();
                serde_json::json!({
                    "uid": neuron.uid,
                    "address": neuron.address,
                    "stake": format!("0x{:x}", neuron.stake),
                    "active": neuron.active,
                    "rank": neuron.rank,
                })
            })
            .collect();

        Ok(Value::Array(neurons_list))
        }
    });

    let neurons_clone = neurons.clone();
    let subnets_clone = subnets.clone();
    let db_for_register = db.clone();

    // neuron_register - Register neuron on subnet (persisted to DB)
    io.add_method("neuron_register", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        let subnets_clone = subnets_clone.clone();
        let db_for_register = db_for_register.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 3 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet ID, address, or stake",
            ));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let address = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid address"))?
            .to_string();

        let stake_str = parsed[2]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid stake"))?;
        let stake = u128::from_str_radix(stake_str.trim_start_matches("0x"), 16)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid stake format"))?;

        let endpoint = parsed.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());

        // Check subnet exists
        if !subnets_clone.contains_key(&subnet_id) {
            return Err(jsonrpc_core::Error::invalid_params("Subnet not found"));
        }

        // Find next UID for this subnet
        let neuron_uid = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| entry.key().1)
            .max()
            .map(|max_uid| max_uid + 1)
            .unwrap_or(0);

        let neuron = NeuronInfo {
            uid: neuron_uid,
            address,
            subnet_id,
            stake,
            trust: 0.0,
            consensus: 0.0,
            rank: 0,
            incentive: 0.0,
            dividends: 0.0,
            active: true,
            endpoint,
        };

        neurons_clone.insert((subnet_id, neuron_uid), neuron.clone());

        // Persist neuron to blockchain DB
        if let Ok(data) = bincode::serialize(&neuron) {
            let _ = db_for_register.store_neuron(subnet_id, neuron_uid, &data);
        }

        // Update subnet participant count
        if let Some(mut subnet) = subnets_clone.get_mut(&subnet_id) {
            subnet.participant_count += 1;
            subnet.total_stake += stake;
        }

        Ok(serde_json::json!({
            "success": true,
            "neuron_uid": neuron_uid
        }))
        }
    });

    let neurons_clone = neurons.clone();

    // neuron_getCount - Get neuron count for subnet
    io.add_method("neuron_getCount", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let count = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .count();

        Ok(Value::Number(count.into()))
        }
    });

    // === SDK Aliases ===

    // query_getNeurons - Alias for neuron_listBySubnet
    let neurons_clone = neurons.clone();
    io.add_method("query_getNeurons", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;
        let neurons_list: Vec<Value> = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let neuron = entry.value();
                serde_json::json!({
                    "uid": neuron.uid,
                    "address": neuron.address,
                    "stake": format!("0x{:x}", neuron.stake),
                    "active": neuron.active,
                    "rank": neuron.rank,
                })
            })
            .collect();
        Ok(Value::Array(neurons_list))
        }
    });

    // query_getNeuronInfo - Alias for neuron_getInfo
    let neurons_clone = neurons.clone();
    io.add_method("query_getNeuronInfo", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID or neuron UID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron UID"))?;
        if let Some(neuron) = neurons_clone.get(&(subnet_id, neuron_uid)) {
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
            }))
        } else {
            Ok(Value::Null)
        }
        }
    });

    // =========================================================================
    // SDK Compatibility Aliases (neuron_mixin.py / neuron_client.py)
    // =========================================================================

    // neuron_get - Alias for neuron_getInfo (SDK uses this name)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_get", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        if let Some(neuron) = neurons_clone.get(&(subnet_id, neuron_uid)) {
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
                "endpoint": neuron.endpoint,
            }))
        } else {
            Ok(Value::Null)
        }
        }
    });

    // neuron_getAll - Alias for neuron_listBySubnet (SDK uses this name)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_getAll", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neurons_list: Vec<Value> = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let neuron = entry.value();
                serde_json::json!({
                    "uid": neuron.uid,
                    "address": neuron.address,
                    "stake": format!("0x{:x}", neuron.stake),
                    "trust": neuron.trust,
                    "rank": neuron.rank,
                    "incentive": neuron.incentive,
                    "dividends": neuron.dividends,
                    "active": neuron.active,
                })
            })
            .collect();
        Ok(Value::Array(neurons_list))
        }
    });

    // neuron_exists - Check if neuron exists (SDK uses this)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_exists", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or neuron_uid"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron_uid"))?;
        let exists = neurons_clone.contains_key(&(subnet_id, neuron_uid));
        Ok(Value::Bool(exists))
        }
    });

    // neuron_getByHotkey - Get neuron by hotkey address (SDK neuron_client.py)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_getByHotkey", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or hotkey"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let hotkey = parsed[1]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;

        let neuron = neurons_clone
            .iter()
            .find(|entry| entry.key().0 == subnet_id && entry.value().address == hotkey);

        if let Some(entry) = neuron {
            let neuron = entry.value();
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
            }))
        } else {
            Ok(Value::Null)
        }
        }
    });

    // neuron_getActive - Get active neuron UIDs (SDK neuron_client.py)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_getActive", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let active_uids: Vec<u64> = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id && entry.value().active)
            .map(|entry| entry.key().1)
            .collect();
        Ok(serde_json::json!(active_uids))
        }
    });

    // neuron_count - Get neuron count (SDK neuron_client.py uses this name)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_count", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;

        if parsed.is_empty() {
            // No subnet specified - return total count
            let count = neurons_clone.len();
            return Ok(Value::Number(count.into()));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let count = neurons_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .count();
        Ok(Value::Number(count.into()))
        }
    });

    // neuron_batchGet - Batch get neurons by UIDs (SDK neuron_client.py)
    let neurons_clone = neurons.clone();
    io.add_method("neuron_batchGet", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id or uids"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let uids: Vec<u64> = parsed[1]
            .as_array()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uids array"))?
            .iter()
            .filter_map(|v| v.as_u64())
            .collect();

        let results: Vec<Value> = uids
            .iter()
            .filter_map(|uid| neurons_clone.get(&(subnet_id, *uid)))
            .map(|entry| {
                let neuron = entry.value();
                serde_json::json!({
                    "uid": neuron.uid,
                    "address": neuron.address,
                    "subnet_id": neuron.subnet_id,
                    "stake": format!("0x{:x}", neuron.stake),
                    "trust": neuron.trust,
                    "rank": neuron.rank,
                    "incentive": neuron.incentive,
                    "dividends": neuron.dividends,
                    "active": neuron.active,
                })
            })
            .collect();
        Ok(Value::Array(results))
        }
    });
}
