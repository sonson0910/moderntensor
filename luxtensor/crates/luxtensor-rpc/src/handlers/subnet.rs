// Subnet RPC handlers
// Extracted from server.rs
// SECURITY: subnet_create requires timestamp + ECDSA signature
// Now with dual-write: BlockchainDB (legacy) + MetagraphDB (lux_* namespace)

use crate::helpers::{parse_address, verify_caller_signature};
use crate::types::SubnetInfo;
use dashmap::DashMap;
use jsonrpc_core::{Params, Value};
use luxtensor_core::{MetagraphTxPayload, Transaction, Address};
use luxtensor_core::constants::precompiles;
use luxtensor_storage::{BlockchainDB, MetagraphDB, SubnetData};
use std::sync::Arc;

/// Default minimum stake required to join a new subnet: 100 LUX (18 decimals)
const SUBNET_DEFAULT_MIN_STAKE: u128 = 100_000_000_000_000_000_000u128;

/// Register subnet-related RPC methods.
///
/// Subnets are persisted to both BlockchainDB (legacy compat) and MetagraphDB
/// (accessed via lux_* methods registered in metagraph.rs).
pub fn register_subnet_handlers(
    io: &mut jsonrpc_core::IoHandler,
    subnets: Arc<DashMap<u64, SubnetInfo>>,
    db: Arc<BlockchainDB>,
    metagraph: Arc<MetagraphDB>,
    mempool: Arc<luxtensor_core::UnifiedMempool>,
) {
    // Load existing subnets from MetagraphDB (source of truth) into in-memory cache
    match metagraph.get_all_subnets() {
        Ok(stored) => {
            for s in &stored {
                if !subnets.contains_key(&s.id) {
                    subnets.insert(
                        s.id,
                        SubnetInfo {
                            id: s.id,
                            name: s.name.clone(),
                            owner: format!("0x{}", hex::encode(s.owner)),
                            emission_rate: s.emission_rate,
                            participant_count: 0,
                            total_stake: 0,
                            created_at: s.created_at,
                        },
                    );
                }
            }
            if !stored.is_empty() {
                tracing::info!("📊 Loaded {} subnets from MetagraphDB", stored.len());
            }
        }
        Err(e) => tracing::warn!("Failed to load subnets from MetagraphDB: {}", e),
    }

    // Fallback: also load from BlockchainDB (for any subnets not yet migrated)
    // AND sync to MetagraphDB — Yuma reads subnets from MetagraphDB to iterate compute_subnet.
    // Without subnets in MetagraphDB, Yuma produces 0 updates even with validators present.
    if let Ok(stored_subnets) = db.get_all_subnets() {
        let mut synced_to_metagraph = 0usize;
        for (id, data) in stored_subnets {
            // Load into DashMap cache
            if !subnets.contains_key(&id) {
                if let Ok(subnet) = bincode::deserialize::<SubnetInfo>(&data) {
                    subnets.insert(id, subnet);
                }
            }
            // ── SYNC: write to MetagraphDB if missing ──
            // Yuma's compute() iterates get_all_subnets() from MetagraphDB.
            match metagraph.get_subnet(id) {
                Ok(None) => {
                    if let Ok(si) = bincode::deserialize::<SubnetInfo>(&data) {
                        let owner_hex = si.owner.trim_start_matches("0x");
                        let mut owner = [0u8; 20];
                        if hex::decode_to_slice(owner_hex, &mut owner).is_ok() {
                            let sd = SubnetData {
                                id,
                                name: si.name.clone(),
                                owner,
                                emission_rate: si.emission_rate,
                                created_at: si.created_at,
                                tempo: 100,
                                max_neurons: 256,
                                min_stake: SUBNET_DEFAULT_MIN_STAKE,
                                active: true,
                            };
                            if metagraph.store_subnet(&sd).is_ok() {
                                synced_to_metagraph += 1;
                            }
                        }
                    }
                }
                Ok(Some(_)) => {} // Already in MetagraphDB
                Err(e) => tracing::warn!("MetagraphDB subnet check error for id={}: {}", id, e),
            }
        }
        if synced_to_metagraph > 0 {
            tracing::info!("🔄 Synced {} subnets from BlockchainDB → MetagraphDB", synced_to_metagraph);
        }
    }


    // =========================================================================
    // subnet_getInfo — get subnet by ID (legacy method, reads from DashMap cache)
    // =========================================================================
    let subnets_clone = subnets.clone();
    io.add_method("subnet_getInfo", move |params: Params| {
        let subnets_clone = subnets_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;
            if let Some(subnet) = subnets_clone.get(&subnet_id) {
                Ok(serde_json::json!({
                    "id": subnet.id,
                    "name": subnet.name,
                    "owner": subnet.owner,
                    "emission_rate": format!("0x{:x}", subnet.emission_rate),
                    "participant_count": subnet.participant_count,
                    "total_stake": format!("0x{:x}", subnet.total_stake),
                    "created_at": format!("0x{:x}", subnet.created_at),
                }))
            } else {
                Ok(Value::Null)
            }
        }
    });

    // =========================================================================
    // subnet_listAll — list all subnets from DashMap cache
    // =========================================================================
    let subnets_clone = subnets.clone();
    io.add_method("subnet_listAll", move |_params: Params| {
        let subnets_clone = subnets_clone.clone();
        async move {
            let subnets_list: Vec<Value> = subnets_clone
                .iter()
                .map(|entry| {
                    let subnet = entry.value();
                    serde_json::json!({
                        "id": subnet.id,
                        "name": subnet.name,
                        "owner": subnet.owner,
                        "emission_rate": format!("0x{:x}", subnet.emission_rate),
                        "participant_count": subnet.participant_count,
                        "total_stake": format!("0x{:x}", subnet.total_stake),
                    })
                })
                .collect();
            Ok(Value::Array(subnets_list))
        }
    });

    // =========================================================================
    // subnet_create — create a new subnet, dual-write to BlockchainDB+MetagraphDB
    // SECURITY: Requires timestamp + ECDSA signature from the owner address
    // Params: [name, owner_address, emission_rate, timestamp, signature]
    // =========================================================================
    let subnets_clone = subnets.clone();
    let db_for_create = db.clone();
    let metagraph_for_create = metagraph.clone();
    let mempool_for_create = mempool.clone();
    io.add_method("subnet_create", move |params: Params| {
        let subnets_clone = subnets_clone.clone();
        let db_for_create = db_for_create.clone();
        let metagraph_for_create = metagraph_for_create.clone();
        let mempool_for_create = mempool_for_create.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            // Requires: name, owner, emission_rate, timestamp, signature
            if parsed.len() < 5 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Missing params: name, owner, emission_rate, timestamp, signature",
                ));
            }

            let name = parsed[0]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid name"))?
                .to_string();
            if name.trim().is_empty() || name.len() > 64 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Subnet name must be 1-64 characters"
                ));
            }

            let owner_str = parsed[1]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid owner"))?;

            let emission_rate_str = parsed[2]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid emission rate"))?;
            let emission_rate = u128::from_str_radix(emission_rate_str.trim_start_matches("0x"), 16)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid emission rate format"))?;

            // Parse and verify timestamp (replay-attack protection)
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

            // Verify owner signature (caller must own the owner address private key)
            let owner_addr = parse_address(owner_str)?;
            let signature = parsed[4]
                .as_str()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid signature"))?;
            // CRITICAL: lowercase owner to match Python eth_account signing convention
            let owner_lc = owner_str.trim_start_matches("0x").to_lowercase();
            let message = format!(
                "subnet_create:{}:{}:{}",
                name,
                owner_lc,
                timestamp
            );
            verify_caller_signature(&owner_addr, &message, signature, 0)
                .or_else(|_| verify_caller_signature(&owner_addr, &message, signature, 1))
                .map_err(|_| jsonrpc_core::Error::invalid_params(
                    "Signature verification failed — caller must own owner address"
                ))?;

            // Parse owner bytes
            let hex_owner = owner_str.trim_start_matches("0x");
            if hex_owner.len() != 40 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Owner must be 20-byte Ethereum address (40 hex chars)"
                ));
            }
            let owner_vec = hex::decode(hex_owner)
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid owner hex"))?;
            let mut owner_bytes = [0u8; 20];
            owner_bytes.copy_from_slice(&owner_vec);

            // Assign subnet_id from MetagraphDB (atomic, avoids race condition with DashMap.len())
            let existing_subnets = metagraph_for_create.get_all_subnets()
                .unwrap_or_default();
            let subnet_id = existing_subnets.iter()
                .map(|s| s.id + 1)
                .max()
                .unwrap_or(0);
            // Also ensure no collision with in-memory cache
            let final_subnet_id = if subnets_clone.contains_key(&subnet_id) {
                subnets_clone.len() as u64
            } else {
                subnet_id
            };

            // Check duplicate subnet name in MetagraphDB
            if existing_subnets.iter().any(|s| s.name == name) {
                return Err(jsonrpc_core::Error::invalid_params(
                    "A subnet with this name already exists"
                ));
            }

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs();

            // Build legacy SubnetInfo for DashMap + BlockchainDB
            let subnet = SubnetInfo {
                id: final_subnet_id,
                name: name.clone(),
                owner: owner_str.to_string(),
                emission_rate,
                participant_count: 0,
                total_stake: 0,
                created_at: now,
            };

            subnets_clone.insert(final_subnet_id, subnet.clone());

            // Persist to legacy BlockchainDB
            if let Ok(data) = bincode::serialize(&subnet) {
                let _ = db_for_create.store_subnet(final_subnet_id, &data);
            }

            // ── SUBMIT METAGRAPH PRECOMPILE TX TO MEMPOOL ──────────────────────
            // Signature verified above (timestamp + ECDSA) — use add_system_transaction.
            // DashMap + BlockchainDB updated above for legacy reads.
            // MetagraphDB will be updated on ALL nodes when executor processes this tx.
            let precompile_addr = Address::from(precompiles::metagraph_address());
            let payload = MetagraphTxPayload::CreateSubnet {
                subnet_id: final_subnet_id,
                name: name.clone(),
                owner: owner_bytes,
                min_stake: SUBNET_DEFAULT_MIN_STAKE,
            };

            let tx_hash_hex = match payload.encode() {
                Ok(tx_data) => {
                    let nonce = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as u64
                        | (final_subnet_id << 32);

                    let tx = Transaction::with_chain_id(
                        mempool_for_create.chain_id(), // ✅ real chain_id
                        nonce,
                        Address::from(owner_bytes),
                        Some(precompile_addr),
                        0, 1_000_000_000, 200_000,
                        tx_data,
                    );
                    let hash = tx.hash();
                    match mempool_for_create.add_system_transaction(tx) {
                        Ok(_) => {
                            tracing::info!(
                                "📨 MetagraphTx submitted: CreateSubnet id={} '{}' hash=0x{}",
                                final_subnet_id, name, hex::encode(&hash)
                            );
                            format!("0x{}", hex::encode(&hash))
                        }
                        Err(e) => {
                            tracing::warn!("Failed to submit MetagraphTx (subnet id={}): {}", final_subnet_id, e);
                            String::new()
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to encode MetagraphTxPayload (subnet): {}", e);
                    String::new()
                }
            };

            Ok(serde_json::json!({
                "success": true,
                "subnet_id": final_subnet_id,
                "name": name,
                "owner": owner_str,
                "min_stake": format!("0x{:x}", SUBNET_DEFAULT_MIN_STAKE),
                "tx_hash": tx_hash_hex,
            }))
        }
    });

    // =========================================================================
    // subnet_getCount — total subnet count from DashMap cache
    // =========================================================================
    let subnets_clone = subnets.clone();
    io.add_method("subnet_getCount", move |_params: Params| {
        let subnets_clone = subnets_clone.clone();
        async move { Ok(Value::Number(subnets_clone.len().into())) }
    });

    // =========================================================================
    // SDK Aliases (legacy compat)
    // =========================================================================

    // query_getSubnets — alias for subnet_listAll
    let subnets_clone = subnets.clone();
    io.add_method("query_getSubnets", move |_params: Params| {
        let subnets_clone = subnets_clone.clone();
        async move {
            let subnets_list: Vec<Value> = subnets_clone
                .iter()
                .map(|entry| {
                    let subnet = entry.value();
                    serde_json::json!({
                        "id": subnet.id,
                        "name": subnet.name,
                        "owner": subnet.owner,
                        "emission_rate": format!("0x{:x}", subnet.emission_rate),
                        "participant_count": subnet.participant_count,
                        "total_stake": format!("0x{:x}", subnet.total_stake),
                    })
                })
                .collect();
            Ok(Value::Array(subnets_list))
        }
    });

    // query_getSubnetInfo — alias for subnet_getInfo
    let subnets_clone = subnets.clone();
    io.add_method("query_getSubnetInfo", move |params: Params| {
        let subnets_clone = subnets_clone.clone();
        async move {
            let parsed: Vec<serde_json::Value> = params.parse()?;
            if parsed.is_empty() {
                return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
            }
            let subnet_id = parsed[0]
                .as_u64()
                .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;
            if let Some(subnet) = subnets_clone.get(&subnet_id) {
                Ok(serde_json::json!({
                    "id": subnet.id,
                    "name": subnet.name,
                    "owner": subnet.owner,
                    "emission_rate": format!("0x{:x}", subnet.emission_rate),
                    "participant_count": subnet.participant_count,
                    "total_stake": format!("0x{:x}", subnet.total_stake),
                }))
            } else {
                Ok(Value::Null)
            }
        }
    });
}
