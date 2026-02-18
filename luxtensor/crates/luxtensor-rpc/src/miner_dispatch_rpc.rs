//! Miner Dispatch RPC Module
//!
//! This module provides RPC endpoints for miners to:
//! - Claim pending AI tasks
//! - Submit task results
//! - List available tasks
//! - Register as miner
//!
//! SECURITY: All state-changing operations require signature verification

use crate::helpers::{parse_address, verify_caller_signature};
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_core::Address;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;


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
}

/// Context for miner dispatch RPC
pub struct MinerDispatchContext {
    pub tasks: Arc<RwLock<HashMap<[u8; 32], DispatchTaskInfo>>>,
    pub miners: Arc<RwLock<HashMap<Address, SimpleMinerInfo>>>,
}

impl MinerDispatchContext {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            miners: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MinerDispatchContext {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// RPC Registration
// ============================================================

/// Register all miner dispatch RPC methods
pub fn register_miner_dispatch_methods(ctx: Arc<MinerDispatchContext>, io: &mut IoHandler) {
    register_task_dispatch_methods(&ctx, io);
    register_miner_registration_methods(&ctx, io);
}

fn register_task_dispatch_methods(ctx: &Arc<MinerDispatchContext>, io: &mut IoHandler) {
    let tasks = ctx.tasks.clone();

    // lux_listPendingTasks - List available tasks for miners
    io.add_sync_method("lux_listPendingTasks", move |params: Params| {
        let parsed: Option<Vec<serde_json::Value>> = params.parse().ok();
        let limit = parsed
            .and_then(|v| v.first().and_then(|x| x.as_u64()))
            .unwrap_or(100) as usize;

        let tasks_map = tasks.read();
        let pending: Vec<serde_json::Value> = tasks_map
            .values()
            .filter(|t| t.status == 0) // pending only
            .take(limit)
            .map(|t| {
                serde_json::json!({
                    "task_id": format!("0x{}", hex::encode(t.task_id)),
                    "model_hash": t.model_hash,
                    "reward": format!("0x{:x}", t.reward),
                    "deadline": t.deadline,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "tasks": pending,
            "count": pending.len(),
        }))
    });

    let tasks = ctx.tasks.clone();
    let miners_for_claim = ctx.miners.clone();

    // lux_claimTask - Miner claims a pending task
    // SECURITY: Now requires signature verification to prevent impersonation
    io.add_sync_method("lux_claimTask", move |params: Params| {
        let claim: TaskClaimRequest = params.parse()?;

        // Parse task_id
        let task_id_hex = claim.task_id.trim_start_matches("0x");
        let task_id_bytes = hex::decode(task_id_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task ID"))?;
        if task_id_bytes.len() != 32 {
            return Err(jsonrpc_core::Error::invalid_params("Task ID must be 32 bytes"));
        }
        let mut task_id = [0u8; 32];
        task_id.copy_from_slice(&task_id_bytes);

        // Parse miner address
        let miner = parse_address(&claim.miner)?;

        // Security: Verify signature ownership
        // Message format: "claim_task:{task_id}:{miner_address}"
        let message = format!(
            "claim_task:{}:{}",
            hex::encode(&task_id),
            hex::encode(&miner)
        );

        // Try recovery IDs 0 and 1
        let sig_valid = verify_caller_signature(&miner, &message, &claim.signature, 0)
            .or_else(|_| verify_caller_signature(&miner, &message, &claim.signature, 1));

        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed - caller does not own miner address"
            ));
        }

        // Check miner is registered
        {
            let miners_map = miners_for_claim.read();
            if !miners_map.contains_key(&miner) {
                return Err(jsonrpc_core::Error::invalid_params("Miner not registered"));
            }
        }

        // Try to claim task
        let mut tasks_map = tasks.write();
        if let Some(task) = tasks_map.get_mut(&task_id) {
            if task.status != 0 {
                return Err(jsonrpc_core::Error::invalid_params("Task not available"));
            }

            // Assign to miner
            task.status = 1; // assigned
            task.assigned_to = Some(miner.clone());

            info!(
                "Task claimed (verified): 0x{} by {}",
                hex::encode(&task_id[..8]),
                miner
            );

            Ok(serde_json::json!({
                "success": true,
                "task_id": format!("0x{}", hex::encode(task_id)),
                "assigned_to": format!("{}", miner),
                "deadline": task.deadline,
                "message": "Task claimed (signature verified)"
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Task not found"))
        }
    });

    let tasks = ctx.tasks.clone();

    // lux_submitResult - Miner submits task result
    // SECURITY: Now requires signature verification to prevent result spoofing
    io.add_sync_method("lux_submitResult", move |params: Params| {
        let submission: TaskResultSubmission = params.parse()?;

        // Parse and verify miner address + signature
        let miner_addr = parse_address(&submission.miner)?;
        let message = format!(
            "submit_result:{}:{}:{}",
            submission.task_id.trim_start_matches("0x"),
            submission.result_hash.trim_start_matches("0x"),
            hex::encode(&miner_addr)
        );
        let sig_valid = verify_caller_signature(&miner_addr, &message, &submission.signature, 0)
            .or_else(|_| verify_caller_signature(&miner_addr, &message, &submission.signature, 1));
        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed - caller does not own miner address",
            ));
        }

        // Parse task_id
        let task_id_hex = submission.task_id.trim_start_matches("0x");
        let task_id_bytes = hex::decode(task_id_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task ID"))?;
        if task_id_bytes.len() != 32 {
            return Err(jsonrpc_core::Error::invalid_params("Task ID must be 32 bytes"));
        }
        let mut task_id = [0u8; 32];
        task_id.copy_from_slice(&task_id_bytes);

        // Parse result hash
        let result_hex = submission.result_hash.trim_start_matches("0x");
        let result_bytes = hex::decode(result_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid result hash"))?;
        if result_bytes.len() != 32 {
            return Err(jsonrpc_core::Error::invalid_params("Result hash must be 32 bytes"));
        }
        let mut result_hash = [0u8; 32];
        result_hash.copy_from_slice(&result_bytes);

        // Update task
        let mut tasks_map = tasks.write();
        if let Some(task) = tasks_map.get_mut(&task_id) {
            if task.status != 1 {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Task not in assigned state",
                ));
            }

            // SECURITY: Verify the submitter is actually assigned to this task
            if let Some(ref assigned) = task.assigned_to {
                if assigned.as_bytes() != miner_addr.as_bytes() {
                    return Err(jsonrpc_core::Error::invalid_params(
                        "Miner is not assigned to this task",
                    ));
                }
            } else {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Task has no assigned miner",
                ));
            }

            task.status = 2; // completed
            task.result_hash = Some(result_hash);

            info!(
                "Task completed: 0x{} result: 0x{}",
                hex::encode(&task_id[..8]),
                hex::encode(&result_hash[..8])
            );

            Ok(serde_json::json!({
                "success": true,
                "task_id": format!("0x{}", hex::encode(task_id)),
                "result_hash": format!("0x{}", hex::encode(result_hash)),
                "execution_time_ms": submission.execution_time_ms,
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Task not found"))
        }
    });

    let tasks = ctx.tasks.clone();

    // lux_getTaskStatus - Get status of a specific task
    io.add_sync_method("lux_getTaskStatus", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing task ID"));
        }

        let task_id_hex = parsed[0].trim_start_matches("0x");
        let task_id_bytes = hex::decode(task_id_hex)
            .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task ID"))?;
        if task_id_bytes.len() != 32 {
            return Err(jsonrpc_core::Error::invalid_params("Task ID must be 32 bytes"));
        }
        let mut task_id = [0u8; 32];
        task_id.copy_from_slice(&task_id_bytes);

        let tasks_map = tasks.read();
        if let Some(task) = tasks_map.get(&task_id) {
            let status_str = match task.status {
                0 => "pending",
                1 => "assigned",
                2 => "completed",
                _ => "unknown",
            };

            Ok(serde_json::json!({
                "task_id": format!("0x{}", hex::encode(task_id)),
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
    });
}

fn register_miner_registration_methods(ctx: &Arc<MinerDispatchContext>, io: &mut IoHandler) {
    let miners = ctx.miners.clone();

    // lux_registerMiner - Register as a miner
    // SECURITY: Now requires signature verification to prevent impersonation
    io.add_sync_method("lux_registerMiner", move |params: Params| {
        let reg: MinerRegistration = params.parse()?;

        // Parse address
        let address = parse_address(&reg.address)?;

        // Parse stake
        let stake = u128::from_str_radix(reg.stake.trim_start_matches("0x"), 16).unwrap_or(0);

        // Security: Verify signature ownership
        // Message format: "register_miner:{address}:{stake}:{capacity}"
        let message = format!(
            "register_miner:{}:{}:{}",
            hex::encode(&address),
            stake,
            reg.capacity
        );

        // Try recovery IDs 0 and 1
        let sig_valid = verify_caller_signature(&address, &message, &reg.signature, 0)
            .or_else(|_| verify_caller_signature(&address, &message, &reg.signature, 1));

        if sig_valid.is_err() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Signature verification failed - caller does not own address"
            ));
        }

        // Register miner
        let miner = SimpleMinerInfo {
            address: address.clone(),
            stake,
            capacity: reg.capacity,
            current_tasks: 0,
        };

        {
            let mut miners_map = miners.write();
            miners_map.insert(address.clone(), miner);
        }

        info!("Miner registered (verified): {}", address);

        Ok(serde_json::json!({
            "success": true,
            "address": format!("{}", address),
            "stake": format!("0x{:x}", stake),
            "capacity": reg.capacity,
            "message": "Miner registered (signature verified)"
        }))
    });

    let miners = ctx.miners.clone();

    // lux_getMinerInfo - Get miner info
    io.add_sync_method("lux_getMinerInfo", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing miner address"));
        }

        let address = parse_address(&parsed[0])?;

        let miners_map = miners.read();
        if let Some(miner) = miners_map.get(&address) {
            Ok(serde_json::json!({
                "address": format!("{}", address),
                "stake": format!("0x{:x}", miner.stake),
                "capacity": miner.capacity,
                "current_tasks": miner.current_tasks,
            }))
        } else {
            Ok(Value::Null)
        }
    });

    let miners = ctx.miners.clone();

    // lux_listMiners - List all registered miners
    io.add_sync_method("lux_listMiners", move |_params: Params| {
        let miners_map = miners.read();
        let list: Vec<serde_json::Value> = miners_map
            .values()
            .map(|m| {
                serde_json::json!({
                    "address": format!("{}", m.address),
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
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = MinerDispatchContext::new();
        assert!(ctx.tasks.read().is_empty());
        assert!(ctx.miners.read().is_empty());
    }
}
