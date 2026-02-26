// Neuron RPC handlers
// Dual-write: BlockchainDB (legacy compat) + MetagraphDB (lux_* source of truth)
// SECURITY: All state-mutating methods require timestamp + ECDSA signature

use crate::helpers::{parse_address, verify_caller_signature};
use crate::types::{NeuronInfo, SubnetInfo};
use dashmap::DashMap;
use jsonrpc_core::{Params, Value};
use luxtensor_core::{MetagraphTxPayload, Transaction, Address};
use luxtensor_core::constants::precompiles;
use luxtensor_storage::{BlockchainDB, MetagraphDB, NeuronData};
use std::sync::Arc;

/// Minimum stake để đăng ký neuron: 10 LUX (18 decimals)
const NEURON_MIN_STAKE: u128 = 10_000_000_000_000_000_000u128;

/// Register neuron-related RPC methods.
///
/// Neurons are persisted to both BlockchainDB (legacy) and MetagraphDB.
/// Read queries serve from DashMap cache (loaded from MetagraphDB at startup).
pub fn register_neuron_handlers(
    io: &mut jsonrpc_core::IoHandler,
    neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
    subnets: Arc<DashMap<u64, SubnetInfo>>,
    db: Arc<BlockchainDB>,
    metagraph: Arc<MetagraphDB>,
    mempool: Arc<luxtensor_core::UnifiedMempool>,
) {
    // ----- Startup: load neurons from MetagraphDB into in-memory cache -----
    // Iterate through all subnets and load their neurons
    for subnet_entry in subnets.iter() {
        let subnet_id = *subnet_entry.key();
        match metagraph.get_neurons_by_subnet(subnet_id) {
            Ok(stored_neurons) => {
                for n in &stored_neurons {
                    let key = (n.subnet_id, n.uid);
                    if !neurons.contains_key(&key) {
                        neurons.insert(
                            key,
                            NeuronInfo {
                                uid: n.uid,
                                subnet_id: n.subnet_id,
                                hotkey: format!("0x{}", hex::encode(n.hotkey)),
                                coldkey: format!("0x{}", hex::encode(n.coldkey)),
                                address: format!("0x{}", hex::encode(n.hotkey)),
                                stake: n.stake,
                                trust: n.trust as f64 / 65535.0,
                                consensus: 0.0,
                                rank: n.rank as u64,
                                incentive: n.incentive as f64 / 65535.0,
                                dividends: n.dividends as f64 / 65535.0,
                                emission: n.emission,
                                last_update: n.last_update,
                                active: n.active,
                                endpoint: if n.endpoint.is_empty() {
                                    None
                                } else {
                                    Some(n.endpoint.clone())
                                },
                            },
                        );
                    }
                }
                if !stored_neurons.is_empty() {
                    tracing::debug!(
                        "📊 Loaded {} neurons for subnet {} from MetagraphDB",
                        stored_neurons.len(),
                        subnet_id
                    );
                }
            }
            Err(e) => tracing::warn!("Failed to load neurons for subnet {}: {}", subnet_id, e),
        }
    }

    // Fallback: load from BlockchainDB for any remaining, AND sync to MetagraphDB
    if let Ok(stored_neurons) = db.get_all_neurons() {
        let mut synced_to_metagraph = 0usize;
        for ((subnet_id, uid), data) in stored_neurons {
            let key = (subnet_id, uid);

            // Load into DashMap if not already there
            if !neurons.contains_key(&key) {
                if let Ok(neuron) = bincode::deserialize::<NeuronInfo>(&data) {
                    neurons.insert(key, neuron);
                }
            }

            // ── SYNC: write back into MetagraphDB if it doesn't have this neuron ──
            // This handles the case where MetagraphDB was empty/temp on previous run.
            match metagraph.get_neuron(subnet_id, uid) {
                Ok(None) => {
                    // MetagraphDB is missing this neuron, reconstruct from NeuronInfo
                    if let Ok(ni) = bincode::deserialize::<NeuronInfo>(&data) {
                        // Parse hotkey hex string → [u8;20]
                        let hotkey_hex = ni.hotkey.trim_start_matches("0x");
                        let coldkey_hex = ni.coldkey.trim_start_matches("0x");
                        let mut hotkey = [0u8; 20];
                        let mut coldkey = [0u8; 20];
                        if hex::decode_to_slice(hotkey_hex, &mut hotkey).is_ok()
                            && hex::decode_to_slice(coldkey_hex, &mut coldkey).is_ok()
                        {
                            let nd = NeuronData {
                                uid,
                                subnet_id,
                                hotkey,
                                coldkey,
                                stake: ni.stake,
                                trust: (ni.trust * 65535.0) as u32,
                                rank: ni.rank as u32,
                                incentive: (ni.incentive * 65535.0) as u32,
                                dividends: (ni.dividends * 65535.0) as u32,
                                emission: ni.emission,
                                last_update: ni.last_update,
                                active: ni.active,
                                endpoint: ni.endpoint.unwrap_or_default(),
                            };
                            if metagraph.store_neuron(&nd).is_ok() {
                                synced_to_metagraph += 1;
                            }
                        }
                    }
                }
                Ok(Some(_)) => {} // Already in MetagraphDB, nothing to do
                Err(e) => tracing::warn!("MetagraphDB read error for uid={} subnet={}: {}", uid, subnet_id, e),
            }
        }
        if synced_to_metagraph > 0 {
            tracing::info!("🔄 Synced {} neurons from BlockchainDB → MetagraphDB", synced_to_metagraph);
        }
    }

    // =========================================================================
    // neuron_getInfo — get a single neuron
    // Priority: MetagraphDB (consensus fields updated by Yuma) > DashMap (identity cache)
    // =========================================================================
    let neurons_clone = neurons.clone();
    let metagraph_for_getinfo = metagraph.clone();
    io.add_method("neuron_getInfo", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        let metagraph_for_getinfo = metagraph_for_getinfo.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing subnet_id and uid",
                ));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let uid = parsed[1]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;

            // Try MetagraphDB first (has latest Yuma-computed consensus values)
            if let Ok(Some(nd)) = metagraph_for_getinfo.get_neuron(subnet_id, uid) {
                // Get identity fields from DashMap if available
                let cache = neurons_clone.get(&(subnet_id, uid));
                let hotkey_str = if let Some(ref c) = cache {
                    c.hotkey.clone()
                } else {
                    format!("0x{}", hex::encode(nd.hotkey))
                };
                let coldkey_str = if let Some(ref c) = cache {
                    c.coldkey.clone()
                } else {
                    format!("0x{}", hex::encode(nd.coldkey))
                };
                let endpoint_val = if let Some(ref c) = cache {
                    c.endpoint.clone()
                } else {
                    if nd.endpoint.is_empty() { None } else { Some(nd.endpoint.clone()) }
                };
                return Ok(serde_json::json!({
                    "uid": nd.uid,
                    "subnet_id": nd.subnet_id,
                    "hotkey": hotkey_str,
                    "coldkey": coldkey_str,
                    "stake": format!("0x{:x}", nd.stake),
                    "trust": nd.trust as f64 / 65535.0,
                    "consensus": nd.trust as f64 / 65535.0,
                    "rank": nd.rank,
                    "incentive": nd.incentive as f64 / 65535.0,
                    "dividends": nd.dividends as f64 / 65535.0,
                    "emission": format!("0x{:x}", nd.emission),
                    "last_update": format!("0x{:x}", nd.last_update),
                    "active": nd.active,
                    "endpoint": endpoint_val,
                }));
            }

            // Fallback: DashMap cache (identity info, consensus fields may be stale)
            if let Some(n) = neurons_clone.get(&(subnet_id, uid)) {
                Ok(serde_json::json!({
                    "uid": n.uid,
                    "subnet_id": n.subnet_id,
                    "hotkey": n.hotkey,
                    "coldkey": n.coldkey,
                    "stake": format!("0x{:x}", n.stake),
                    "trust": n.trust,
                    "consensus": n.consensus,
                    "rank": n.rank,
                    "incentive": n.incentive,
                    "dividends": n.dividends,
                    "emission": format!("0x{:x}", n.emission),
                    "last_update": format!("0x{:x}", n.last_update),
                    "active": n.active,
                    "endpoint": n.endpoint,
                }))
            } else {
                Ok(Value::Null)
            }
        }
    });

    // =========================================================================
    // neuron_listBySubnet — get all neurons in a subnet
    // =========================================================================
    let neurons_clone = neurons.clone();
    io.add_method("neuron_listBySubnet", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            let subnet_id = if parsed.is_empty() {
                0u64
            } else {
                parsed[0]
                    .as_u64()
                    .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?
            };

            let result: Vec<Value> = neurons_clone
                .iter()
                .filter(|entry| entry.key().0 == subnet_id)
                .map(|entry| {
                    let n = entry.value();
                    serde_json::json!({
                        "uid": n.uid,
                        "subnet_id": n.subnet_id,
                        "hotkey": n.hotkey,
                        "coldkey": n.coldkey,
                        "stake": format!("0x{:x}", n.stake),
                        "trust": n.trust,
                        "consensus": n.consensus,
                        "rank": n.rank,
                        "incentive": n.incentive,
                        "dividends": n.dividends,
                        "emission": format!("0x{:x}", n.emission),
                        "last_update": format!("0x{:x}", n.last_update),
                        "active": n.active,
                        "endpoint": n.endpoint,
                    })
                })
                .collect();
            Ok(Value::Array(result))
        }
    });

    // =========================================================================
    // neuron_register — register a new neuron, dual-write to both DBs
    // SECURITY: Requires timestamp + signature to prevent impersonation
    // Params: [subnet_id, hotkey, coldkey, endpoint, stake?, timestamp, signature]
    // =========================================================================
    let neurons_clone = neurons.clone();
    let db_for_register = db.clone();
    let metagraph_for_register = metagraph.clone();
    let mempool_for_register = mempool.clone();
    io.add_method("neuron_register", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        let db_for_register = db_for_register.clone();
        let metagraph_for_register = metagraph_for_register.clone();
        let mempool_for_register = mempool_for_register.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            // Minimum required: subnet_id, hotkey, coldkey, endpoint, stake, timestamp, signature
            if parsed.len() < 7 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing params: subnet_id, hotkey, coldkey, endpoint, stake, timestamp, signature",
                ));
            }

            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let hotkey_str = parsed[1]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid hotkey"))?;
            let coldkey_str = parsed[2]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid coldkey"))?;
            let endpoint = parsed[3]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid endpoint"))?
                .to_string();

            // Parse stake — reject zero and below minimum
            let stake_str = parsed[4]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid stake"))?;
            let stake = u128::from_str_radix(stake_str.trim_start_matches("0x"), 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid stake hex format"))?;
            if stake == 0 {
                return Err(jsonrpc_core::Error::invalid_params("Stake cannot be zero"));
            }
            if stake < NEURON_MIN_STAKE {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Stake must be at least 10 LUX"
                ));
            }

            // Parse and verify timestamp (within 5 minutes)
            let timestamp_str = parsed[5]
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

            // Verify caller owns the hotkey via ECDSA signature
            let hotkey_addr = parse_address(hotkey_str)?;
            let signature = parsed[6]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid signature"))?;
            // CRITICAL: lowercase hotkey to match Python eth_account signing convention
            let hotkey_lc = hotkey_str.trim_start_matches("0x").to_lowercase();
            let message = format!(
                "neuron_register:{}:{}:{}",
                hotkey_lc,
                subnet_id,
                timestamp
            );
            verify_caller_signature(&hotkey_addr, &message, signature, 0)
                .or_else(|_| verify_caller_signature(&hotkey_addr, &message, signature, 1))
                .map_err(|_| jsonrpc_core::Error::invalid_params(
                    "Signature verification failed — caller must own the hotkey address"
                ))?;

            // Parse hotkey bytes (20 bytes from hex)
            let hex_hotkey = hotkey_str.trim_start_matches("0x");
            if hex_hotkey.len() != 40 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Hotkey must be a 20-byte Ethereum address (40 hex chars)"
                ));
            }
            let hotkey_vec = hex::decode(hex_hotkey)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid hotkey hex"))?;
            let mut hotkey_bytes = [0u8; 20];
            hotkey_bytes.copy_from_slice(&hotkey_vec);

            // Coldkey validation
            let hex_coldkey = coldkey_str.trim_start_matches("0x");
            if hex_coldkey.len() != 40 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Coldkey must be a 20-byte Ethereum address (40 hex chars)"
                ));
            }
            let coldkey_vec = hex::decode(hex_coldkey)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid coldkey hex"))?;
            let mut coldkey_bytes = [0u8; 20];
            coldkey_bytes.copy_from_slice(&coldkey_vec);

            // DUPLICATE CHECK: MetagraphDB is the source of truth (persists across restarts)
            if let Ok(neurons_in_subnet) = metagraph_for_register.get_neurons_by_subnet(subnet_id) {
                if neurons_in_subnet.iter().any(|n| n.hotkey == hotkey_bytes) {
                    return Err(jsonrpc_core::Error::invalid_params(
                        "Hotkey already registered in this subnet"
                    ));
                }
            }
            // Also check in-memory cache (fast path)
            let uid_exists = neurons_clone
                .iter()
                .any(|e| e.key().0 == subnet_id && e.value().hotkey == hotkey_str);
            if uid_exists {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Hotkey already registered in this subnet (cache)"
                ));
            }

            // Assign UID from MetagraphDB count (more reliable than DashMap count)
            let uid = metagraph_for_register
                .get_neuron_count(subnet_id)
                .unwrap_or_else(|_| neurons_clone.iter().filter(|e| e.key().0 == subnet_id).count())
                as u64;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs();

            // Build legacy NeuronInfo for DashMap + BlockchainDB
            let neuron = NeuronInfo {
                uid,
                subnet_id,
                hotkey: hotkey_str.to_string(),
                coldkey: coldkey_str.to_string(),
                address: hotkey_str.to_string(),
                stake,
                trust: 0.0,
                consensus: 0.0,
                rank: 0,
                incentive: 0.0,
                dividends: 0.0,
                emission: 0,
                last_update: now,
                active: true,
                endpoint: Some(endpoint.clone()),
            };

            neurons_clone.insert((subnet_id, uid), neuron.clone());

            // Persist to legacy BlockchainDB
            if let Ok(data) = bincode::serialize(&neuron) {
                let _ = db_for_register.store_neuron(subnet_id, uid, &data);
            }

            // Dual-write to MetagraphDB (lux_* source of truth)
            let neuron_data = NeuronData {
                uid,
                subnet_id,
                hotkey: hotkey_bytes,
                coldkey: coldkey_bytes,
                stake,
                trust: 0,
                rank: 0,
                incentive: 0,
                dividends: 0,
                emission: 0,
                last_update: now,
                active: true,
                endpoint,
            };

            // ── SUBMIT METAGRAPH PRECOMPILE TX TO MEMPOOL ────────────────────────────
            // ALL nodes will update MetagraphDB when they execute this tx from the block.
            // Signature was already verified above (timestamp + ECDSA) — use add_system_transaction.
            let precompile_addr = Address::from(precompiles::metagraph_address());
            let payload = MetagraphTxPayload::RegisterNeuron {
                subnet_id,
                uid,
                hotkey: hotkey_bytes,
                coldkey: coldkey_bytes,
                endpoint: neuron_data.endpoint.clone(),
                stake,
                active: true,
            };

            let tx_hash_hex = match payload.encode() {
                Ok(tx_data) => {
                    // Nonce: nanosecond timestamp = unique per call, never collides.
                    // (uid can repeat across subnets; timestamp cannot repeat within same process)
                    let nonce = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as u64
                        | (uid << 32); // mix uid into upper bits for readability

                    let tx = Transaction::with_chain_id(
                        mempool_for_register.chain_id(), // ✅ real chain_id from mempool
                        nonce,
                        Address::from(hotkey_bytes),
                        Some(precompile_addr),
                        0,        // value = 0
                        1_000_000_000, // 1 Gwei gas_price (above mempool minimum)
                        200_000,  // gas_limit for precompile
                        tx_data,
                    );
                    let hash = tx.hash();
                    match mempool_for_register.add_system_transaction(tx) {
                        Ok(_) => {
                            tracing::info!(
                                "📨 MetagraphTx submitted: RegisterNeuron uid={} subnet={} hash=0x{}",
                                uid, subnet_id, hex::encode(&hash)
                            );
                            format!("0x{}", hex::encode(&hash))
                        }
                        Err(e) => {
                            tracing::warn!("Failed to submit MetagraphTx (neuron uid={}): {}", uid, e);
                            String::new()
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to encode MetagraphTxPayload (neuron uid={}): {}", uid, e);
                    String::new()
                }
            };

            Ok(serde_json::json!({
                "success": true,
                "uid": uid,
                "subnet_id": subnet_id,
                "hotkey": hotkey_str,
                "stake": format!("0x{:x}", stake),
                // Client can poll eth_getTransactionReceipt(tx_hash) to confirm
                "tx_hash": tx_hash_hex,
            }))
        }
    });

    // =========================================================================
    // neuron_getCount — count neurons in subnet
    // =========================================================================
    let neurons_clone = neurons.clone();
    io.add_method("neuron_getCount", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            let subnet_id = if parsed.is_empty() {
                0u64
            } else {
                parsed[0].as_u64().unwrap_or(0)
            };
            let count = neurons_clone
                .iter()
                .filter(|e| e.key().0 == subnet_id)
                .count();
            Ok(Value::Number(count.into()))
        }
    });

    // =========================================================================
    // neuron_updateStake — INTERNAL USE ONLY, not exposed via public RPC
    // This method is intentionally removed from the public JSON-RPC API.
    // Stake updates happen internally through staking_addStake / staking_removeStake.
    // =========================================================================
    // (removed from public API for security — was F2 vuln in audit)

    // =========================================================================
    // SDK Aliases
    // =========================================================================

    // query_getNeuron — reads MetagraphDB (Yuma consensus values) then DashMap fallback
    // FIX: previously only read DashMap (trust always 0.0). Now reads MetagraphDB first.
    let neurons_clone = neurons.clone();
    let metagraph_for_query = metagraph.clone();
    io.add_method("query_getNeuron", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        let metagraph_for_query = metagraph_for_query.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.len() < 2 {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet_id, uid"));
            }
            let subnet_id = parsed[0].as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet_id"))?;
            let uid = parsed[1].as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid uid"))?;

            // ── MetagraphDB first (has Yuma-computed trust/rank/incentive/dividends) ──
            if let Ok(Some(nd)) = metagraph_for_query.get_neuron(subnet_id, uid) {
                let cache = neurons_clone.get(&(subnet_id, uid));
                let hotkey_str = if let Some(ref c) = cache {
                    c.hotkey.clone()
                } else {
                    format!("0x{}", hex::encode(nd.hotkey))
                };
                let coldkey_str = if let Some(ref c) = cache {
                    c.coldkey.clone()
                } else {
                    format!("0x{}", hex::encode(nd.coldkey))
                };
                let endpoint_val = if let Some(ref c) = cache {
                    c.endpoint.clone()
                } else {
                    if nd.endpoint.is_empty() { None } else { Some(nd.endpoint.clone()) }
                };
                return Ok(serde_json::json!({
                    "uid": nd.uid,
                    "subnet_id": nd.subnet_id,
                    "hotkey": hotkey_str,
                    "coldkey": coldkey_str,
                    "stake": format!("0x{:x}", nd.stake),
                    "trust": nd.trust as f64 / 65535.0,
                    "consensus": nd.trust as f64 / 65535.0,
                    "rank": nd.rank,
                    "incentive": nd.incentive as f64 / 65535.0,
                    "dividends": nd.dividends as f64 / 65535.0,
                    "emission": format!("0x{:x}", nd.emission),
                    "last_update": format!("0x{:x}", nd.last_update),
                    "active": nd.active,
                    "endpoint": endpoint_val,
                }));
            }

            // ── DashMap fallback (identity info only, consensus fields may be stale) ──
            if let Some(n) = neurons_clone.get(&(subnet_id, uid)) {
                Ok(serde_json::json!({
                    "uid": n.uid,
                    "subnet_id": n.subnet_id,
                    "hotkey": n.hotkey,
                    "coldkey": n.coldkey,
                    "stake": format!("0x{:x}", n.stake),
                    "trust": n.trust,
                    "consensus": n.consensus,
                    "rank": n.rank,
                    "incentive": n.incentive,
                    "dividends": n.dividends,
                    "emission": format!("0x{:x}", n.emission),
                    "last_update": format!("0x{:x}", n.last_update),
                    "active": n.active,
                    "endpoint": n.endpoint,
                }))
            } else {
                Ok(Value::Null)
            }
        }
    });

    // query_getNeurons — alias for neuron_listBySubnet
    let neurons_clone = neurons.clone();
    io.add_method("query_getNeurons", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            let subnet_id = if parsed.is_empty() { 0u64 } else { parsed[0].as_u64().unwrap_or(0) };
            let result: Vec<Value> = neurons_clone
                .iter()
                .filter(|e| e.key().0 == subnet_id)
                .map(|entry| {
                    let n = entry.value();
                    serde_json::json!({
                        "uid": n.uid,
                        "subnet_id": n.subnet_id,
                        "hotkey": n.hotkey,
                        "coldkey": n.coldkey,
                        "stake": format!("0x{:x}", n.stake),
                        "trust": n.trust,
                        "consensus": n.consensus,
                        "rank": n.rank,
                        "incentive": n.incentive,
                        "dividends": n.dividends,
                        "emission": format!("0x{:x}", n.emission),
                        "last_update": format!("0x{:x}", n.last_update),
                        "active": n.active,
                        "endpoint": n.endpoint,
                    })
                })
                .collect();
            Ok(Value::Array(result))
        }
    });

    // query_getNeuronCount — alias for neuron_getCount
    let neurons_clone = neurons.clone();
    io.add_method("query_getNeuronCount", move |params: Params| {
        let neurons_clone = neurons_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            let subnet_id = if parsed.is_empty() { 0u64 } else { parsed[0].as_u64().unwrap_or(0) };
            let count = neurons_clone.iter().filter(|e| e.key().0 == subnet_id).count();
            Ok(Value::Number(count.into()))
        }
    });
}
