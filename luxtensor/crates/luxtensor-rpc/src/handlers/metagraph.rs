// Metagraph RPC handlers
// Exposes metagraph data stored in RocksDB via `lux_*` and `metagraph_*` namespaces
//
// ## Methods
//
// ### lux_* namespace (primary)
// | Method               | Params                           | Returns                               |
// |----------------------|----------------------------------|---------------------------------------|
// | `lux_getSubnet`      | [subnet_id]                      | SubnetData as JSON                    |
// | `lux_getSubnetInfo`  | [subnet_id]                      | alias lux_getSubnet                   |
// | `lux_listSubnets`    | []                               | Array of SubnetData                   |
// | `lux_getSubnetCount` | []                               | { count: N }                          |
// | `lux_getNeurons`     | [subnet_id]                      | Array of NeuronData                   |
// | `lux_getNeuron`      | [subnet_id, uid]                 | NeuronData or null                    |
// | `lux_getNeuronCount` | [subnet_id]                      | { count: N }                          |
// | `lux_getWeights`     | [subnet_id, from_uid]            | Array of WeightData from that neuron  |
// | `lux_getAllWeights`  | [subnet_id]                      | All WeightData in subnet              |
// | `lux_getEmissions`   | [subnet_id]                      | { subnet_id, emission_rate, ... }     |
// | `lux_getMetagraph`   | [subnet_id]                      | Full metagraph snapshot               |
//
// ### metagraph_* namespace
// | Method                | Params     | Returns                               |
// |-----------------------|------------|---------------------------------------|
// | `metagraph_getState`  | [subnet_id]| Full metagraph state (same as above)  |
// | `metagraph_getWeights`| [subnet_id]| All weights in subnet                 |

use jsonrpc_core::{Params, Value};
use luxtensor_consensus::YumaConsensus;
use luxtensor_storage::{MetagraphDB, SubnetData, NeuronData, WeightData};
use std::sync::Arc;

