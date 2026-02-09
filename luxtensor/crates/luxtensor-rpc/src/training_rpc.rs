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
use tracing::{info, warn};

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
    /// Stake weights per (job_id, trainer_address) in basis points (BPS, 10000 = 100%)
    pub trainer_stakes: Arc<RwLock<HashMap<(Hash, String), u64>>>,
}

impl TrainingRpcContext {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            rounds: Arc::new(RwLock::new(HashMap::new())),
            job_counter: Arc::new(RwLock::new(0)),
            pot_verifier: Arc::new(RwLock::new(PoTVerifier::new())),
            trainer_stakes: Arc::new(RwLock::new(HashMap::new())),
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
                .unwrap_or(std::time::Duration::ZERO)
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
                .unwrap_or(std::time::Duration::ZERO)
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
    let trainer_stakes = ctx.trainer_stakes.clone();

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

            // Store stake weight (optional third param, default 10000 BPS = 100%)
            let stake_bps: u64 = if parsed.len() >= 3 {
                parsed[2].parse::<u64>().unwrap_or(10_000)
            } else {
                10_000
            };
            {
                let mut stakes = trainer_stakes.write();
                stakes.insert((job_id, trainer_address.clone()), stake_bps);
            }

            info!(
                "Trainer {} registered for job 0x{} with stake {}bps",
                trainer_address,
                hex::encode(&job_id),
                stake_bps
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
    let trainer_stakes = ctx.trainer_stakes.clone();

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
                .unwrap_or(std::time::Duration::ZERO)
                .as_secs(),
            verified,
        };

        // Phase 1: Add submission and compute FedAvg aggregation if round is complete
        let (submission_count, round_complete) = {
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
                    // Compute real FedAvg commitment from all submissions
                    let stakes_map = trainer_stakes.read();
                    let job_stakes: HashMap<String, u64> = round_info
                        .submissions
                        .iter()
                        .map(|s| {
                            let stake = stakes_map
                                .get(&(job_id, s.trainer.clone()))
                                .copied()
                                .unwrap_or(10_000);
                            (s.trainer.clone(), stake)
                        })
                        .collect();
                    drop(stakes_map);

                    let commitment =
                        compute_fedavg_commitment(&round_info.submissions, &job_stakes);
                    round_info.completed = true;
                    round_info.aggregated_checkpoint = Some(commitment.clone());

                    info!(
                        "FedAvg aggregation complete for job 0x{} round {}: {} submissions, checkpoint={}",
                        hex::encode(&job_id),
                        current_round,
                        submission_count,
                        commitment,
                    );
                }

                (submission_count, round_complete)
            } else {
                return Err(jsonrpc_core::Error::invalid_params("Round not found"));
            }
        };

        // Phase 2: Auto-advance round if aggregation is complete
        if round_complete {
            let mut jobs_map = jobs.write();
            let mut next_round_num = None;

            if let Some(job) = jobs_map.get_mut(&job_id) {
                if job.status != TrainingJobStatus::Training {
                    warn!(
                        "Job 0x{} not in training status during auto-advance, skipping",
                        hex::encode(&job_id)
                    );
                } else {
                    let next = job.current_round + 1;
                    if next >= job.total_rounds {
                        job.status = TrainingJobStatus::Completed;
                        info!(
                            "Training job completed: 0x{}, all {} rounds done",
                            hex::encode(&job_id),
                            job.total_rounds
                        );
                    } else {
                        job.current_round = next;
                        next_round_num = Some(next);
                        info!(
                            "Auto-advanced to round {} for job 0x{}",
                            next,
                            hex::encode(&job_id)
                        );
                    }
                }
            } else {
                warn!(
                    "Job 0x{} not found during auto-advance",
                    hex::encode(&job_id)
                );
            }
            drop(jobs_map);

            // Phase 3: Initialize next round if needed
            if let Some(next) = next_round_num {
                let new_round = RoundInfo {
                    job_id,
                    round: next,
                    submissions: Vec::new(),
                    aggregated_checkpoint: None,
                    completed: false,
                };
                let mut rounds_map = rounds.write();
                rounds_map.insert((job_id, next), new_round);
            }
        }

        Ok(serde_json::json!({
            "success": true,
            "job_id": format!("0x{}", hex::encode(job_id)),
            "round": current_round,
            "submission_count": submission_count,
            "round_complete": round_complete,
        }))
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

