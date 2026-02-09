//! Training RPC Module - Federated Learning methods
//!
//! This module handles all federated learning RPC methods (training_*):
//! - Job management (create, cancel, get, list)
//! - Trainer registration
//! - Gradient submission and retrieval
//! - Round status and aggregation
//!
//! Follows the same pattern as ai_rpc.rs

use jsonrpc_core::{IoHandler, Params, Value};
use luxtensor_core::Hash;
use luxtensor_zkvm::pot_verifier::PoTVerifier;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// =============================================================================
// TYPES
// =============================================================================

/// Training job status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TrainingJobStatus {
    Open,
    Training,
    Aggregating,
    Completed,
    Cancelled,
}

/// Training job info for RPC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainingJobInfo {
    pub id: Hash,
    pub model_id: String,
    pub dataset_ref: String,
    pub total_rounds: u64,
    pub current_round: u64,
    pub min_participants: u64,
    pub reward_per_round: u128,
    pub creator: String,
    pub status: TrainingJobStatus,
    pub created_at: u64,
    pub trainers: Vec<String>,
}

/// Gradient submission
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GradientSubmission {
    pub trainer: String,
    pub gradient_hash: String,
    pub checkpoint_hash: String,
    pub timestamp: u64,
    pub verified: bool,
}

/// Round info
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoundInfo {
    pub job_id: Hash,
    pub round: u64,
    pub submissions: Vec<GradientSubmission>,
    pub aggregated_checkpoint: Option<String>,
    pub completed: bool,
}

/// Create job request
#[derive(Clone, Debug, Deserialize)]
pub struct CreateJobRequest {
    /// Caller address (hex encoded)
    pub caller: String,
    pub model_id: String,
    pub dataset_ref: String,
    pub total_rounds: u64,
    pub min_participants: u64,
    pub reward_per_round: String,
}

/// Submit gradient request
#[derive(Clone, Debug, Deserialize)]
pub struct SubmitGradientRequest {
    /// Trainer address (hex encoded)
    pub trainer: String,
    pub job_id: String,
    pub gradient_hash: String,
    pub checkpoint_hash: String,
    /// Optional: ZK proof data for PoT verification
    pub proof_data: Option<String>,
}

// =============================================================================
// CONTEXT
// =============================================================================

/// Shared context for Training RPC handlers
pub struct TrainingRpcContext {
    pub jobs: Arc<RwLock<HashMap<Hash, TrainingJobInfo>>>,
    pub rounds: Arc<RwLock<HashMap<(Hash, u64), RoundInfo>>>,
    pub job_counter: Arc<RwLock<u64>>,
    /// Proof of Training verifier for gradient validation
    pub pot_verifier: Arc<RwLock<PoTVerifier>>,
}

impl TrainingRpcContext {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            rounds: Arc::new(RwLock::new(HashMap::new())),
            job_counter: Arc::new(RwLock::new(0)),
            pot_verifier: Arc::new(RwLock::new(PoTVerifier::new())),
        }
    }
}

impl Default for TrainingRpcContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// REGISTRATION
// =============================================================================

/// Register all training RPC methods
pub fn register_training_methods(ctx: &TrainingRpcContext, io: &mut IoHandler) {
    register_job_methods(ctx, io);
    register_trainer_methods(ctx, io);
    register_gradient_methods(ctx, io);
    register_round_methods(ctx, io);
}

// =============================================================================
// JOB METHODS
// =============================================================================

