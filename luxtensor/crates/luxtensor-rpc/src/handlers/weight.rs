// Weight RPC handlers
// Extracted from server.rs

use crate::types::WeightInfo;
use jsonrpc_core::{Params, Value};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Register weight-related RPC methods
pub fn register_weight_handlers(
    io: &mut jsonrpc_core::IoHandler,
    weights: Arc<RwLock<HashMap<(u64, u64), Vec<WeightInfo>>>>,
) {
    let weights_clone = weights.clone();

    // weight_getWeights - Get weights for neuron
    io.add_sync_method("weight_getWeights", move |params: Params| {
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

        let weights_map = weights_clone.read();

        if let Some(weight_list) = weights_map.get(&(subnet_id, neuron_uid)) {
            let weights_json: Vec<Value> = weight_list
                .iter()
                .map(|w| {
                    serde_json::json!({
                        "neuron_uid": w.neuron_uid,
                        "weight": w.weight
                    })
                })
                .collect();
            Ok(Value::Array(weights_json))
        } else {
            Ok(Value::Array(vec![]))
        }
    });

    let weights_clone = weights.clone();

    // weight_setWeights - Set weights for neuron
    io.add_sync_method("weight_setWeights", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 4 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing subnet ID, neuron UID, target UIDs, or weights",
            ));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let neuron_uid = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid neuron UID"))?;

        let target_uids: Vec<u64> = parsed[2]
            .as_array()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid target UIDs array"))?
            .iter()
            .map(|v| v.as_u64().ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid UID")))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let weight_values: Vec<u32> = parsed[3]
            .as_array()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid weights array"))?
            .iter()
            .map(|v| v.as_u64().and_then(|n| n.try_into().ok())
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid weight value")))
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if target_uids.len() != weight_values.len() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Mismatched UIDs and weights arrays",
            ));
        }

        let mut weights_map = weights_clone.write();

        let weight_info: Vec<WeightInfo> = target_uids
            .into_iter()
            .zip(weight_values.into_iter())
            .map(|(uid, weight)| WeightInfo {
                neuron_uid: uid,
                weight,
            })
            .collect();

        weights_map.insert((subnet_id, neuron_uid), weight_info);

        Ok(serde_json::json!({
            "success": true
        }))
    });

    let weights_clone = weights.clone();

    // weight_getAll - Get all weights for a subnet
    io.add_sync_method("weight_getAll", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let weights_map = weights_clone.read();

        let all_weights: Vec<Value> = weights_map
            .iter()
            .filter(|((sid, _), _)| *sid == subnet_id)
            .map(|((_, neuron_uid), weights)| {
                serde_json::json!({
                    "neuron_uid": neuron_uid,
                    "weights": weights.iter().map(|w| {
                        serde_json::json!({
                            "target_uid": w.neuron_uid,
                            "weight": w.weight
                        })
                    }).collect::<Vec<_>>()
                })
            })
            .collect();

        Ok(Value::Array(all_weights))
    });
}