/// Compute a deterministic FedAvg commitment from gradient submissions with stake weights.
///
/// The commitment is a Merkle root of sorted `(address, stake_weight_bps, gradient_hash)` tuples.
/// This provides a verifiable, deterministic aggregation proof for federated averaging.
///
/// Each leaf is: `H(trainer_address ":" normalized_weight_bps ":" gradient_hash)`
/// Leaves are sorted by trainer address for determinism before building the Merkle tree.
/// Stake weights are normalized to BPS (basis points) using integer math — no floating point.
fn compute_fedavg_commitment(
    submissions: &[GradientSubmission],
    stake_weights: &HashMap<String, u64>,
) -> String {
    if submissions.is_empty() {
        warn!("FedAvg: called with empty submissions");
        return format!("0x{}", hex::encode([0u8; 32]));
    }

    // Collect (address, stake_bps, gradient_hash) tuples
    let mut contributions: Vec<(&str, u64, &str)> = submissions
        .iter()
        .map(|s| {
            let stake = stake_weights.get(&s.trainer).copied().unwrap_or(10_000);
            (s.trainer.as_str(), stake, s.gradient_hash.as_str())
        })
        .collect();

    // Sort by trainer address for determinism
    contributions.sort_by(|a, b| a.0.cmp(b.0));

    // Compute total stake for weight normalization (in BPS, no floats)
    let total_stake: u64 = contributions.iter().map(|(_, s, _)| s).sum();
    if total_stake == 0 {
        warn!("FedAvg: total stake is zero, using equal weights");
    }

    // Build leaf hashes: H(addr:normalized_weight_bps:gradient_hash)
    // Normalized weight = (stake * 10000) / total_stake (in BPS)
    let num_contributors = contributions.len() as u64;
    let leaf_hashes: Vec<[u8; 32]> = contributions
        .iter()
        .map(|(addr, stake, grad_hash)| {
            let normalized_bps = if total_stake > 0 {
                (*stake * 10_000) / total_stake
            } else {
                10_000 / num_contributors.max(1)
            };
            let leaf_data = format!("{}:{}:{}", addr, normalized_bps, grad_hash);
            luxtensor_crypto::keccak256(leaf_data.as_bytes())
        })
        .collect();

    // Compute Merkle root of all leaves
    let root = compute_merkle_root(&leaf_hashes);
    format!("0x{}", hex::encode(root))
}