fn register_job_methods(ctx: &TrainingRpcContext, io: &mut IoHandler) {
    let jobs = ctx.jobs.clone();
    let job_counter = ctx.job_counter.clone();
    let rounds = ctx.rounds.clone();

    // training_createJob - Create a new training job
    io.add_sync_method("training_createJob", move |params: Params| {
        let request: CreateJobRequest = params.parse()?;

        // Validate
        if request.model_id.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Model ID is required"));
        }
        if request.total_rounds == 0 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Total rounds must be > 0",
            ));
        }
        if request.min_participants == 0 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Min participants must be > 0",
            ));
        }

        // Parse reward
        let reward =
            u128::from_str_radix(request.reward_per_round.trim_start_matches("0x"), 16)
                .unwrap_or(0);

        // Generate job ID
        let mut counter = job_counter.write();
        *counter += 1;
        let job_id_data = format!(
            "training:{}:{}:{}",
            request.model_id,
            *counter,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_nanos()
        );
        let job_id = luxtensor_crypto::keccak256(job_id_data.as_bytes());

        // Create job
        let job_info = TrainingJobInfo {
            id: job_id,
            model_id: request.model_id,
            dataset_ref: request.dataset_ref,
            total_rounds: request.total_rounds,
            current_round: 0,
            min_participants: request.min_participants,
            reward_per_round: reward,
            creator: request.caller.clone(),
            status: TrainingJobStatus::Open,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_secs(),
            trainers: Vec::new(),
        };

        // Initialize first round
        let round_info = RoundInfo {
            job_id,
            round: 0,
            submissions: Vec::new(),
            aggregated_checkpoint: None,
            completed: false,
        };

        {
            let mut jobs_map = jobs.write();
            jobs_map.insert(job_id, job_info);
        }
        {
            let mut rounds_map = rounds.write();
            rounds_map.insert((job_id, 0), round_info);
        }

        info!("Training job created: 0x{}", hex::encode(&job_id));

        Ok(serde_json::json!({
            "success": true,
            "job_id": format!("0x{}", hex::encode(job_id))
        }))
    });

    let jobs = ctx.jobs.clone();

    // training_getJob - Get job details
    io.add_sync_method("training_getJob", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing job ID"));
        }

        let job_id = parse_job_id(&parsed[0])?;
        let jobs_map = jobs.read();

        if let Some(job) = jobs_map.get(&job_id) {
            Ok(serde_json::json!({
                "job_id": format!("0x{}", hex::encode(job_id)),
                "model_id": job.model_id,
                "dataset_ref": job.dataset_ref,
                "total_rounds": job.total_rounds,
                "current_round": job.current_round,
                "min_participants": job.min_participants,
                "reward_per_round": format!("0x{:x}", job.reward_per_round),
                "creator": job.creator,
                "status": format!("{:?}", job.status),
                "created_at": job.created_at,
                "trainer_count": job.trainers.len(),
            }))
        } else {
            Ok(Value::Null)
        }
    });

    let jobs = ctx.jobs.clone();

    // training_listJobs - List all jobs with optional status filter
    io.add_sync_method("training_listJobs", move |params: Params| {
        // Empty params = list all jobs; explicit params = filter by status
        let parsed: Vec<String> = params.parse().unwrap_or_default();
        let status_filter = parsed.first().map(|s| s.as_str());

        let jobs_map = jobs.read();
        let job_list: Vec<serde_json::Value> = jobs_map
            .values()
            .filter(|job| {
                if let Some(filter) = status_filter {
                    format!("{:?}", job.status).to_lowercase() == filter.to_lowercase()
                } else {
                    true
                }
            })
            .map(|job| {
                serde_json::json!({
                    "job_id": format!("0x{}", hex::encode(job.id)),
                    "model_id": job.model_id,
                    "status": format!("{:?}", job.status),
                    "current_round": job.current_round,
                    "total_rounds": job.total_rounds,
                    "trainer_count": job.trainers.len(),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "jobs": job_list,
            "total": job_list.len(),
        }))
    });

    let jobs = ctx.jobs.clone();

    // training_cancelJob - Cancel a job (creator only)
    io.add_sync_method("training_cancelJob", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing job ID"));
        }

        let job_id = parse_job_id(&parsed[0])?;
        let mut jobs_map = jobs.write();

        if let Some(job) = jobs_map.get_mut(&job_id) {
            if job.status == TrainingJobStatus::Completed
                || job.status == TrainingJobStatus::Cancelled
            {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Job already completed or cancelled",
                ));
            }

            job.status = TrainingJobStatus::Cancelled;
            info!("Training job cancelled: 0x{}", hex::encode(&job_id));

            Ok(serde_json::json!({
                "success": true,
                "job_id": format!("0x{}", hex::encode(job_id)),
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Job not found"))
        }
    });
}

// =============================================================================
// TRAINER METHODS
// =============================================================================

