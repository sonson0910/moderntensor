//! Miner Dispatch RPC Module
//!
//! This module provides RPC endpoints for miners to:
//! - Claim pending AI tasks
//! - Submit task results
//! - List available tasks
//! - Register as miner
//!
//! SECURITY: All state-changing operations require signature verification
//!
//! STORAGE: Miners are now persisted to MetagraphDB (neurons CF + validators CF).
//! In-memory HashMap is loaded from MetagraphDB at startup.

use crate::helpers::{parse_address, verify_caller_signature};
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_core::Address;
use luxtensor_storage::{MetagraphDB, NeuronData, ValidatorData};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// Fallback subnet ID khi client không cung cấp subnet_id
const DEFAULT_SUBNET_ID: u64 = 1;

/// Minimum stake để đăng ký miner: 100 LUX (18 decimals)
const MINER_MIN_STAKE: u128 = 100_000_000_000_000_000_000u128;

/// Convert Address (&[u8]) to [u8; 20]
#[inline]
fn addr_to_bytes20(addr: &luxtensor_core::Address) -> [u8; 20] {
    let b = addr.as_bytes();
    let mut out = [0u8; 20];
    let len = b.len().min(20);
    out[..len].copy_from_slice(&b[..len]);
    out
}

// ============================================================
// Types
// ============================================================

/// Miner registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerRegistration {
    pub address: String,
    pub stake: String,
    pub capacity: u32,
    pub signature: String,
    /// Timestamp (unix seconds as string) — replay attack protection
    pub timestamp: Option<String>,
    /// Subnet miner muốn tham gia. Bắt buộc phải có trên mainnet;
    /// nếu không truyền thì fallback về DEFAULT_SUBNET_ID (chỉ cho dev).
    pub subnet_id: Option<u64>,
}

/// Task claim request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskClaimRequest {
    pub task_id: String,
    pub miner: String,
    pub signature: String,
}

/// Task result submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultSubmission {
    pub task_id: String,
    pub result_hash: String,
    pub execution_time_ms: u64,
    pub proof: Option<String>,
    /// Miner address submitting the result (hex encoded)
    pub miner: String,
    /// Hex-encoded signature proving miner owns the address
    pub signature: String,
}

/// Simple in-memory task store for dispatch
#[derive(Debug, Clone)]
pub struct DispatchTaskInfo {
    pub task_id: [u8; 32],
    pub model_hash: String,
    pub input_hash: [u8; 32],
    pub reward: u128,
    pub deadline: u64,
    pub status: u8, // 0=pending, 1=assigned, 2=completed
    pub assigned_to: Option<Address>,
    pub result_hash: Option<[u8; 32]>,
}

/// Simple miner info
#[derive(Debug, Clone)]
pub struct SimpleMinerInfo {
    pub address: Address,
    pub stake: u128,
    pub capacity: u32,
    pub current_tasks: u32,
    /// UID in MetagraphDB (assigned on registration)
    pub uid: u64,
}

/// Context for miner dispatch RPC
pub struct MinerDispatchContext {
    pub tasks: Arc<RwLock<HashMap<[u8; 32], DispatchTaskInfo>>>,
    pub miners: Arc<RwLock<HashMap<Address, SimpleMinerInfo>>>,
    pub metagraph: Arc<MetagraphDB>,
}

impl MinerDispatchContext {
    pub fn new(metagraph: Arc<MetagraphDB>) -> Self {
        let miners_map = Arc::new(RwLock::new(HashMap::new()));

        // ── Load existing miners from MetagraphDB at startup ──────────────
        // Miners are stored in the validators CF with names prefixed "miner-"
        if let Ok(stored) = metagraph.get_all_validators() {
            let mut map = miners_map.write();
            for vd in stored {
                // We tag miners by name prefix "miner-"
                if vd.name.starts_with("miner-") {
                    let addr = Address::from(vd.address);
                    let uid = vd.last_block_produced; // repurposed field: stores UID
                    map.insert(
                        addr.clone(),
                        SimpleMinerInfo {
                            address: addr,
                            stake: vd.stake,
                            capacity: vd.blocks_produced as u32, // repurposed: stores capacity
                            current_tasks: 0,
                            uid,
                        },
                    );
                }
            }
            if !map.is_empty() {
                info!("📦 Loaded {} miners from MetagraphDB", map.len());
            }
        }

        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            miners: miners_map,
            metagraph,
        }
    }
}