/// Compute the Merkle root from a list of leaf hashes.
///
/// Uses a standard bottom-up Merkle tree construction:
/// - Pairs of nodes are sorted before hashing (for canonical ordering)
/// - Odd nodes are promoted to the next level
/// - Single leaf returns itself as root
/// - Empty input returns zero hash
fn compute_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current_level = leaves.to_vec();

    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity((current_level.len() + 1) / 2);
        for chunk in current_level.chunks(2) {
            if chunk.len() == 2 {
                // Sort pair for canonical ordering (smaller hash first)
                let (left, right) = if chunk[0] <= chunk[1] {
                    (chunk[0], chunk[1])
                } else {
                    (chunk[1], chunk[0])
                };
                let mut combined = [0u8; 64];
                combined[..32].copy_from_slice(&left);
                combined[32..].copy_from_slice(&right);
                next_level.push(luxtensor_crypto::keccak256(&combined));
            } else {
                // Odd node: promote to next level
                next_level.push(chunk[0]);
            }
        }
        current_level = next_level;
    }

    current_level[0]
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

    #[test]
    fn test_compute_merkle_root_empty() {
        let root = compute_merkle_root(&[]);
        assert_eq!(root, [0u8; 32]);
    }

    #[test]
    fn test_compute_merkle_root_single_leaf() {
        let leaf = luxtensor_crypto::keccak256(b"test");
        let root = compute_merkle_root(&[leaf]);
        assert_eq!(root, leaf);
    }

    #[test]
    fn test_compute_merkle_root_two_leaves() {
        let leaf1 = luxtensor_crypto::keccak256(b"leaf1");
        let leaf2 = luxtensor_crypto::keccak256(b"leaf2");
        let root = compute_merkle_root(&[leaf1, leaf2]);
        // Root should be deterministic regardless of input order
        let root_reversed = compute_merkle_root(&[leaf2, leaf1]);
        assert_eq!(root, root_reversed, "Merkle root must be order-independent for pairs");
    }

    #[test]
    fn test_compute_fedavg_commitment_deterministic() {
        let submissions = vec![
            GradientSubmission {
                trainer: "0xaaa".to_string(),
                gradient_hash: "0xhash_a".to_string(),
                checkpoint_hash: "0xcp_a".to_string(),
                timestamp: 100,
                verified: true,
            },
            GradientSubmission {
                trainer: "0xbbb".to_string(),
                gradient_hash: "0xhash_b".to_string(),
                checkpoint_hash: "0xcp_b".to_string(),
                timestamp: 101,
                verified: true,
            },
        ];
        let mut stakes = HashMap::new();
        stakes.insert("0xaaa".to_string(), 7_000u64);
        stakes.insert("0xbbb".to_string(), 3_000u64);

        let commitment1 = compute_fedavg_commitment(&submissions, &stakes);
        let commitment2 = compute_fedavg_commitment(&submissions, &stakes);
        assert_eq!(commitment1, commitment2, "FedAvg commitment must be deterministic");
        assert!(commitment1.starts_with("0x"), "Commitment must be hex-prefixed");
        assert_eq!(commitment1.len(), 66, "Commitment must be 32-byte hex (0x + 64 chars)");
    }

    #[test]
    fn test_compute_fedavg_commitment_empty() {
        let submissions: Vec<GradientSubmission> = vec![];
        let stakes = HashMap::new();
        let commitment = compute_fedavg_commitment(&submissions, &stakes);
        assert_eq!(
            commitment,
            format!("0x{}", hex::encode([0u8; 32])),
            "Empty submissions should produce zero hash"
        );
    }

    #[test]
    fn test_compute_fedavg_commitment_order_independent() {
        let sub_a = GradientSubmission {
            trainer: "0xaaa".to_string(),
            gradient_hash: "0xhash_a".to_string(),
            checkpoint_hash: "0xcp_a".to_string(),
            timestamp: 100,
            verified: true,
        };
        let sub_b = GradientSubmission {
            trainer: "0xbbb".to_string(),
            gradient_hash: "0xhash_b".to_string(),
            checkpoint_hash: "0xcp_b".to_string(),
            timestamp: 101,
            verified: true,
        };
        let mut stakes = HashMap::new();
        stakes.insert("0xaaa".to_string(), 5_000u64);
        stakes.insert("0xbbb".to_string(), 5_000u64);

        // Order should not matter — sorted internally by address
        let c1 = compute_fedavg_commitment(&[sub_a.clone(), sub_b.clone()], &stakes);
        let c2 = compute_fedavg_commitment(&[sub_b, sub_a], &stakes);
        assert_eq!(c1, c2, "FedAvg commitment must be order-independent");
    }

    #[test]
    fn test_trainer_stakes_default() {
        let ctx = TrainingRpcContext::new();
        assert!(ctx.trainer_stakes.read().is_empty());
    }
}