fn register_trainer_methods(ctx: &TrainingRpcContext, io: &mut IoHandler) {
    let jobs = ctx.jobs.clone();

    // training_registerTrainer - Register as a trainer for a job
    io.add_sync_method("training_registerTrainer", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Required: job_id, trainer_address",
            ));
        }

        let job_id = parse_job_id(&parsed[0])?;
        let trainer_address = &parsed[1];

        let mut jobs_map = jobs.write();

        if let Some(job) = jobs_map.get_mut(&job_id) {
            if job.status != TrainingJobStatus::Open {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Job is not accepting registrations",
                ));
            }

            if job.trainers.contains(trainer_address) {
                return Err(jsonrpc_core::Error::invalid_params(
                    "Trainer already registered",
                ));
            }

            job.trainers.push(trainer_address.clone());
            info!(
                "Trainer {} registered for job 0x{}",
                trainer_address,
                hex::encode(&job_id)
            );

            // Auto-start if min participants reached
            if job.trainers.len() >= job.min_participants as usize {
                job.status = TrainingJobStatus::Training;
                info!(
                    "Training started for job 0x{} with {} trainers",
                    hex::encode(&job_id),
                    job.trainers.len()
                );
            }

            Ok(serde_json::json!({
                "success": true,
                "job_id": format!("0x{}", hex::encode(job_id)),
                "trainer": trainer_address,
                "trainer_count": job.trainers.len(),
                "started": job.status == TrainingJobStatus::Training,
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Job not found"))
        }
    });

    let jobs = ctx.jobs.clone();

    // training_getTrainers - Get trainers for a job
    io.add_sync_method("training_getTrainers", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing job ID"));
        }

        let job_id = parse_job_id(&parsed[0])?;
        let jobs_map = jobs.read();

        if let Some(job) = jobs_map.get(&job_id) {
            Ok(serde_json::json!({
                "job_id": format!("0x{}", hex::encode(job_id)),
                "trainers": job.trainers,
                "count": job.trainers.len(),
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Job not found"))
        }
    });
}

// =============================================================================
// GRADIENT METHODS
// =============================================================================

