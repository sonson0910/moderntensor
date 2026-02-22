//! AI RPC Module - AI-specific and Network methods
//!
//! This module handles all AI task methods (lux_*), metagraph queries (ai_*),
//! and network info methods (net_*, web3_*).
//! Refactored from server.rs to follow clean-code principles.

use crate::helpers::parse_address;
use crate::types::{AITaskInfo, AITaskRequest, AITaskStatus, NeuronInfo, SubnetInfo};
use dashmap::DashMap;
use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_consensus::ValidatorSet;
use luxtensor_core::Hash;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::info;

/// Shared context for AI RPC handlers
pub struct AiRpcContext {
    pub ai_tasks: Arc<DashMap<Hash, AITaskInfo>>,
    pub validators: Arc<RwLock<ValidatorSet>>,
    pub neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
    pub subnets: Arc<DashMap<u64, SubnetInfo>>,
}

impl AiRpcContext {
    pub fn new(
        ai_tasks: Arc<DashMap<Hash, AITaskInfo>>,
        validators: Arc<RwLock<ValidatorSet>>,
        neurons: Arc<DashMap<(u64, u64), NeuronInfo>>,
        subnets: Arc<DashMap<u64, SubnetInfo>>,
    ) -> Self {
        Self {
            ai_tasks,
            validators,
            neurons,
            subnets,
        }
    }
}

/// Register all AI-specific methods
pub fn register_ai_methods(ctx: &AiRpcContext, io: &mut IoHandler) {
    register_task_methods(ctx, io);
    register_validator_methods(ctx, io);
    register_metagraph_methods(ctx, io);
    register_network_methods(io);
}

// =============================================================================
// TASK METHODS
// =============================================================================

fn register_task_methods(ctx: &AiRpcContext, io: &mut IoHandler) {
    let ai_tasks = ctx.ai_tasks.clone();

    // lux_submitAITask - Submit AI computation task
    io.add_sync_method("lux_submitAITask", move |params: Params| {
        let task_request: AITaskRequest = params.parse()?;

        // Validate
        if task_request.model_hash.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Model hash is required"));
        }
        if task_request.requester.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Requester address is required",
            ));
        }

        // Parse reward
        let reward =
            u128::from_str_radix(task_request.reward.trim_start_matches("0x"), 16).unwrap_or(0);

        // Generate task ID
        let task_id_data = format!(
            "{}:{}:{}:{}",
            task_request.model_hash,
            task_request.requester,
            task_request.input_data,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_nanos()
        );
        let task_id = luxtensor_crypto::keccak256(task_id_data.as_bytes());

        // Create task
        let task_info = AITaskInfo {
            id: task_id,
            model_hash: task_request.model_hash,
            input_data: task_request.input_data,
            requester: task_request.requester,
            reward,
            status: AITaskStatus::Pending,
            result: None,
            worker: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs(),
            completed_at: None,
        };

        {
            ai_tasks.insert(task_id, task_info);
            info!("AI task submitted: 0x{}", hex::encode(&task_id));
        }

        Ok(serde_json::json!({
            "success": true,
            "task_id": format!("0x{}", hex::encode(task_id))
        }))
    });

    let ai_tasks = ctx.ai_tasks.clone();

    // lux_getAIResult - Get AI task result
    io.add_sync_method("lux_getAIResult", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing task ID"));
        }

        let task_id = parse_task_id(&parsed[0])?;

        if let Some(task) = ai_tasks.get(&task_id) {
            let status_str = match task.status {
                AITaskStatus::Pending => "pending",
                AITaskStatus::Processing => "processing",
                AITaskStatus::Completed => "completed",
                AITaskStatus::Failed => "failed",
            };

            Ok(serde_json::json!({
                "task_id": format!("0x{}", hex::encode(task_id)),
                "status": status_str,
                "model_hash": task.model_hash,
                "requester": task.requester,
                "reward": format!("0x{:x}", task.reward),
                "result": task.result,
                "worker": task.worker,
                "created_at": task.created_at,
                "completed_at": task.completed_at,
            }))
        } else {
            Ok(Value::Null)
        }
    });
}

// =============================================================================
// VALIDATOR METHODS
// =============================================================================