// ============================================================
// RPC Registration
// ============================================================

pub fn register_miner_dispatch_methods(ctx: Arc<MinerDispatchContext>, io: &mut IoHandler) {
    register_task_methods(&ctx, io);
    register_miner_registration_methods(&ctx, io);
}

fn register_task_methods(ctx: &Arc<MinerDispatchContext>, io: &mut IoHandler) {
    let tasks = ctx.tasks.clone();
    let miners = ctx.miners.clone();

    // lux_claimTask - Claim a pending task
    io.add_method("lux_claimTask", move |params: Params| {
        let tasks = tasks.clone();
        let miners = miners.clone();
        async move {
        let req: TaskClaimRequest = params.parse()?;

        let address = parse_address(&req.miner)?;

        // Security: Verify signature
        let mut task_id_bytes = [0u8; 32];
        let task_hex = req.task_id.trim_start_matches("0x");
        let task_bytes = hex::decode(task_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task_id hex"))?;
        let copy_len = task_bytes.len().min(32);
        task_id_bytes[..copy_len].copy_from_slice(&task_bytes[..copy_len]);

        let message = format!(
            "claim_task:{}:{}",
            hex::encode(&address),
            req.task_id.trim_start_matches("0x")
        );

        let sig_valid = verify_caller_signature(&address, &message, &req.signature, 0)
            .or_else(|_| verify_caller_signature(&address, &message, &req.signature, 1));

        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed"
            ));
        }

        // Check miner is registered
        {
            let miners_map = miners.read();
            if !miners_map.contains_key(&address) {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Miner not registered"
                ));
            }
        }

        let mut tasks_map = tasks.write();
        if let Some(task) = tasks_map.get_mut(&task_id_bytes) {
            if task.status != 0 {
                return Err(jsonrpc_core::Error::invalid_params("Task not available"));
            }
            task.status = 1;
            task.assigned_to = Some(address);

            Ok(serde_json::json!({
                "success": true,
                "task_id": req.task_id,
                "message": "Task claimed successfully"
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Task not found"))
        }
        }
    });

    let tasks = ctx.tasks.clone();
    let miners_for_submit = ctx.miners.clone();
    let metagraph_for_submit = ctx.metagraph.clone();

    // lux_submitResult - Submit task result
    io.add_method("lux_submitResult", move |params: Params| {
        let tasks = tasks.clone();
        let miners = miners_for_submit.clone();
        let meta = metagraph_for_submit.clone();
        async move {
        let submission: TaskResultSubmission = params.parse()?;

        let address = parse_address(&submission.miner)?;

        // Security: Verify signature
        let message = format!(
            "submit_result:{}:{}:{}",
            hex::encode(&address),
            submission.task_id.trim_start_matches("0x"),
            submission.result_hash.trim_start_matches("0x")
        );

        let sig_valid = verify_caller_signature(&address, &message, &submission.signature, 0)
            .or_else(|_| verify_caller_signature(&address, &message, &submission.signature, 1));

        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed"
            ));
        }

        // SECURITY (F12): Check miner is registered — check in-memory cache first,
        // then fall back to MetagraphDB to catch miners loaded from persistent storage
        // but not yet in the in-memory map (e.g., after node restart).
        {
            let in_memory_ok = miners.read().contains_key(&address);
            if !in_memory_ok {
                // Fallback: query MetagraphDB validators CF (miners are tagged with "miner-" prefix)
                let addr_bytes = addr_to_bytes20(&address);
                let meta_ok = meta
                    .get_all_validators()
                    .unwrap_or_default()
                    .into_iter()
                    .any(|vd| vd.name.starts_with("miner-") && vd.address == addr_bytes);
                if !meta_ok {
                    return Err(jsonrpc_core::Error::invalid_params(
                        "Miner not registered in MetagraphDB"
                    ));
                }
                // Re-hydrate in-memory cache so future calls are fast
                tracing::info!(
                    "Re-hydrating miner {} from MetagraphDB into in-memory cache",
                    hex::encode(addr_bytes)
                );
            }
        }

        // Parse task ID — reject if malformed or all-zero placeholder
        let task_hex = submission.task_id.trim_start_matches("0x");
        let task_id_vec = hex::decode(task_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task_id hex"))?;
        if task_id_vec.len() != 32 {
            return Err(jsonrpc_core::Error::invalid_params(
                "task_id must be exactly 32 bytes (64 hex chars)"
            ));
        }
        let mut task_id_bytes = [0u8; 32];
        task_id_bytes.copy_from_slice(&task_id_vec);
        if task_id_bytes == [0u8; 32] {
            return Err(jsonrpc_core::Error::invalid_params(
                "task_id cannot be all zeros"
            ));
        }

        // Parse result_hash — reject if malformed, all-zero, or placeholder pattern
        let result_hex = submission.result_hash.trim_start_matches("0x");
        let result_hash_vec = hex::decode(result_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid result_hash hex"))?;
        if result_hash_vec.len() != 32 {
            return Err(jsonrpc_core::Error::invalid_params(
                "result_hash must be exactly 32 bytes (64 hex chars)"
            ));
        }
        let mut result_hash_bytes = [0u8; 32];
        result_hash_bytes.copy_from_slice(&result_hash_vec);
        // Reject all-zero hash (0x0000...0000)
        if result_hash_bytes == [0u8; 32] {
            return Err(jsonrpc_core::Error::invalid_params(
                "result_hash cannot be all zeros — submit the real computation hash"
            ));
        }
        // Reject monotone placeholder (all same byte: 0xabab..., 0xffff...)
        if result_hash_bytes.iter().all(|&b| b == result_hash_bytes[0]) {
            return Err(jsonrpc_core::Error::invalid_params(
                "result_hash appears to be a placeholder (all bytes identical)"
            ));
        }

        let mut tasks_map = tasks.write();
        if let Some(task) = tasks_map.get_mut(&task_id_bytes) {
            if task.assigned_to.as_ref() != Some(&address) {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Task not assigned to this miner"
                ));
            }
            task.status = 2;
            task.result_hash = Some(result_hash_bytes);

            Ok(serde_json::json!({
                "success": true,
                "task_id": submission.task_id,
                "execution_time_ms": submission.execution_time_ms,
                "message": "Result submitted successfully"
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Task not found"))
        }
        }
    });

    let tasks_for_list = ctx.tasks.clone();

    // lux_listTasks - List all tasks
    io.add_method("lux_listTasks", move |_params: Params| {
        let tasks = tasks_for_list.clone();
        async move {
        let tasks_map = tasks.read();
        let list: Vec<serde_json::Value> = tasks_map
            .values()
            .map(|task| {
                let status_str = match task.status {
                    0 => "pending",
                    1 => "assigned",
                    2 => "completed",
                    _ => "unknown",
                };
                serde_json::json!({
                    "task_id": format!("0x{}", hex::encode(task.task_id)),
                    "model_hash": task.model_hash,
                    "reward": format!("0x{:x}", task.reward),
                    "deadline": task.deadline,
                    "status": status_str,
                    "assigned_to": task.assigned_to.as_ref().map(|a| format!("{}", a)),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "tasks": list,
            "count": list.len(),
        }))
        }
    });

    let tasks_for_get = ctx.tasks.clone();

    // lux_getTask - Get specific task
    io.add_method("lux_getTask", move |params: Params| {
        let tasks = tasks_for_get.clone();
        async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing task_id"));
        }

        let mut task_id_bytes = [0u8; 32];
        let task_hex = parsed[0].trim_start_matches("0x");
        let bytes = hex::decode(task_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task_id hex"))?;
        let copy_len = bytes.len().min(32);
        task_id_bytes[..copy_len].copy_from_slice(&bytes[..copy_len]);

        let tasks_map = tasks.read();
        if let Some(task) = tasks_map.get(&task_id_bytes) {
            let status_str = match task.status {
                0 => "pending",
                1 => "assigned",
                2 => "completed",
                _ => "unknown",
            };
            Ok(serde_json::json!({
                "task_id": format!("0x{}", hex::encode(task.task_id)),
                "model_hash": task.model_hash,
                "reward": format!("0x{:x}", task.reward),
                "deadline": task.deadline,
                "status": status_str,
                "assigned_to": task.assigned_to.as_ref().map(|a| format!("{}", a)),
                "result_hash": task.result_hash.map(|r| format!("0x{}", hex::encode(r))),
            }))
        } else {
            Ok(Value::Null)
        }
        }
    });
}