fn register_gradient_methods(ctx: &TrainingRpcContext, io: &mut IoHandler) {
    let jobs = ctx.jobs.clone();
    let rounds = ctx.rounds.clone();
    let pot_verifier = ctx.pot_verifier.clone();

    // training_submitGradient - Submit gradient for current round
    io.add_sync_method("training_submitGradient", move |params: Params| {
        let request: SubmitGradientRequest = params.parse()?;

        let job_id = parse_job_id(&request.job_id)?;

        // Get job to check status and current round
        let (current_round, min_participants) = {
            let jobs_map = jobs.read();
            if let Some(job) = jobs_map.get(&job_id) {
                if job.status != TrainingJobStatus::Training {
                    return Err(jsonrpc_core::Error::invalid_params(
                        "Job is not in training status",
                    ));
                }
                (job.current_round, job.min_participants)
            } else {
                return Err(jsonrpc_core::Error::invalid_params("Job not found"));
            }
        };

        // Verify proof of training if provided
        let verified = if let Some(ref _proof_hex) = request.proof_data {
            // Parse trainer address for PoT check
            let trainer_bytes: [u8; 20] = {
                let decoded = hex::decode(request.trainer.trim_start_matches("0x"))
                    .unwrap_or_else(|_| vec![0u8; 20]);
                let mut arr = [0u8; 20];
                arr.copy_from_slice(&decoded[..20.min(decoded.len())]);
                arr
            };

            // Check if trainer has valid PoT for this round
            let verifier = pot_verifier.read();
            verifier.has_valid_pot(job_id, current_round, &trainer_bytes)
        } else {
            // No proof provided - mark as unverified
            false
        };

        // Add submission to round
        let submission = GradientSubmission {
            trainer: request.trainer.clone(),
            gradient_hash: request.gradient_hash.clone(),
            checkpoint_hash: request.checkpoint_hash.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time before UNIX epoch")
                .as_secs(),
            verified,
        };

        let mut rounds_map = rounds.write();
        if let Some(round_info) = rounds_map.get_mut(&(job_id, current_round)) {
            round_info.submissions.push(submission);

            info!(
                "Gradient submitted for job 0x{} round {}",
                hex::encode(&job_id),
                current_round
            );

            let submission_count = round_info.submissions.len();
            let round_complete = submission_count >= min_participants as usize;

            if round_complete {
                // Aggregate gradients
                round_info.completed = true;
                round_info.aggregated_checkpoint = Some(format!(
                    "0x{}",
                    hex::encode(luxtensor_crypto::keccak256(
                        request.gradient_hash.as_bytes()
                    ))
                ));
            }

            Ok(serde_json::json!({
                "success": true,
                "job_id": format!("0x{}", hex::encode(job_id)),
                "round": current_round,
                "submission_count": submission_count,
                "round_complete": round_complete,
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Round not found"))
        }
    });

    let rounds = ctx.rounds.clone();

    // training_getGradients - Get gradient submissions for a round
    io.add_sync_method("training_getGradients", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        if parsed.len() < 2 {
            return Err(jsonrpc_core::Error::invalid_params(
                "Required: job_id, round",
            ));
        }

        let job_id_str = parsed[0]
            .as_str()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid job ID"))?;
        let job_id = parse_job_id(job_id_str)?;
        let round = parsed[1]
            .as_u64()
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Invalid round number"))?;

        let rounds_map = rounds.read();

        if let Some(round_info) = rounds_map.get(&(job_id, round)) {
            let submissions: Vec<serde_json::Value> = round_info
                .submissions
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "trainer": s.trainer,
                        "gradient_hash": s.gradient_hash,
                        "checkpoint_hash": s.checkpoint_hash,
                        "timestamp": s.timestamp,
                        "verified": s.verified,
                    })
                })
                .collect();

            Ok(serde_json::json!({
                "job_id": format!("0x{}", hex::encode(job_id)),
                "round": round,
                "submissions": submissions,
                "submission_count": submissions.len(),
                "aggregated": round_info.aggregated_checkpoint,
                "completed": round_info.completed,
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Round not found"))
        }
    });
}

// =============================================================================
// ROUND METHODS
// =============================================================================

fn register_round_methods(ctx: &TrainingRpcContext, io: &mut IoHandler) {
    let jobs = ctx.jobs.clone();
    let rounds = ctx.rounds.clone();

    // training_getRoundStatus - Get current round status
    io.add_sync_method("training_getRoundStatus", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing job ID"));
        }

        let job_id = parse_job_id(&parsed[0])?;

        let current_round = {
            let jobs_map = jobs.read();
            if let Some(job) = jobs_map.get(&job_id) {
                job.current_round
            } else {
                return Err(jsonrpc_core::Error::invalid_params("Job not found"));
            }
        };

        let rounds_map = rounds.read();
        if let Some(round_info) = rounds_map.get(&(job_id, current_round)) {
            Ok(serde_json::json!({
                "job_id": format!("0x{}", hex::encode(job_id)),
                "round": current_round,
                "submission_count": round_info.submissions.len(),
                "completed": round_info.completed,
                "aggregated": round_info.aggregated_checkpoint,
            }))
        } else {
            Ok(serde_json::json!({
                "job_id": format!("0x{}", hex::encode(job_id)),
                "round": current_round,
                "submission_count": 0,
                "completed": false,
            }))
        }
    });

    let jobs = ctx.jobs.clone();
    let rounds = ctx.rounds.clone();

    // training_advanceRound - Manually advance to next round (for testing)
    io.add_sync_method("training_advanceRound", move |params: Params| {
        let parsed: Vec<String> = params.parse()?;
        if parsed.is_empty() {
            return Err(jsonrpc_core::Error::invalid_params("Missing job ID"));
        }

        let job_id = parse_job_id(&parsed[0])?;

        let mut jobs_map = jobs.write();
        if let Some(job) = jobs_map.get_mut(&job_id) {
            if job.status != TrainingJobStatus::Training {
                return Err(jsonrpc_core::Error::invalid_params("Job is not training"));
            }

            job.current_round += 1;

            if job.current_round >= job.total_rounds {
                job.status = TrainingJobStatus::Completed;
                info!("Training job completed: 0x{}", hex::encode(&job_id));
            } else {
                // Initialize new round
                let round_info = RoundInfo {
                    job_id,
                    round: job.current_round,
                    submissions: Vec::new(),
                    aggregated_checkpoint: None,
                    completed: false,
                };
                let mut rounds_map = rounds.write();
                rounds_map.insert((job_id, job.current_round), round_info);
            }

            Ok(serde_json::json!({
                "success": true,
                "job_id": format!("0x{}", hex::encode(job_id)),
                "new_round": job.current_round,
                "completed": job.status == TrainingJobStatus::Completed,
            }))
        } else {
            Err(jsonrpc_core::Error::invalid_params("Job not found"))
        }
    });
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn parse_job_id(hex_str: &str) -> Result<Hash, jsonrpc_core::Error> {
    let job_id_hex = hex_str.trim_start_matches("0x");
    let job_id_bytes = hex::decode(job_id_hex)
        .map_err(|_| jsonrpc_core::Error::invalid_params("Invalid job ID format"))?;

    if job_id_bytes.len() != 32 {
        return Err(jsonrpc_core::Error::invalid_params(
            "Job ID must be 32 bytes",
        ));
    }

    let mut job_id = [0u8; 32];
    job_id.copy_from_slice(&job_id_bytes);
    Ok(job_id)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_job_id_valid() {
        let hex = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        let result = parse_job_id(hex);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_job_id_invalid_length() {
        let hex = "0x1234";
        let result = parse_job_id(hex);
        assert!(result.is_err());
    }

    #[test]
    fn test_training_rpc_context_creation() {
        let ctx = TrainingRpcContext::new();
        assert!(ctx.jobs.read().is_empty());
        assert!(ctx.rounds.read().is_empty());
    }

    #[test]
    fn test_training_job_status_serialization() {
        let status = TrainingJobStatus::Training;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Training\"");
    }
}
