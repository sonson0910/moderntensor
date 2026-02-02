// Neuron RPC handlers
// Extracted from server.rs
// Now with on-chain persistent storage

use crate::types::{NeuronInfo, SubnetInfo};
use jsonrpc_core::{Params, Value};
use luxtensor_storage::BlockchainDB;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Register neuron-related RPC methods
/// Neurons are persisted to BlockchainDB for on-chain storage
pub fn register_neuron_handlers(
    io: &mut jsonrpc_core::IoHandler,
    neurons: Arc<RwLock<HashMap<(u64, u64), NeuronInfo>>>,
    subnets: Arc<RwLock<HashMap<u64, SubnetInfo>>>,
    db: Arc<BlockchainDB>,
) {
    // Load existing neurons from DB into memory on startup
    if let Ok(stored_neurons) = db.get_all_neurons() {
        let mut neurons_map = neurons.write();
        for ((subnet_id, uid), data) in stored_neurons {
            if let Ok(neuron) = bincode::deserialize::<NeuronInfo>(&data) {
                neurons_map.insert((subnet_id, uid), neuron);
            }
        }
        if !neurons_map.is_empty() {
            tracing::info!("📊 Loaded {} neurons from blockchain DB", neurons_map.len());
        }
    }

    let neurons_clone = neurons.clone();

    // neuron_getInfo - Get neuron information
    io.add_sync_method("neuron_getInfo", move |params: Params| {
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

        let neurons_map = neurons_clone.read();

        if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
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
    });

    let neurons_clone = neurons.clone();

    // neuron_listBySubnet - List neurons in subnet
    io.add_sync_method("neuron_listBySubnet", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let neurons_map = neurons_clone.read();

        let neurons_list: Vec<Value> = neurons_map
            .iter()
            .filter(|((sid, _), _)| *sid == subnet_id)
            .map(|(_, neuron)| {
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
    });

    let neurons_clone = neurons.clone();
    let subnets_clone = subnets.clone();
    let db_for_register = db.clone();

    // neuron_register - Register neuron on subnet (persisted to DB)
    io.add_sync_method("neuron_register", move |params: Params| {
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

        let mut neurons_map = neurons_clone.write();
        let mut subnets_map = subnets_clone.write();

        // Check subnet exists
        if !subnets_map.contains_key(&subnet_id) {
            return Err(jsonrpc_core::Error::invalid_params("Subnet not found"));
        }

        // Find next UID for this subnet
        let neuron_uid = neurons_map
            .keys()
            .filter(|(sid, _)| *sid == subnet_id)
            .map(|(_, uid)| uid)
            .max()
            .map(|max_uid| max_uid + 1)
            .unwrap_or(0);

        let neuron = NeuronInfo {
            uid: neuron_uid,
            address,
            subnet_id,
            stake,
            trust: 0.0,
            rank: 0,
            incentive: 0.0,
            dividends: 0.0,
            active: true,
            endpoint,
        };

        neurons_map.insert((subnet_id, neuron_uid), neuron.clone());

        // Persist neuron to blockchain DB
        if let Ok(data) = bincode::serialize(&neuron) {
            let _ = db_for_register.store_neuron(subnet_id, neuron_uid, &data);
        }

        // Update subnet participant count
        if let Some(subnet) = subnets_map.get_mut(&subnet_id) {
            subnet.participant_count += 1;
            subnet.total_stake += stake;
        }

        Ok(serde_json::json!({
            "success": true,
            "neuron_uid": neuron_uid
        }))
    });

    let neurons_clone = neurons.clone();

    // neuron_getCount - Get neuron count for subnet
    io.add_sync_method("neuron_getCount", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let neurons_map = neurons_clone.read();
        let count = neurons_map
            .keys()
            .filter(|(sid, _)| *sid == subnet_id)
            .count();

        Ok(Value::Number(count.into()))
    });

    // === SDK Aliases ===

    // query_getNeurons - Alias for neuron_listBySubnet
    let neurons_clone = neurons.clone();
    io.add_sync_method("query_getNeurons", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;
        let neurons_map = neurons_clone.read();
        let neurons_list: Vec<Value> = neurons_map
            .iter()
            .filter(|((sid, _), _)| *sid == subnet_id)
            .map(|(_, neuron)| {
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
    });

    // query_getNeuronInfo - Alias for neuron_getInfo
    let neurons_clone = neurons.clone();
    io.add_sync_method("query_getNeuronInfo", move |params: Params| {
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
        let neurons_map = neurons_clone.read();
        if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
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
    });

    // =========================================================================
    // SDK Compatibility Aliases (neuron_mixin.py / neuron_client.py)
    // =========================================================================

    // neuron_get - Alias for neuron_getInfo (SDK uses this name)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_get", move |params: Params| {
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
        let neurons_map = neurons_clone.read();
        if let Some(neuron) = neurons_map.get(&(subnet_id, neuron_uid)) {
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
    });

    // neuron_getAll - Alias for neuron_listBySubnet (SDK uses this name)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_getAll", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neurons_map = neurons_clone.read();
        let neurons_list: Vec<Value> = neurons_map
            .iter()
            .filter(|((sid, _), _)| *sid == subnet_id)
            .map(|(_, neuron)| {
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
    });

    // neuron_exists - Check if neuron exists (SDK uses this)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_exists", move |params: Params| {
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
        let neurons_map = neurons_clone.read();
        let exists = neurons_map.contains_key(&(subnet_id, neuron_uid));
        Ok(Value::Bool(exists))
    });

    // neuron_getByHotkey - Get neuron by hotkey address (SDK neuron_client.py)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_getByHotkey", move |params: Params| {
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

        let neurons_map = neurons_clone.read();
        let neuron = neurons_map
            .iter()
            .find(|((sid, _), n)| *sid == subnet_id && n.address == hotkey)
            .map(|(_, n)| n);

        if let Some(neuron) = neuron {
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
    });

    // neuron_getActive - Get active neuron UIDs (SDK neuron_client.py)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_getActive", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let neurons_map = neurons_clone.read();
        let active_uids: Vec<u64> = neurons_map
            .iter()
            .filter(|((sid, _), n)| *sid == subnet_id && n.active)
            .map(|((_, uid), _)| *uid)
            .collect();
        Ok(serde_json::json!(active_uids))
    });

    // neuron_count - Get neuron count (SDK neuron_client.py uses this name)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_count", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        let neurons_map = neurons_clone.read();

        if parsed.is_empty() {
            // No subnet specified - return total count
            let count = neurons_map.len();
            return Ok(Value::Number(count.into()));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
        let count = neurons_map
            .keys()
            .filter(|(sid, _)| *sid == subnet_id)
            .count();
        Ok(Value::Number(count.into()))
    });

    // neuron_batchGet - Batch get neurons by UIDs (SDK neuron_client.py)
    let neurons_clone = neurons.clone();
    io.add_sync_method("neuron_batchGet", move |params: Params| {
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

        let neurons_map = neurons_clone.read();
        let results: Vec<Value> = uids
            .iter()
            .filter_map(|uid| neurons_map.get(&(subnet_id, *uid)))
            .map(|neuron| {
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
    });
}

