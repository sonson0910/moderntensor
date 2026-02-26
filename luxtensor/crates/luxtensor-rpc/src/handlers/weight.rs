// Weight RPC handlers
// Dual-write: BlockchainDB (legacy compat) + MetagraphDB (lux_* source of truth)
// SECURITY: weight_setWeights requires timestamp + ECDSA signature

use crate::helpers::verify_caller_signature;
use crate::types::WeightInfo;
use dashmap::DashMap;
use jsonrpc_core::{Params, Value};
use luxtensor_storage::{BlockchainDB, MetagraphDB, NeuronData};
use std::sync::Arc;

/// Register weight-related RPC methods.
///
/// Weights are persisted to both BlockchainDB (legacy) and MetagraphDB.
/// The lux_getWeights / lux_getAllWeights methods in metagraph.rs read from MetagraphDB.
pub fn register_weight_handlers(
    io: &mut jsonrpc_core::IoHandler,
    weights: Arc<DashMap<(u64, u64), Vec<WeightInfo>>>,
    db: Arc<BlockchainDB>,
    metagraph: Arc<MetagraphDB>,
) {
    // =========================================================================
    // weight_setWeights — set weights from a neuron, dual-write to both DBs
    // SECURITY: Requires timestamp + ECDSA signature from neuron owner (hotkey)
    // Params: [subnet_id, uid, weights_array, timestamp, signature]
    // weights_array: [[to_uid, weight_u16], ...]
    // =========================================================================
    let weights_clone = weights.clone();
    let db_for_set = db.clone();
    let metagraph_for_set = metagraph.clone();
    io.add_method("weight_setWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        let db_for_set = db_for_set.clone();
        let metagraph_for_set = metagraph_for_set.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            // Requires: subnet_id, uid, weights[], timestamp, signature
            if parsed.len() < 5 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing params: subnet_id, uid, weights, timestamp, signature",
                ));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;

            let weights_arr = parsed[2]
                .as_array()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid weights array"))?;

            // Parse timestamp (replay-attack protection)
            let timestamp_str = parsed[3]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid timestamp"))?;
            let timestamp: u64 = timestamp_str.parse()
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid timestamp format"))?;
            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now_unix > timestamp + 300 || timestamp > now_unix + 60 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Signature expired or future timestamp (max 5 min)"
                ));
            }

            // Lookup neuron to get the authoritative hotkey address
            let neuron_data: Option<NeuronData> = metagraph_for_set
                .get_neuron(subnet_id, uid)
                .unwrap_or(None);
            let hotkey_addr = match neuron_data {
                Some(ref nd) => {
                    crate::helpers::parse_address(
                        &format!("0x{}", hex::encode(nd.hotkey))
                    )?
                }
                None => {
                    return Err(jsonrpc_core::Error::invalid_params(
                        format!("Neuron uid={} not found in subnet={}", uid, subnet_id)
                    ));
                }
            };

            // Verify signature — caller must own the neuron's hotkey
            let signature = parsed[4]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid signature"))?;
            let message = format!(
                "weight_setWeights:{}:{}:{}:{}",
                hex::encode(neuron_data.as_ref().unwrap().hotkey),
                subnet_id,
                uid,
                timestamp
            );
            verify_caller_signature(&hotkey_addr, &message, signature, 0)
                .or_else(|_| verify_caller_signature(&hotkey_addr, &message, signature, 1))
                .map_err(|_| jsonrpc_core::Error::invalid_params(
                    "Signature verification failed — caller must own neuron hotkey"
                ))?;

            // Parse and validate weights: each entry is [to_uid, weight_value]
            let mut weight_list: Vec<WeightInfo> = Vec::new();
            let mut metagraph_weights: Vec<(u64, u16)> = Vec::new();

            for w in weights_arr {
                let arr = w
                    .as_array()
                    .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid weight entry"))?;
                if arr.len() < 2 {
                    continue;
                }
                let to_uid = arr[0]
                    .as_u64()
                    .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid to_uid"))?;
                let weight_raw = arr[1].as_u64().unwrap_or(0);
                // Validate weight range (u16 max = 65535)
                if weight_raw > 65535 {
                    return Err(jsonrpc_core::Error::invalid_params(
                        format!("Weight value {} exceeds max 65535 for to_uid={}", weight_raw, to_uid)
                    ));
                }
                let weight_val = weight_raw as u32;
                weight_list.push(WeightInfo {
                    neuron_uid: to_uid,
                    weight: weight_val,
                });
                metagraph_weights.push((to_uid, weight_val as u16));
            }

            // Require at least one non-zero weight entry
            if weight_list.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Weights array must not be empty"
                ));
            }
            let total_weight: u32 = weight_list.iter().map(|w| w.weight).sum();
            if total_weight == 0 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Total weights must be greater than zero"
                ));
            }

            weights_clone.insert((subnet_id, uid), weight_list.clone());

            // Persist to legacy BlockchainDB
            if let Ok(data) = bincode::serialize(&weight_list) {
                let _ = db_for_set.store_weights(subnet_id, uid, &data);
            }

            // Dual-write to MetagraphDB (lux_getWeights reads from here)
            if let Err(e) = metagraph_for_set.store_weights(subnet_id, uid, &metagraph_weights) {
                tracing::warn!(
                    "MetagraphDB store_weights failed for subnet={} uid={}: {}",
                    subnet_id, uid, e
                );
            } else {
                tracing::info!(
                    "✅ Weights uid={} subnet={} set by owner ({} entries, total={})",
                    uid, subnet_id, metagraph_weights.len(), total_weight
                );
            }

            Ok(serde_json::json!({
                "success": true,
                "subnet_id": subnet_id,
                "uid": uid,
                "weight_count": weight_list.len(),
                "total_weight": total_weight,
            }))
        }
    });

    // =========================================================================
    // weight_getWeights — get weights set by a neuron (reads from DashMap cache)
    // =========================================================================
    let weights_clone = weights.clone();
    io.add_method("weight_getWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id, uid"));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;

            if let Some(w) = weights_clone.get(&(subnet_id, uid)) {
                let result: Vec<Value> = w
                    .iter()
                    .map(|wi| {
                        serde_json::json!({
                            "neuron_uid": wi.neuron_uid,
                            "weight": wi.weight,
                        })
                    })
                    .collect();
                Ok(Value::Array(result))
            } else {
                Ok(Value::Array(vec![]))
            }
        }
    });

    // =========================================================================
    // weight_getAllWeights — get all weights for a subnet (reads from DashMap cache)
    // =========================================================================
    let weights_clone = weights.clone();
    io.add_method("weight_getAllWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            let subnet_id = if parsed.is_empty() {
                0u64
            } else {
                parsed[0].as_u64().unwrap_or(0)
            };

            let result: Vec<Value> = weights_clone
                .iter()
                .filter(|e| e.key().0 == subnet_id)
                .flat_map(|entry| {
                    let from_uid = entry.key().1;
                    entry
                        .value()
                        .iter()
                        .map(move |wi| {
                            serde_json::json!({
                                "from_uid": from_uid,
                                "neuron_uid": wi.neuron_uid,
                                "weight": wi.weight,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            Ok(Value::Array(result))
        }
    });

    // =========================================================================
    // SDK Aliases
    // =========================================================================

    // query_getWeights — alias for weight_getWeights
    let weights_clone = weights.clone();
    io.add_method("query_getWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id, uid"));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;
            if let Some(w) = weights_clone.get(&(subnet_id, uid)) {
                let result: Vec<Value> = w
                    .iter()
                    .map(|wi| {
                        serde_json::json!({
                            "neuron_uid": wi.neuron_uid,
                            "weight": wi.weight,
                        })
                    })
                    .collect();
                Ok(Value::Array(result))
            } else {
                Ok(Value::Array(vec![]))
            }
        }
    });

    // query_getAllWeights — alias for weight_getAllWeights
    let weights_clone = weights.clone();
    io.add_method("query_getAllWeights", move |params: Params| {
        let weights_clone = weights_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            let subnet_id = if parsed.is_empty() {
                0u64
            } else {
                parsed[0].as_u64().unwrap_or(0)
            };
            let result: Vec<Value> = weights_clone
                .iter()
                .filter(|e| e.key().0 == subnet_id)
                .flat_map(|entry| {
                    let from_uid = entry.key().1;
                    entry
                        .value()
                        .iter()
                        .map(move |wi| {
                            serde_json::json!({
                                "from_uid": from_uid,
                                "neuron_uid": wi.neuron_uid,
                                "weight": wi.weight,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .collect();
            Ok(Value::Array(result))
        }
    });
}
