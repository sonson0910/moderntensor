// Neuron RPC handlers
// Extracted from server.rs

use crate::types::{NeuronInfo, SubnetInfo};
use jsonrpc_core::{Params, Value};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Register neuron-related RPC methods
pub fn register_neuron_handlers(
    io: &mut jsonrpc_core::IoHandler,
    neurons: Arc<RwLock<HashMap<(u64, u64), NeuronInfo>>>,
    subnets: Arc<RwLock<HashMap<u64, SubnetInfo>>>,
) {
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

    // neuron_register - Register neuron on subnet
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

        neurons_map.insert((subnet_id, neuron_uid), neuron);

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
}