/// Register lux_* and metagraph_* RPC methods backed by MetagraphDB (RocksDB)
pub fn register_metagraph_methods(
    io: &mut jsonrpc_core::IoHandler,
    metagraph: Arc<MetagraphDB>,
) {
    // =========================================================================
    // lux_getSubnet — query a single subnet from persistent storage
    // Params: [subnet_id: number]   Returns: SubnetData | null
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getSubnet", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            match db.get_subnet(subnet_id) {
                Ok(Some(s)) => Ok(subnet_to_json(&s)),
                Ok(None) => Ok(Value::Null),
                Err(e) => {
                    tracing::error!("lux_getSubnet error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getSubnetInfo — alias for lux_getSubnet (SDK compat)
    // Params: [subnet_id: number]   Returns: SubnetData | null
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getSubnetInfo", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            match db.get_subnet(subnet_id) {
                Ok(Some(s)) => Ok(subnet_to_json(&s)),
                Ok(None) => Ok(Value::Null),
                Err(e) => {
                    tracing::error!("lux_getSubnetInfo error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_listSubnets — list all subnets from persistent storage
    // Params: []   Returns: SubnetData[]
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_listSubnets", move |_params: Params| {
        let db = db.clone();
        async move {
            match db.get_all_subnets() {
                Ok(subnets) => Ok(Value::Array(subnets.iter().map(subnet_to_json).collect())),
                Err(e) => {
                    tracing::error!("lux_listSubnets error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getSubnetCount — count of all subnets
    // Params: []   Returns: { count: N }
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getSubnetCount", move |_params: Params| {
        let db = db.clone();
        async move {
            match db.get_all_subnets() {
                Ok(subnets) => Ok(serde_json::json!({ "count": subnets.len() })),
                Err(e) => {
                    tracing::error!("lux_getSubnetCount error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getNeurons — all neurons in a subnet
    // Params: [subnet_id: number]   Returns: NeuronData[]
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getNeurons", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            match db.get_neurons_by_subnet(subnet_id) {
                Ok(neurons) => Ok(Value::Array(neurons.iter().map(neuron_to_json).collect())),
                Err(e) => {
                    tracing::error!("lux_getNeurons error for subnet {}: {}", subnet_id, e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getNeuron — single neuron by subnet_id + uid
    // Params: [subnet_id: number, uid: number]   Returns: NeuronData | null
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getNeuron", move |params: Params| {
        let db = db.clone();
        async move {
            let (subnet_id, uid) = parse_two_u64(params, "subnet_id", "uid")?;
            match db.get_neuron(subnet_id, uid) {
                Ok(Some(n)) => Ok(neuron_to_json(&n)),
                Ok(None) => Ok(Value::Null),
                Err(e) => {
                    tracing::error!("lux_getNeuron error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getNeuronCount — count neurons in a subnet
    // Params: [subnet_id: number]   Returns: { subnet_id, count }
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getNeuronCount", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            match db.get_neurons_by_subnet(subnet_id) {
                Ok(neurons) => Ok(serde_json::json!({
                    "subnet_id": subnet_id,
                    "count": neurons.len(),
                })),
                Err(e) => {
                    tracing::error!("lux_getNeuronCount error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getWeights — weights set BY a specific neuron
    // Params: [subnet_id: number, from_uid: number]
    // Returns: { from_uid, to_uid, weight, updated_at }[]
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getWeights", move |params: Params| {
        let db = db.clone();
        async move {
            let (subnet_id, from_uid) = parse_two_u64(params, "subnet_id", "from_uid")?;
            match db.get_weights(subnet_id, from_uid) {
                Ok(weights) => Ok(Value::Array(weights.iter().map(weight_to_json).collect())),
                Err(e) => {
                    tracing::error!("lux_getWeights error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getAllWeights — all weights in a subnet (across all neurons)
    // Params: [subnet_id: number]
    // Returns: { from_uid, to_uid, weight, updated_at }[]
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getAllWeights", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            let neurons = db.get_neurons_by_subnet(subnet_id).map_err(|e| {
                tracing::error!("lux_getAllWeights get_neurons error: {}", e);
                jsonrpc_core::Error::internal_error()
            })?;

            let mut all: Vec<Value> = Vec::new();
            for n in &neurons {
                if let Ok(ws) = db.get_weights(subnet_id, n.uid) {
                    all.extend(ws.iter().map(weight_to_json));
                }
            }
            Ok(Value::Array(all))
        }
    });

    // =========================================================================
    // lux_getEmissions — emission info for a subnet
    // Params: [subnet_id: number]
    // Returns: { subnet_id, name, emission_rate, emission_rate_decimal, active }
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getEmissions", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            match db.get_subnet(subnet_id) {
                Ok(Some(s)) => Ok(serde_json::json!({
                    "subnet_id": s.id,
                    "name": s.name,
                    "emission_rate": format!("0x{:x}", s.emission_rate),
                    "emission_rate_decimal": s.emission_rate.to_string(),
                    // emission per mille (per 1000 blocks) expressed as float
                    "active": s.active,
                    "tempo": s.tempo,
                })),
                Ok(None) => Ok(Value::Null),
                Err(e) => {
                    tracing::error!("lux_getEmissions error: {}", e);
                    Err(jsonrpc_core::Error::internal_error())
                }
            }
        }
    });

    // =========================================================================
    // lux_getMetagraph — full metagraph snapshot for a subnet
    // Params: [subnet_id: number]
    // Returns: { subnet, neurons, weight_matrix, neuron_count, weight_count }
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("lux_getMetagraph", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            build_metagraph_snapshot(&db, subnet_id)
        }
    });

    // =========================================================================
    // metagraph_getState — full metagraph state (alias for lux_getMetagraph)
    // Params: [subnet_id: number]
    // Returns: { subnet, neurons, weight_matrix, neuron_count, weight_count }
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("metagraph_getState", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            build_metagraph_snapshot(&db, subnet_id)
        }
    });

    // =========================================================================
    // metagraph_getWeights — all weights in a subnet (alias for lux_getAllWeights)
    // Params: [subnet_id: number]
    // Returns: { from_uid, to_uid, weight, updated_at }[]
    // =========================================================================
    let db = metagraph.clone();
    io.add_method("metagraph_getWeights", move |params: Params| {
        let db = db.clone();
        async move {
            let subnet_id = parse_single_u64(params, "subnet_id")?;
            let neurons = db.get_neurons_by_subnet(subnet_id).map_err(|e| {
                tracing::error!("metagraph_getWeights get_neurons error: {}", e);
                jsonrpc_core::Error::internal_error()
            })?;

            let mut all: Vec<Value> = Vec::new();
            for n in &neurons {
                if let Ok(ws) = db.get_weights(subnet_id, n.uid) {
                    all.extend(ws.iter().map(weight_to_json));
                }
            }
            Ok(Value::Array(all))
        }
    });
}

// =========================================================================
// Shared helpers
// =========================================================================

/// Parse a single u64 from positional params [value]
fn parse_single_u64(params: Params, name: &str) -> Result<u64, jsonrpc_core::Error> {
    let parsed: Vec<serde_json::Value> = params.parse()?;
    if parsed.is_empty() {
        return Err(jsonrpc_core::Error::invalid_params(
            format!("Missing required parameter: {}", name),
        ));
    }
    parsed[0]
        .as_u64()
        .ok_or_else(|| jsonrpc_core::Error::invalid_params(format!("{} must be a number", name)))
}

/// Parse two u64 values from positional params [a, b]
fn parse_two_u64(
    params: Params,
    name_a: &str,
    name_b: &str,
) -> Result<(u64, u64), jsonrpc_core::Error> {
    let parsed: Vec<serde_json::Value> = params.parse()?;
    if parsed.len() < 2 {
        return Err(jsonrpc_core::Error::invalid_params(format!(
            "Missing required parameters: {} and {}",
            name_a, name_b
        )));
    }
    let a = parsed[0]
        .as_u64()
        .ok_or_else(|| jsonrpc_core::Error::invalid_params(format!("{} must be a number", name_a)))?;
    let b = parsed[1]
        .as_u64()
        .ok_or_else(|| jsonrpc_core::Error::invalid_params(format!("{} must be a number", name_b)))?;
    Ok((a, b))
}

/// Build a full metagraph snapshot for a given subnet
fn build_metagraph_snapshot(db: &MetagraphDB, subnet_id: u64) -> Result<Value, jsonrpc_core::Error> {
    let subnet = match db.get_subnet(subnet_id) {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(Value::Null),
        Err(e) => {
            tracing::error!("metagraph snapshot get_subnet error: {}", e);
            return Err(jsonrpc_core::Error::internal_error());
        }
    };

    let neurons = db.get_neurons_by_subnet(subnet_id).map_err(|e| {
        tracing::error!("metagraph snapshot get_neurons error: {}", e);
        jsonrpc_core::Error::internal_error()
    })?;

    // Build weight matrix: { "uid": [ {to_uid, weight, updated_at}, ... ] }
    let mut weight_matrix = serde_json::Map::new();
    let mut total_weights = 0usize;

    for n in &neurons {
        if let Ok(ws) = db.get_weights(subnet_id, n.uid) {
            if !ws.is_empty() {
                let uid_str = n.uid.to_string();
                let w_list: Vec<Value> = ws.iter().map(|w| serde_json::json!({
                    "to_uid": w.to_uid,
                    "weight": w.weight,
                    "weight_normalized": w.weight as f64 / 65535.0,
                    "updated_at": w.updated_at,
                })).collect();
                total_weights += w_list.len();
                weight_matrix.insert(uid_str, Value::Array(w_list));
            }
        }
    }

    let neuron_count = neurons.len();
    let neurons_json: Vec<Value> = neurons.iter().map(neuron_to_json).collect();

    Ok(serde_json::json!({
        "subnet": subnet_to_json(&subnet),
        "neurons": neurons_json,
        "weight_matrix": Value::Object(weight_matrix),
        "neuron_count": neuron_count,
        "weight_count": total_weights,
    }))
}

// =========================================================================
// Serialization helpers: MetagraphDB structs → serde_json::Value
// =========================================================================

fn subnet_to_json(s: &SubnetData) -> Value {
    serde_json::json!({
        "id": s.id,
        "name": s.name,
        "owner": format!("0x{}", hex::encode(s.owner)),
        "emission_rate": format!("0x{:x}", s.emission_rate),
        "emission_rate_decimal": s.emission_rate.to_string(),
        "created_at": s.created_at,
        "tempo": s.tempo,
        "max_neurons": s.max_neurons,
        "min_stake": format!("0x{:x}", s.min_stake),
        "min_stake_decimal": s.min_stake.to_string(),
        "active": s.active,
    })
}

fn neuron_to_json(n: &NeuronData) -> Value {
    serde_json::json!({
        "uid": n.uid,
        "subnet_id": n.subnet_id,
        "hotkey": format!("0x{}", hex::encode(n.hotkey)),
        "coldkey": format!("0x{}", hex::encode(n.coldkey)),
        "stake": format!("0x{:x}", n.stake),
        "stake_decimal": n.stake.to_string(),
        "trust": n.trust,
        "trust_normalized": n.trust as f64 / 65535.0,
        "rank": n.rank,
        "rank_normalized": n.rank as f64 / 65535.0,
        "incentive": n.incentive,
        "incentive_normalized": n.incentive as f64 / 65535.0,
        "dividends": n.dividends,
        "dividends_normalized": n.dividends as f64 / 65535.0,
        "emission": format!("0x{:x}", n.emission),
        "emission_decimal": n.emission.to_string(),
        "last_update": n.last_update,
        "active": n.active,
        "endpoint": n.endpoint,
    })
}

fn weight_to_json(w: &WeightData) -> Value {
    serde_json::json!({
        "from_uid": w.from_uid,
        "to_uid": w.to_uid,
        "weight": w.weight,
        "weight_normalized": w.weight as f64 / 65535.0,
        "updated_at": w.updated_at,
    })
}

/// Add `admin_runEpoch` — manually triggers YumaConsensus for testing.
/// This should only be called from the start of the RPC registration function
/// (after the metagraph_for_epoch clone is made).
pub fn register_admin_epoch_handler(
    io: &mut jsonrpc_core::IoHandler,
    metagraph: Arc<MetagraphDB>,
) {
    let db = metagraph.clone();
    io.add_method("admin_runEpoch", move |params: Params| {
        let db = db.clone();
        async move {
            // Optional epoch_num param; default to 0
            let epoch_num: u64 = params
                .parse::<Vec<serde_json::Value>>()
                .ok()
                .and_then(|v| v.into_iter().next())
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            tracing::info!("🔧 admin_runEpoch: triggering YumaConsensus epoch {}", epoch_num);

            let updates = YumaConsensus::compute(&db, epoch_num);
            let update_count = updates.len();

            tracing::info!("🧠 admin_runEpoch: {} neuron updates computed", update_count);

            // Persist updates to MetagraphDB
            let mut persisted = 0usize;
            for upd in &updates {
                if let Ok(Some(mut neuron)) = db.get_neuron(upd.subnet_id, upd.uid) {
                    neuron.trust = upd.trust;
                    neuron.rank = upd.rank;
                    neuron.incentive = upd.incentive;
                    neuron.dividends = upd.dividends;
                    neuron.emission = upd.emission;
                    neuron.last_update = epoch_num;
                    if let Ok(()) = db.store_neuron(&neuron) {
                        persisted += 1;
                    } else {
                        tracing::warn!("admin_runEpoch: failed to update neuron uid={}", upd.uid);
                    }
                } else {
                    tracing::warn!("admin_runEpoch: neuron uid={} not found in subnet {}", upd.uid, upd.subnet_id);
                }
            }

            tracing::info!("✅ admin_runEpoch: persisted {}/{} updates", persisted, update_count);

            Ok(serde_json::json!({
                "epoch_num": epoch_num,
                "updates_computed": update_count,
                "updates_persisted": persisted,
                "neuron_updates": updates.iter().map(|u| serde_json::json!({
                    "subnet_id": u.subnet_id,
                    "uid": u.uid,
                    "trust": u.trust,
                    "rank": u.rank,
                    "incentive": u.incentive,
                    "dividends": u.dividends,
                })).collect::<Vec<_>>(),
            }))
        }
    });
}

/// Debug endpoint: dump MetagraphDB state (validators, subnets, neurons count)
/// Usage: admin_debugMetagraph → shows all validators with is_active + stake
pub fn register_debug_metagraph_handler(
    io: &mut jsonrpc_core::IoHandler,
    metagraph: Arc<MetagraphDB>,
) {
    let db = metagraph.clone();
    io.add_method("admin_debugMetagraph", move |_params: Params| {
        let db = db.clone();
        async move {
            // Validators
            let validators = db.get_all_validators().unwrap_or_default();
            let val_list: Vec<serde_json::Value> = validators.iter().map(|v| {
                serde_json::json!({
                    "address": format!("0x{}", hex::encode(&v.address)),
                    "stake": v.stake.to_string(),
                    "is_active": v.is_active,
                    "name": v.name,
                })
            }).collect();

            // Subnets
            let subnets = db.get_all_subnets().unwrap_or_default();
            let subnet_list: Vec<serde_json::Value> = subnets.iter().map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "active": s.active,
                })
            }).collect();

            // Stakes (StakingData)
            let stakes = db.get_all_stakes().unwrap_or_default();
            let stake_list: Vec<serde_json::Value> = stakes.iter().map(|s| {
                serde_json::json!({
                    "address": format!("0x{}", hex::encode(&s.address)),
                    "stake": s.stake.to_string(),
                })
            }).collect();

            tracing::info!(
                "admin_debugMetagraph: {} validators, {} subnets, {} stakes",
                val_list.len(), subnet_list.len(), stake_list.len()
            );

            Ok(serde_json::json!({
                "validators": val_list,
                "subnets": subnet_list,
                "staking_data": stake_list,
            }))
        }
    });
}
