// Weight RPC handlers
// Extracted from server.rs
// Now with on-chain persistent storage

use crate::types::WeightInfo;
use dashmap::DashMap;
use jsonrpc_core::{Params, Value};
use luxtensor_storage::BlockchainDB;
use std::sync::Arc;

/// Register weight-related RPC methods
/// Weights are persisted to BlockchainDB for on-chain storage
pub fn register_weight_handlers(
    io: &mut jsonrpc_core::IoHandler,
    weights: Arc<DashMap<(u64, u64), Vec<WeightInfo>>>,
    db: Arc<BlockchainDB>,
) {
    // Load existing weights from DB into memory on startup
    if let Ok(stored_weights) = db.get_all_weights() {
        for ((subnet_id, uid), data) in stored_weights {
            if let Ok(weight_list) = bincode::deserialize::<Vec<WeightInfo>>(&data) {
                weights.insert((subnet_id, uid), weight_list);
            }
        }
        if !weights.is_empty() {
            tracing::info!("📊 Loaded {} weight entries from blockchain DB", weights.len());
        }
    }

    let weights_clone = weights.clone();

    // weight_getWeights - Get weights for neuron
    io.add_method("weight_getWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
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

        if let Some(weight_list) = weights_clone.get(&(subnet_id, neuron_uid)) {
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
        }
    });

    let weights_clone = weights.clone();
    let db_for_set = db.clone();

    // weight_setWeights - Set weights for neuron (persisted to DB)
    io.add_method("weight_setWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        let db_for_set = db_for_set.clone();
        async move {
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

        let weight_info: Vec<WeightInfo> = target_uids
            .into_iter()
            .zip(weight_values.into_iter())
            .map(|(uid, weight)| WeightInfo {
                neuron_uid: uid,
                weight,
            })
            .collect();

        weights_clone.insert((subnet_id, neuron_uid), weight_info.clone());

        // Persist weights to blockchain DB
        if let Ok(data) = bincode::serialize(&weight_info) {
            let _ = db_for_set.store_weights(subnet_id, neuron_uid, &data);
        }

        Ok(serde_json::json!({
            "success": true
        }))
        }
    });

    let weights_clone = weights.clone();

    // weight_getAll - Get all weights for a subnet
    io.add_method("weight_getAll", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }

        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let all_weights: Vec<Value> = weights_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let neuron_uid = entry.key().1;
                let weights = entry.value();
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
        }
    });

    // === SDK Aliases ===

    // query_getWeights - Alias for weight_getAll
    let weights_clone = weights.clone();
    io.add_method("query_getWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;
        let all_weights: Vec<Value> = weights_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let neuron_uid = entry.key().1;
                let weights = entry.value();
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
        }
    });

    // =========================================================================
    // SDK Compatibility Methods (neuron_client.py)
    // =========================================================================

    // weight_getCommits - Get weight commits for a subnet (SDK neuron_client.py)
    // Returns list of recent weight submissions with metadata
    let weights_clone = weights.clone();
    io.add_method("weight_getCommits", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        // Get all neurons that have set weights in this subnet
        let commits: Vec<Value> = weights_clone
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let neuron_uid = entry.key().1;
                let weights = entry.value();
                let total_weight: u64 = weights.iter().map(|w| w.weight as u64).sum();
                serde_json::json!({
                    "neuron_uid": neuron_uid,
                    "subnet_id": subnet_id,
                    "weight_count": weights.len(),
                    "total_weight": total_weight,
                    "committed": true,
                    // Note: In a full implementation, these would come from a commit-reveal store
                    "commit_hash": format!("0x{:0>64x}", neuron_uid),
                    "revealed": true
                })
            })
            .collect();

        let count = commits.len();
        Ok(serde_json::json!({
            "subnet_id": subnet_id,
            "commits": commits,
            "count": count
        }))
        }
    });
}