fn register_validator_methods(ctx: &AiRpcContext, io: &mut IoHandler) {
    let validators = ctx.validators.clone();

    // lux_getValidatorStatus - Get validator information
    io.add_sync_method("lux_getValidatorStatus", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params(
                "Missing validator address",
            ));
        }

        let address = parse_address(&parsed[0])?;

        let validator_set = validators.read();
        if let Some(validator) = validator_set.get_validator(&address) {
            Ok(serde_json::json!({
                "address": format!("0x{}", hex::encode(address.as_bytes())),
                "stake": format!("0x{:x}", validator.stake),
                "active": validator.active,
                "rewards": format!("0x{:x}", validator.rewards),
                "public_key": format!("0x{}", hex::encode(validator.public_key)),
            }))
        } else {
            Ok(Value::Null)
        }
    });
}

// =============================================================================
// METAGRAPH METHODS
// =============================================================================

fn register_metagraph_methods(ctx: &AiRpcContext, io: &mut IoHandler) {
    let neurons = ctx.neurons.clone();
    let subnets = ctx.subnets.clone();

    // ai_getMetagraph - Get metagraph for subnet
    io.add_sync_method("ai_getMetagraph", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let neurons_in_subnet: Vec<serde_json::Value> = neurons
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let n = entry.value();
                serde_json::json!({
                    "uid": n.uid,
                    "address": n.address,
                    "stake": format!("0x{:x}", n.stake),
                    "trust": n.trust,
                    "rank": n.rank,
                    "incentive": n.incentive,
                    "dividends": n.dividends,
                    "active": n.active,
                })
            })
            .collect();

        let total_stake = subnets.get(&subnet_id)
            .map(|s| format!("0x{:x}", s.total_stake))
            .unwrap_or_else(|| "0x0".to_string());

        Ok(serde_json::json!({
            "subnet_id": subnet_id,
            "neurons": neurons_in_subnet,
            "neuron_count": neurons_in_subnet.len(),
            "total_stake": total_stake,
        }))
    });

    let neurons = ctx.neurons.clone();

    // ai_getIncentive - Get incentive info for subnet
    io.add_sync_method("ai_getIncentive", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing subnet ID"));
        }
        let subnet_id = parsed[0]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid subnet ID"))?;

        let incentives: Vec<serde_json::Value> = neurons
            .iter()
            .filter(|entry| entry.key().0 == subnet_id)
            .map(|entry| {
                let n = entry.value();
                serde_json::json!({
                    "uid": n.uid,
                    "incentive": n.incentive,
                    "dividends": n.dividends,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "subnet_id": subnet_id,
            "incentives": incentives,
        }))
    });
}

// =============================================================================
// NETWORK METHODS
// =============================================================================

fn register_network_methods(io: &mut IoHandler) {
    // net_version - Get network version
    io.add_sync_method("net_version", move |_params: Params| {
        Ok(Value::String("1".to_string()))
    });

    // net_peerCount - Get peer count
    io.add_sync_method("net_peerCount", move |_params: Params| {
        let count = crate::peer_count::get_peer_count();
        Ok(Value::String(format!("0x{:x}", count)))
    });

    // web3_clientVersion - Get client version
    io.add_sync_method("web3_clientVersion", move |_params: Params| {
        Ok(Value::String(format!(
            "Luxtensor/{}",
            env!("CARGO_PKG_VERSION")
        )))
    });
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn parse_task_id(hex_str: &str) -> Result<Hash, jsonrpc_core::Error> {
    let task_id_hex = hex_str.trim_start_matches("0x");
    let task_id_bytes = hex::decode(task_id_hex)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid task ID format"))?;

    if task_id_bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Task ID must be 32 bytes",
        ));
    }

    let mut task_id = [0u8; 32];
    task_id.copy_from_slice(&task_id_bytes);
    Ok(task_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task_id_valid() {
        let hex = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let result = parse_task_id(hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_task_id_invalid_length() {
        let hex = "0x1234";
        let result = parse_task_id(hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_ai_rpc_context_creation() {
        let ai_tasks = Arc::new(DashMap::new());
        let validators = Arc::new(RwLock::new(ValidatorSet::new()));
        let neurons = Arc::new(DashMap::new());
        let subnets = Arc::new(DashMap::new());

        let ctx = AiRpcContext::new(ai_tasks, validators, neurons, subnets);
        assert!(ctx.ai_tasks.is_empty());
    }
}