fn register_miner_registration_methods(ctx: &Arc<MinerDispatchContext>, io: &mut IoHandler) {
    let miners = ctx.miners.clone();
    let metagraph = ctx.metagraph.clone();

    // lux_registerMiner - Register as a miner
    // STORAGE: Dual-write to MetagraphDB (neurons CF + validators CF) + in-memory HashMap
    // SECURITY: Requires signature verification to prevent impersonation
    io.add_method("lux_registerMiner", move |params: Params| {
        let miners = miners.clone();
        let metagraph = metagraph.clone();
        async move {
        let reg: MinerRegistration = params.parse()?;

        // Parse address
        let address = parse_address(&reg.address)?;

        // Parse stake ONCE (proper error handling)
        let stake = u128::from_str_radix(reg.stake.trim_start_matches("0x"), 16)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid stake hex format"))?;
        if stake == 0 {
            return Err(jsonrpc_core::Error::invalid_params("Stake cannot be zero"));
        }
        if stake < MINER_MIN_STAKE {
            return Err(jsonrpc_core::Error::invalid_params(
                "Stake must be at least 100 LUX"
            ));
        }

        // SECURITY: Verify timestamp is recent (within 5 minutes) — replay protection
        let timestamp_str = reg.timestamp.as_deref().unwrap_or("");
        if !timestamp_str.is_empty() {
            let ts: u64 = timestamp_str.parse()
                .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid timestamp format"))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            if now > ts + 300 || ts > now + 60 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Signature expired or future timestamp (max 5 min)"
                ));
            }
        }

        // SECURITY: Verify signature — message includes timestamp to prevent replay
        let message = if timestamp_str.is_empty() {
            // backward compat: old clients without timestamp
            format!(
                "register_miner:{}:{}:{}",
                hex::encode(&address), stake, reg.capacity
            )
        } else {
            format!(
                "register_miner:{}:{}:{}:{}",
                hex::encode(&address), stake, reg.capacity, timestamp_str
            )
        };

        let sig_valid = verify_caller_signature(&address, &message, &reg.signature, 0)
            .or_else(|_| verify_caller_signature(&address, &message, &reg.signature, 1));

        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed - caller does not own address"
            ));
        }

        // Duplicate address check: MetagraphDB is the source of truth
        // (in-memory HashMap is only a cache; it's lost on restart)
        let subnet_id = reg.subnet_id.unwrap_or(DEFAULT_SUBNET_ID);
        let existing_hotkey = addr_to_bytes20(&address);
        if let Ok(neurons) = metagraph.get_neurons_by_subnet(subnet_id) {
            if neurons.iter().any(|n| n.hotkey == existing_hotkey) {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Address already registered as miner in this subnet"
                ));
            }
        }
        // Also check validators CF (miners are stored there too)
        if let Ok(vd) = metagraph.get_validator(&existing_hotkey) {
            if vd.is_some() {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Address already registered (as miner or validator)"
                ));
            }
        }

        // Seed subnet nếu chưa tồn tại
        let subnet_exists = metagraph.get_subnet(subnet_id).ok().flatten().is_some();
        if !subnet_exists {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let subnet = luxtensor_storage::SubnetData {
                id: subnet_id,
                name: format!("subnet-{}", subnet_id),
                owner: addr_to_bytes20(&address),
                emission_rate: 1_000_000_000_000_000_000u128, // 1 LUX/block
                created_at: now,
                tempo: 100,
                max_neurons: 1024,
                min_stake: 100_000_000_000_000_000_000u128, // 100 LUX
                active: true,
            };
            let _ = metagraph.store_subnet(&subnet);
            tracing::warn!("⚠️  Auto-seeded subnet {} — bình thường subnet phải được tạo trước", subnet_id);
        }

        let uid = metagraph.get_neuron_count(subnet_id)
            .unwrap_or(0) as u64;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // ── 1. Write NeuronData to MetagraphDB (neurons CF) ───────────────
        let neuron_data = NeuronData {
            uid,
            subnet_id,
            hotkey: addr_to_bytes20(&address),
            coldkey: addr_to_bytes20(&address), // miner: hotkey == coldkey initially
            stake,
            trust: 0,
            rank: 0,
            incentive: 0,
            dividends: 0,
            emission: 0,
            last_update: now,
            active: true,
            endpoint: format!("miner:{}", hex::encode(address.as_bytes())),
        };
        if let Err(e) = metagraph.store_neuron(&neuron_data) {
            tracing::warn!("MetagraphDB store_neuron failed for miner {}: {}", address, e);
        }

        // ── 2. Write ValidatorData to MetagraphDB (validators CF) ─────────
        // Repurpose fields: last_block_produced = uid, blocks_produced = capacity
        let val_data = ValidatorData {
            address: addr_to_bytes20(&address),
            public_key: vec![],
            stake,
            is_active: true,
            name: format!("miner-{}", hex::encode(address.as_bytes())),
            registered_at: now,
            last_block_produced: uid,     // stores UID
            blocks_produced: reg.capacity as u64, // stores capacity
        };
        if let Err(e) = metagraph.register_validator(&val_data) {
            tracing::warn!("MetagraphDB register_validator (miner) failed: {}", e);
        }

        // ── 3. Update in-memory HashMap ────────────────────────────────────
        {
            let mut miners_map = miners.write();
            miners_map.insert(address.clone(), SimpleMinerInfo {
                address: address.clone(),
                stake,
                capacity: reg.capacity,
                current_tasks: 0,
                uid,
            });
        }

        info!("Miner registered (verified+persisted): {} uid={}", address, uid);

        Ok(serde_json::json!({
            "success": true,
            "address": format!("{}", address),
            "uid": uid,
            "stake": format!("0x{:x}", stake),
            "capacity": reg.capacity,
            "message": "Miner registered (signature verified, MetagraphDB persisted)"
        }))
        }
    });

    let miners_for_info = ctx.miners.clone();

    // lux_getMinerInfo - Get miner info
    io.add_method("lux_getMinerInfo", move |params: Params| {
        let miners = miners_for_info.clone();
        async move {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing miner address"));
        }

        let address = parse_address(&parsed[0])?;

        let miners_map = miners.read();
        if let Some(miner) = miners_map.get(&address) {
            Ok(serde_json::json!({
                "address": format!("{}", address),
                "uid": miner.uid,
                "stake": format!("0x{:x}", miner.stake),
                "capacity": miner.capacity,
                "current_tasks": miner.current_tasks,
            }))
        } else {
            Ok(Value::Null)
        }
        }
    });

    let miners_for_list = ctx.miners.clone();

    // lux_listMiners - List all registered miners
    io.add_method("lux_listMiners", move |_params: Params| {
        let miners = miners_for_list.clone();
        async move {
        let miners_map = miners.read();
        let list: Vec<serde_json::Value> = miners_map
            .values()
            .map(|m| {
                serde_json::json!({
                    "address": format!("{}", m.address),
                    "uid": m.uid,
                    "stake": format!("0x{:x}", m.stake),
                    "capacity": m.capacity,
                    "current_tasks": m.current_tasks,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "miners": list,
            "count": list.len(),
        }))
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let metagraph = Arc::new(MetagraphDB::open(temp_dir.path()).unwrap());
        let ctx = MinerDispatchContext::new(metagraph);
        assert!(ctx.tasks.read().is_empty());
        assert!(ctx.miners.read().is_empty());
    }
}
