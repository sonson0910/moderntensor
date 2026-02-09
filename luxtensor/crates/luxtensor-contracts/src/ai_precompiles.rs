//! AI Precompiled Contract Handlers for LuxTensor
//!
//! This module implements custom precompiled contracts for native AI integration.
//! Precompile addresses range from 0x10 to 0x14.
//!
//! # Precompiles
//!
//! - `0x10` AI_REQUEST: Submit AI inference request
//! - `0x11` VERIFY_PROOF: Verify ZK proof for AI computation
//! - `0x12` GET_RESULT: Retrieve completed AI result
//! - `0x13` COMPUTE_PAYMENT: Calculate required payment for request
//! - `0x14` TRAIN_REQUEST: Submit federated learning training job

use crate::revm_integration::precompiles;
use luxtensor_core::hnsw::HnswVectorStore;
use luxtensor_core::semantic_registry::{SemanticRegistry, SemanticDomain, RegistryError};
use luxtensor_crypto::keccak256;
use revm::primitives::{Bytes, PrecompileOutput, PrecompileResult, PrecompileError};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Gas costs for AI precompiles
pub mod gas_costs {
    /// Base cost for AI_REQUEST
    pub const AI_REQUEST_BASE: u64 = 21_000;
    /// Per-byte cost for input data
    pub const AI_REQUEST_PER_BYTE: u64 = 16;

    /// Base cost for VERIFY_PROOF
    pub const VERIFY_PROOF_BASE: u64 = 50_000;
    /// Additional cost per proof byte
    pub const VERIFY_PROOF_PER_BYTE: u64 = 8;

    /// Cost for GET_RESULT
    pub const GET_RESULT: u64 = 3_000;

    /// Cost for COMPUTE_PAYMENT
    pub const COMPUTE_PAYMENT: u64 = 1_000;

    /// Base cost for TRAIN_REQUEST
    pub const TRAIN_REQUEST_BASE: u64 = 30_000;
    /// Per-byte cost for training config
    pub const TRAIN_REQUEST_PER_BYTE: u64 = 12;

    /// Base cost for VECTOR_STORE (HNSW insert with graph updates)
    pub const VECTOR_STORE_BASE: u64 = 80_000;
    /// Per-dimension cost for store
    pub const VECTOR_STORE_PER_DIM: u64 = 80;
    /// Graph update cost (HNSW neighbor connections)
    pub const VECTOR_STORE_GRAPH_UPDATE: u64 = 5_000;

    /// Base cost for VECTOR_QUERY (HNSW O(log N) search)
    pub const VECTOR_QUERY_BASE: u64 = 15_000;
    /// Per-dimension cost for query
    pub const VECTOR_QUERY_PER_DIM: u64 = 30;
    /// Cost per layer traversed (log N factor)
    pub const VECTOR_QUERY_PER_LAYER: u64 = 500;

    // ==================== AI Primitives (0x22-0x26) ====================

    /// CLASSIFY: Base cost (search + label lookup)
    pub const CLASSIFY_BASE: u64 = 25_000;
    /// Per-label cost (for label matching)
    pub const CLASSIFY_PER_LABEL: u64 = 100;

    /// ANOMALY_SCORE: Base cost (k-NN search + score calculation)
    pub const ANOMALY_SCORE_BASE: u64 = 30_000;

    /// SIMILARITY_GATE: Base cost (distance calculation)
    pub const SIMILARITY_GATE_BASE: u64 = 10_000;

    /// SEMANTIC_RELATE: Base cost (cross-contract vector lookup)
    pub const SEMANTIC_RELATE_BASE: u64 = 20_000;

    /// CLUSTER_ASSIGN: Base cost (find cluster centroid)
    pub const CLUSTER_ASSIGN_BASE: u64 = 28_000;

    // ==================== World Semantic Index (0x27-0x28) ====================

    /// REGISTER_VECTOR: Base cost for global registry registration
    pub const REGISTER_VECTOR_BASE: u64 = 35_000;
    /// Per-dimension cost for registration
    pub const REGISTER_VECTOR_PER_DIM: u64 = 50;
    /// Per-tag cost
    pub const REGISTER_VECTOR_PER_TAG: u64 = 200;

    /// GLOBAL_SEARCH: Base cost for cross-domain search
    pub const GLOBAL_SEARCH_BASE: u64 = 40_000;
    /// Per-domain cost (searches multiple shards)
    pub const GLOBAL_SEARCH_PER_DOMAIN: u64 = 5_000;
}

/// Stored AI request for precompile state
#[derive(Clone, Debug)]
pub struct AIRequestEntry {
    pub request_id: [u8; 32],
    pub model_hash: [u8; 32],
    pub input_hash: [u8; 32],
    pub callback_address: [u8; 20],
    pub max_reward: u128,
    pub status: RequestStatus,
    pub result: Vec<u8>,
    pub fulfiller: [u8; 20],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestStatus {
    Pending,
    Fulfilled,
    Expired,
    Cancelled,
}

/// Training job status
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingStatus {
    Open,           // Accepting participants
    Training,       // Active training round
    Aggregating,    // Aggregating gradients
    Completed,      // All rounds finished
    Cancelled,
}

/// Training job entry for federated learning
#[derive(Clone, Debug)]
pub struct TrainingJob {
    pub job_id: [u8; 32],
    pub model_id: [u8; 32],       // IPFS CID of base model
    pub dataset_ref: [u8; 32],    // IPFS reference to dataset
    pub total_rounds: u64,
    pub current_round: u64,
    pub min_participants: u64,
    pub reward_per_round: u128,
    pub creator: [u8; 20],
    pub status: TrainingStatus,
    pub participants: Vec<[u8; 20]>,
    pub gradient_hashes: Vec<[u8; 32]>,
}

/// AI Precompile state manager
#[derive(Clone, Default)]
pub struct AIPrecompileState {
    requests: Arc<RwLock<HashMap<[u8; 32], AIRequestEntry>>>,
    request_counter: Arc<RwLock<u64>>,
    /// Training jobs for federated learning
    training_jobs: Arc<RwLock<HashMap<[u8; 32], TrainingJob>>>,
    training_job_counter: Arc<RwLock<u64>>,

    /// Native Vector Store for Semantic Layer (0x20, 0x21)
    /// Uses HNSW index for O(log N) approximate nearest neighbor search
    vector_store: Arc<RwLock<HnswVectorStore>>,

    /// World Semantic Index - Global shared registry (0x27, 0x28)
    /// Supports domain sharding, quota management, and cross-contract composability
    pub semantic_registry: Arc<RwLock<SemanticRegistry>>,
}

impl AIPrecompileState {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            request_counter: Arc::new(RwLock::new(0)),
            training_jobs: Arc::new(RwLock::new(HashMap::new())),
            training_job_counter: Arc::new(RwLock::new(0)),
            // Default dimension 768 with HNSW index for O(log N) search
            vector_store: Arc::new(RwLock::new(HnswVectorStore::new(768))),
            // World Semantic Index with domain sharding
            semantic_registry: Arc::new(RwLock::new(SemanticRegistry::new(768))),
        }
    }

    /// Generate unique request ID
    fn generate_request_id(&self, caller: &[u8; 20], model_hash: &[u8; 32]) -> [u8; 32] {
        let mut counter = self.request_counter.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *counter += 1;

        // Concatenate inputs for hashing
        let mut data = Vec::with_capacity(20 + 32 + 8);
        data.extend_from_slice(caller);
        data.extend_from_slice(model_hash);
        data.extend_from_slice(&counter.to_be_bytes());

        keccak256(&data)
    }

    /// Store new request
    pub fn store_request(&self, entry: AIRequestEntry) {
        let mut requests = self.requests.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        requests.insert(entry.request_id, entry);
    }

    /// Get request by ID
    pub fn get_request(&self, request_id: &[u8; 32]) -> Option<AIRequestEntry> {
        let requests = self.requests.read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        requests.get(request_id).cloned()
    }

    /// Update request result
    pub fn fulfill_request(
        &self,
        request_id: &[u8; 32],
        fulfiller: [u8; 20],
        result: Vec<u8>,
    ) -> bool {
        let mut requests = self.requests.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = requests.get_mut(request_id) {
            if entry.status == RequestStatus::Pending {
                entry.status = RequestStatus::Fulfilled;
                entry.fulfiller = fulfiller;
                entry.result = result;
                return true;
            }
        }
        false
    }

    /// Generate unique training job ID
    fn generate_job_id(&self, creator: &[u8; 20], model_id: &[u8; 32]) -> [u8; 32] {
        let mut counter = self.training_job_counter.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *counter += 1;

        let mut data = Vec::with_capacity(20 + 32 + 8);
        data.extend_from_slice(creator);
        data.extend_from_slice(model_id);
        data.extend_from_slice(&counter.to_be_bytes());

        keccak256(&data)
    }

    /// Store new training job
    pub fn store_training_job(&self, job: TrainingJob) {
        let mut jobs = self.training_jobs.write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        jobs.insert(job.job_id, job);
    }

    /// Get training job by ID
    pub fn get_training_job(&self, job_id: &[u8; 32]) -> Option<TrainingJob> {
        let jobs = self.training_jobs.read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        jobs.get(job_id).cloned()
    }

    /// List active training jobs
    pub fn list_active_training_jobs(&self) -> Vec<TrainingJob> {
        let jobs = self.training_jobs.read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        jobs.values()
            .filter(|j| j.status == TrainingStatus::Open || j.status == TrainingStatus::Training)
            .cloned()
            .collect()
    }
}

/// AI_REQUEST precompile handler (0x10)
///
/// Input format: abi.encode(model_hash, input_data, callback_address, max_reward)
/// Output format: bytes32 request_id
pub fn ai_request_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
    caller: [u8; 20],
) -> PrecompileResult {
    // Calculate gas
    let gas_cost = gas_costs::AI_REQUEST_BASE +
        (input.len() as u64 * gas_costs::AI_REQUEST_PER_BYTE);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse input (simplified - requires min 116 bytes)
    // 32 bytes model_hash + 32 bytes input_data_hash + 20 bytes callback + 32 bytes reward
    if input.len() < 116 {
        return Err(PrecompileError::other("Invalid input length").into());
    }

    let mut model_hash = [0u8; 32];
    model_hash.copy_from_slice(&input[0..32]);

    let mut input_hash = [0u8; 32];
    input_hash.copy_from_slice(&input[32..64]);

    let mut callback_address = [0u8; 20];
    callback_address.copy_from_slice(&input[64..84]);

    // Parse reward (big-endian u128 from 32 bytes)
    let reward_bytes = &input[84..116];
    let max_reward = u128::from_be_bytes(
        reward_bytes[16..32]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid reward bytes"))?
    );

    // Generate request ID
    let request_id = state.generate_request_id(&caller, &model_hash);

    // Store request
    let entry = AIRequestEntry {
        request_id,
        model_hash,
        input_hash,
        callback_address,
        max_reward,
        status: RequestStatus::Pending,
        result: Vec::new(),
        fulfiller: [0u8; 20],
    };
    state.store_request(entry);

    // Return request_id
    Ok(PrecompileOutput::new(
        gas_cost,
        Bytes::copy_from_slice(&request_id),
    ))
}

/// VERIFY_PROOF precompile handler (0x11)
///
/// Input format: abi.encode(proof_type, proof_data, public_inputs)
/// Output format: bool is_valid (32 bytes, right-padded)
pub fn verify_proof_precompile(
    input: &Bytes,
    gas_limit: u64,
) -> PrecompileResult {
    let gas_cost = gas_costs::VERIFY_PROOF_BASE +
        (input.len() as u64 * gas_costs::VERIFY_PROOF_PER_BYTE);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse proof type (first 32 bytes)
    if input.len() < 32 {
        return Err(PrecompileError::other("Invalid input: missing proof type").into());
    }

    let proof_type = input[31]; // Last byte of first 32-byte word

    // For now, implement a simplified verification
    // In production, this would call into the actual ZK verification logic
    let is_valid = match proof_type {
        0 => verify_risc_zero_proof(input),
        1 => verify_groth16_proof(input),
        // NOTE: Dev-mode proof_type=2 (always-valid) was REMOVED for security.
        // All proofs must go through actual verification.
        _ => false,
    };

    // Encode result as 32-byte bool
    let mut result = [0u8; 32];
    if is_valid {
        result[31] = 1;
    }

    Ok(PrecompileOutput::new(
        gas_cost,
        Bytes::copy_from_slice(&result),
    ))
}

/// GET_RESULT precompile handler (0x12)
///
/// Input format: bytes32 request_id
/// Output format: abi.encode(status, result_data, fulfiller_address)
pub fn get_result_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    if gas_costs::GET_RESULT > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    if input.len() < 32 {
        return Err(PrecompileError::other("Invalid input: missing request_id").into());
    }

    let mut request_id = [0u8; 32];
    request_id.copy_from_slice(&input[0..32]);

    match state.get_request(&request_id) {
        Some(entry) => {
            // Encode: status (32) + result_offset (32) + fulfiller (32) + result_length (32) + result
            let status: u8 = match entry.status {
                RequestStatus::Pending => 0,
                RequestStatus::Fulfilled => 1,
                RequestStatus::Expired => 2,
                RequestStatus::Cancelled => 3,
            };

            let mut output = Vec::with_capacity(128 + entry.result.len());

            // Status (32 bytes)
            output.extend_from_slice(&[0u8; 31]);
            output.push(status);

            // Result data offset (points to dynamic data)
            output.extend_from_slice(&[0u8; 31]);
            output.push(96); // offset = 96 bytes

            // Fulfiller address (32 bytes, left-padded)
            output.extend_from_slice(&[0u8; 12]);
            output.extend_from_slice(&entry.fulfiller);

            // Result length
            let result_len = entry.result.len() as u32;
            output.extend_from_slice(&[0u8; 28]);
            output.extend_from_slice(&result_len.to_be_bytes());

            // Result data
            output.extend_from_slice(&entry.result);

            // Pad to 32-byte boundary
            while output.len() % 32 != 0 {
                output.push(0);
            }

            Ok(PrecompileOutput::new(
                gas_costs::GET_RESULT,
                Bytes::from(output),
            ))
        }
        None => {
            // Return empty result for non-existent request
            Ok(PrecompileOutput::new(
                gas_costs::GET_RESULT,
                Bytes::from(&[0u8; 96][..]),
            ))
        }
    }
}

/// COMPUTE_PAYMENT precompile handler (0x13)
///
/// Input format: bytes32 model_hash, uint256 input_size
/// Output format: uint256 required_payment
pub fn compute_payment_precompile(
    input: &Bytes,
    gas_limit: u64,
) -> PrecompileResult {
    if gas_costs::COMPUTE_PAYMENT > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    if input.len() < 64 {
        return Err(PrecompileError::other("Invalid input length").into());
    }

    // Parse input_size from second 32-byte word
    let input_size_bytes = &input[32..64];
    let input_size = u64::from_be_bytes(
        input_size_bytes[24..32]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid input size bytes"))?
    );

    // Simple pricing formula:
    // base_cost + (input_size * per_byte_cost)
    // Base: 0.01 MDT = 10^16 wei
    // Per-byte: 0.00001 MDT = 10^13 wei
    let base_cost: u128 = 10_000_000_000_000_000; // 0.01 MDT
    let per_byte_cost: u128 = 10_000_000_000_000; // 0.00001 MDT

    let total_cost = base_cost + (input_size as u128 * per_byte_cost);

    // Encode as uint256
    let mut output = [0u8; 32];
    output[16..32].copy_from_slice(&total_cost.to_be_bytes());

    Ok(PrecompileOutput::new(
        gas_costs::COMPUTE_PAYMENT,
        Bytes::copy_from_slice(&output),
    ))
}

/// TRAIN_REQUEST precompile handler (0x14)
///
/// Input format: abi.encode(model_id, dataset_ref, total_rounds, min_participants, reward_per_round)
/// Output format: bytes32 job_id
pub fn train_request_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
    caller: [u8; 20],
) -> PrecompileResult {
    // Calculate gas
    let gas_cost = gas_costs::TRAIN_REQUEST_BASE +
        (input.len() as u64 * gas_costs::TRAIN_REQUEST_PER_BYTE);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse input (min 128 bytes)
    // 32 bytes model_id + 32 bytes dataset_ref + 32 bytes total_rounds +
    // 32 bytes min_participants + 32 bytes reward_per_round = 160 bytes minimum
    if input.len() < 160 {
        return Err(PrecompileError::other("Invalid input length for training job").into());
    }

    let mut model_id = [0u8; 32];
    model_id.copy_from_slice(&input[0..32]);

    let mut dataset_ref = [0u8; 32];
    dataset_ref.copy_from_slice(&input[32..64]);

    // Parse total_rounds (uint256 -> u64)
    let total_rounds = u64::from_be_bytes(
        input[88..96]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid total_rounds bytes"))?
    );

    // Parse min_participants (uint256 -> u64)
    let min_participants = u64::from_be_bytes(
        input[120..128]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid min_participants bytes"))?
    );

    // Parse reward_per_round (uint256 -> u128)
    let reward_per_round = u128::from_be_bytes(
        input[144..160]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid reward_per_round bytes"))?
    );

    // Validate parameters
    if total_rounds == 0 || min_participants == 0 {
        return Err(PrecompileError::other("Invalid training parameters").into());
    }

    // Generate job ID
    let job_id = state.generate_job_id(&caller, &model_id);

    // Create and store training job
    let job = TrainingJob {
        job_id,
        model_id,
        dataset_ref,
        total_rounds,
        current_round: 0,
        min_participants,
        reward_per_round,
        creator: caller,
        status: TrainingStatus::Open,
        participants: Vec::new(),
        gradient_hashes: Vec::new(),
    };
    state.store_training_job(job);

    // Return job_id
    Ok(PrecompileOutput::new(
        gas_cost,
        Bytes::copy_from_slice(&job_id),
    ))
}

/// VECTOR_STORE precompile handler (0x20)
///
/// Input format: abi.encode(vector_id: uint64, vector_data: float32[])
/// Output format: bool success
pub fn vector_store_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    // 1. Basic parsing
    if input.len() < 32 {
        return Err(PrecompileError::other("Invalid input: missing vector ID").into());
    }

    // Parse vector ID (uint64 from first 32 bytes)
    let vector_id = u64::from_be_bytes(
        input[24..32]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid vector ID bytes"))?
    );

    // Parse vector data (offset-based parsing simplified for prototype)
    // Assume input format: [ID: 32 bytes] [Offset: 32 bytes] [Length: 32 bytes] [Data: 4*N bytes]
    if input.len() < 96 {
        return Err(PrecompileError::other("Invalid input structure").into());
    }

    let length_word = &input[64..96];
    let vector_len = u32::from_be_bytes(
        length_word[28..32]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid length bytes"))?
    ) as usize;

    let float_data_start = 96;
    let expected_size = float_data_start + (vector_len * 4);

    if input.len() < expected_size {
        return Err(PrecompileError::other("Input too short for vector data").into());
    }

    // 2. Calculate Gas (HNSW insert with graph structure updates)
    let gas_cost = gas_costs::VECTOR_STORE_BASE +
                   (vector_len as u64 * gas_costs::VECTOR_STORE_PER_DIM) +
                   gas_costs::VECTOR_STORE_GRAPH_UPDATE;

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // 3. Decode floats (IEEE 754)
    let mut vector = Vec::with_capacity(vector_len);
    for i in 0..vector_len {
        let start = float_data_start + (i * 4);
        let bytes: [u8; 4] = input[start..start+4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        // Use from_be_bytes because EVM is big-endian, but standard floats are typically LE in Rust/x86
        // We assume ABI encoding puts bytes in standard network order (BE)
        let val = f32::from_bits(u32::from_be_bytes(bytes));
        vector.push(val);
    }

    // 4. Store Vector (with RwLock poisoning handling)
    {
        let mut store = state.vector_store.write()
            .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;
        store.insert(vector_id, vector)
            .map_err(|_| PrecompileError::other("Vector store error"))?;
    }

    // 5. Return true
    let mut result = [0u8; 32];
    result[31] = 1;

    Ok(PrecompileOutput::new(
        gas_cost,
        Bytes::copy_from_slice(&result),
    ))
}

/// VECTOR_QUERY precompile handler (0x21)
///
/// Input format: abi.encode(k: uint64, query_vector: float32[])
/// Output format: (uint64[], float32[]) - IDs and scores
///
/// Maximum k is capped at 100 to prevent DoS attacks.
pub fn vector_query_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    // Maximum k to prevent DoS
    const MAX_K: usize = 100;

    // 1. Basic parsing
    if input.len() < 32 {
        return Err(PrecompileError::other("Invalid input: missing k").into());
    }

    let raw_k = u64::from_be_bytes(
        input[24..32]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid k bytes"))?
    ) as usize;

    // Cap k to prevent DoS attacks
    let k = raw_k.min(MAX_K);

    // Parse vector (similar to store)
    // [K: 32 bytes] [Offset: 32 bytes] [Length: 32 bytes] [Data...]
    if input.len() < 96 {
        return Err(PrecompileError::other("Invalid input structure").into());
    }

    let length_word = &input[64..96];
    let vector_len = u32::from_be_bytes(
        length_word[28..32]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid length bytes"))?
    ) as usize;

    // 2. Calculate Gas (HNSW O(log N) search cost)
    // Cost = base + per_dim * dimensions + per_layer * estimated_layers
    let estimated_layers = {
        let store = state.vector_store.read()
            .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;
        let n = store.len();
        if n <= 1 { 1 } else { ((n as f64).ln() / 16_f64.ln()).ceil() as u64 }
    };

    let gas_cost = gas_costs::VECTOR_QUERY_BASE +
                   (vector_len as u64 * gas_costs::VECTOR_QUERY_PER_DIM) +
                   (estimated_layers * gas_costs::VECTOR_QUERY_PER_LAYER);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // 3. Decode query vector
    let float_data_start = 96;
    if input.len() < float_data_start + (vector_len * 4) {
        return Err(PrecompileError::other("Input too short").into());
    }

    let mut query = Vec::with_capacity(vector_len);
    for i in 0..vector_len {
        let start = float_data_start + (i * 4);
        let bytes: [u8; 4] = input[start..start+4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        let val = f32::from_bits(u32::from_be_bytes(bytes));
        query.push(val);
    }

    // 4. Perform Search (with RwLock poisoning handling)
    let results = {
        let store = state.vector_store.read()
            .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;
        store.search(&query, k)
            .map_err(|_| PrecompileError::other("Vector search error"))?
    };

    // 5. Encode Output with both IDs and Scores
    // ABI format: (uint64[] ids, uint256[] scores)
    // Struct of two dynamic arrays:
    // [Offset to ids: 32] [Offset to scores: 32]
    // [Len ids: 32] [ids padded to 32 each...]
    // [Len scores: 32] [scores as uint256 (fixed-point) ...]

    let res_len = results.len() as u64;
    let mut output = Vec::new();

    // Offset to first array (ids) = 64 (after the two offset words)
    output.extend_from_slice(&[0u8; 31]);
    output.push(64);

    // Offset to second array (scores) = 64 + 32 + (res_len * 32)
    // = 96 + res_len * 32
    let scores_offset = 64u64 + 32 + (res_len * 32);
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&scores_offset.to_be_bytes());

    // First array: ids
    // Length
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&res_len.to_be_bytes());

    // IDs (padded to 32 bytes each for uint64)
    let mut scores_vec = Vec::with_capacity(results.len());
    for (id, score) in &results {
        output.extend_from_slice(&[0u8; 24]);
        output.extend_from_slice(&id.to_be_bytes());
        scores_vec.push(*score);
    }

    // Second array: scores
    // Length
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&res_len.to_be_bytes());

    // Scores as fixed-point uint256 (score * 1e18 for precision)
    // This converts f32 distance to a scaled integer representation
    for score in scores_vec {
        // Convert f32 score to fixed-point (18 decimals)
        // Score is typically a distance (lower = better), so we keep as-is
        let scaled_score = (score as f64 * 1e18) as u128;
        let mut score_bytes = [0u8; 32];
        score_bytes[16..32].copy_from_slice(&scaled_score.to_be_bytes());
        output.extend_from_slice(&score_bytes);
    }

    Ok(PrecompileOutput::new(
        gas_cost,
        Bytes::from(output),
    ))
}


// ==================== AI PRIMITIVES (0x22 - 0x26) ====================

/// CLASSIFY precompile handler (0x22)
///
/// Classifies a vector against labeled reference vectors using k-NN.
/// Input format: abi.encode(query_vector: float32[], labels: (uint64, uint32)[])
/// Output format: (uint32 label, uint256 confidence)
///
/// # Use Cases
/// - Sentiment classification for content moderation
/// - Category assignment for marketplace items
/// - Risk level classification for DeFi positions
pub fn classify_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    // Parse input structure:
    // [vector_offset: 32] [labels_offset: 32]
    // [vector_len: 32] [vector_data...]
    // [labels_len: 32] [labels_data...] where each label is (uint64 id, uint32 label_id)

    if input.len() < 64 {
        return Err(PrecompileError::other("Invalid classify input").into());
    }

    // Parse vector offset and length
    let vector_offset = u32::from_be_bytes(
        input[28..32].try_into().map_err(|_| PrecompileError::other("Invalid offset"))?
    ) as usize;

    if input.len() < vector_offset + 32 {
        return Err(PrecompileError::other("Invalid vector offset").into());
    }

    let vector_len = u32::from_be_bytes(
        input[vector_offset + 28..vector_offset + 32].try_into()
            .map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    // Parse labels offset and count
    let labels_offset = u32::from_be_bytes(
        input[60..64].try_into().map_err(|_| PrecompileError::other("Invalid offset"))?
    ) as usize;

    if input.len() < labels_offset + 32 {
        return Err(PrecompileError::other("Invalid labels offset").into());
    }

    let labels_len = u32::from_be_bytes(
        input[labels_offset + 28..labels_offset + 32].try_into()
            .map_err(|_| PrecompileError::other("Invalid labels length"))?
    ) as usize;

    // Calculate gas
    let gas_cost = gas_costs::CLASSIFY_BASE +
        (vector_len as u64 * gas_costs::VECTOR_QUERY_PER_DIM) +
        (labels_len as u64 * gas_costs::CLASSIFY_PER_LABEL);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse query vector
    let vector_data_start = vector_offset + 32;
    if input.len() < vector_data_start + (vector_len * 4) {
        return Err(PrecompileError::other("Input too short for vector").into());
    }

    let mut query = Vec::with_capacity(vector_len);
    for i in 0..vector_len {
        let start = vector_data_start + (i * 4);
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        query.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    // Parse labels (each is 12 bytes: 8-byte id + 4-byte label)
    let labels_data_start = labels_offset + 32;
    let mut labels = Vec::with_capacity(labels_len);
    for i in 0..labels_len {
        let start = labels_data_start + (i * 32); // ABI-encoded as 32-byte chunks
        if input.len() < start + 32 {
            break;
        }
        let id = u64::from_be_bytes(
            input[start + 24..start + 32].try_into()
                .map_err(|_| PrecompileError::other("Invalid label id"))?
        );
        // Label is in next 32-byte word
        let label_start = labels_data_start + ((labels_len + i) * 32);
        if input.len() < label_start + 32 {
            continue;
        }
        let label = u32::from_be_bytes(
            input[label_start + 28..label_start + 32].try_into()
                .map_err(|_| PrecompileError::other("Invalid label value"))?
        );
        labels.push((id, label));
    }

    // Perform classification
    let store = state.vector_store.read()
        .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;

    let (label, confidence) = store.classify(&query, &labels)
        .map_err(|_| PrecompileError::other("Classification failed"))?;

    // Encode output: (uint32 label, uint256 confidence)
    let mut output = vec![0u8; 64];
    output[28..32].copy_from_slice(&label.to_be_bytes());

    // Confidence as fixed-point (18 decimals)
    let scaled_confidence = (confidence as f64 * 1e18) as u128;
    output[48..64].copy_from_slice(&scaled_confidence.to_be_bytes());

    Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
}

/// CLUSTER_ASSIGN precompile handler (0x23)
///
/// Assigns a vector to the nearest cluster centroid.
/// Input format: abi.encode(query_vector: float32[], centroid_ids: uint64[])
/// Output format: (uint64 cluster_id, uint256 distance)
///
/// # Use Cases
/// - User segmentation for targeted rewards
/// - Geographic grouping for logistics
/// - Content categorization
pub fn cluster_assign_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    if input.len() < 64 {
        return Err(PrecompileError::other("Invalid cluster input").into());
    }

    // Parse query vector (simplified - assume vector starts at offset 64)
    let vector_len = u32::from_be_bytes(
        input[60..64].try_into().map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    // Calculate gas
    let gas_cost = gas_costs::CLUSTER_ASSIGN_BASE +
        (vector_len as u64 * gas_costs::VECTOR_QUERY_PER_DIM);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse vector
    let vector_start = 64;
    if input.len() < vector_start + (vector_len * 4) {
        return Err(PrecompileError::other("Input too short").into());
    }

    let mut query = Vec::with_capacity(vector_len);
    for i in 0..vector_len {
        let start = vector_start + (i * 4);
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        query.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    // Find nearest centroid using HNSW search
    let store = state.vector_store.read()
        .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;

    let results = store.search(&query, 1)
        .map_err(|_| PrecompileError::other("Search failed"))?;

    let (cluster_id, distance) = results.first()
        .map(|(id, dist)| (*id, *dist))
        .unwrap_or((0, f32::MAX));

    // Encode output: (uint64 cluster_id, uint256 distance)
    let mut output = vec![0u8; 64];
    output[24..32].copy_from_slice(&cluster_id.to_be_bytes());

    let scaled_distance = (distance as f64 * 1e18) as u128;
    output[48..64].copy_from_slice(&scaled_distance.to_be_bytes());

    Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
}

/// ANOMALY_SCORE precompile handler (0x24)
///
/// Calculates anomaly score for a vector (how different from stored vectors).
/// Input format: abi.encode(query_vector: float32[])
/// Output format: uint256 anomaly_score (0 = normal, 1e18 = highly anomalous)
///
/// # Use Cases
/// - Fraud detection in transactions
/// - Bot detection for governance
/// - Unusual activity monitoring
pub fn anomaly_score_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    // Parse vector length and data
    if input.len() < 64 {
        return Err(PrecompileError::other("Invalid anomaly input").into());
    }

    let vector_len = u32::from_be_bytes(
        input[60..64].try_into().map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    // Calculate gas
    let gas_cost = gas_costs::ANOMALY_SCORE_BASE +
        (vector_len as u64 * gas_costs::VECTOR_QUERY_PER_DIM);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse vector
    let vector_start = 64;
    if input.len() < vector_start + (vector_len * 4) {
        return Err(PrecompileError::other("Input too short").into());
    }

    let mut query = Vec::with_capacity(vector_len);
    for i in 0..vector_len {
        let start = vector_start + (i * 4);
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        query.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    // Calculate anomaly score
    let store = state.vector_store.read()
        .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;

    let score = store.anomaly_score(&query)
        .map_err(|_| PrecompileError::other("Anomaly calculation failed"))?;

    // Encode output: uint256 anomaly_score
    let mut output = vec![0u8; 32];
    let scaled_score = (score as f64 * 1e18) as u128;
    output[16..32].copy_from_slice(&scaled_score.to_be_bytes());

    Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
}

/// SIMILARITY_GATE precompile handler (0x25)
///
/// Checks if two vectors are similar above a threshold (gating mechanism).
/// Input format: abi.encode(vector_a: float32[], vector_b: float32[], threshold: uint256)
/// Output format: (bool is_similar, uint256 similarity)
///
/// # Use Cases
/// - Access control based on semantic similarity
/// - Content matching for duplicate detection
/// - Identity verification
pub fn similarity_gate_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    // Minimum input: 3 offsets (96 bytes) + 2 vectors + threshold
    if input.len() < 96 {
        return Err(PrecompileError::other("Invalid similarity input").into());
    }

    // Parse offsets
    let vec_a_offset = u32::from_be_bytes(
        input[28..32].try_into().map_err(|_| PrecompileError::other("Invalid offset"))?
    ) as usize;

    let vec_b_offset = u32::from_be_bytes(
        input[60..64].try_into().map_err(|_| PrecompileError::other("Invalid offset"))?
    ) as usize;

    // Threshold is at offset 64-96 (direct value, not offset)
    let threshold_bytes = &input[80..96];
    let threshold_scaled = u128::from_be_bytes(
        threshold_bytes.try_into().map_err(|_| PrecompileError::other("Invalid threshold"))?
    );
    let threshold = (threshold_scaled as f64 / 1e18) as f32;

    // Parse vector lengths
    if input.len() < vec_a_offset + 32 || input.len() < vec_b_offset + 32 {
        return Err(PrecompileError::other("Invalid vector offsets").into());
    }

    let vec_a_len = u32::from_be_bytes(
        input[vec_a_offset + 28..vec_a_offset + 32].try_into()
            .map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    let vec_b_len = u32::from_be_bytes(
        input[vec_b_offset + 28..vec_b_offset + 32].try_into()
            .map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    // Calculate gas
    let gas_cost = gas_costs::SIMILARITY_GATE_BASE +
        ((vec_a_len + vec_b_len) as u64 * gas_costs::VECTOR_QUERY_PER_DIM / 2);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse vectors
    let mut vec_a = Vec::with_capacity(vec_a_len);
    let vec_a_data_start = vec_a_offset + 32;
    for i in 0..vec_a_len {
        let start = vec_a_data_start + (i * 4);
        if start + 4 > input.len() { break; }
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        vec_a.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    let mut vec_b = Vec::with_capacity(vec_b_len);
    let vec_b_data_start = vec_b_offset + 32;
    for i in 0..vec_b_len {
        let start = vec_b_data_start + (i * 4);
        if start + 4 > input.len() { break; }
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        vec_b.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    // Check similarity
    let store = state.vector_store.read()
        .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;

    let (is_similar, similarity) = store.similarity_check(&vec_a, &vec_b, threshold)
        .map_err(|_| PrecompileError::other("Similarity check failed"))?;

    // Encode output: (bool is_similar, uint256 similarity)
    let mut output = vec![0u8; 64];
    output[31] = if is_similar { 1 } else { 0 };

    let scaled_similarity = (similarity as f64 * 1e18) as u128;
    output[48..64].copy_from_slice(&scaled_similarity.to_be_bytes());

    Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
}

/// SEMANTIC_RELATE precompile handler (0x26)
///
/// Retrieves a stored vector for cross-contract composability.
/// Input format: abi.encode(vector_id: uint64)
/// Output format: (bool exists, float32[] vector)
///
/// # Use Cases
/// - Share embeddings between contracts
/// - Build composable AI applications
/// - Create semantic graphs across contracts
pub fn semantic_relate_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    if input.len() < 32 {
        return Err(PrecompileError::other("Invalid relate input").into());
    }

    // Parse vector ID
    let vector_id = u64::from_be_bytes(
        input[24..32].try_into().map_err(|_| PrecompileError::other("Invalid id"))?
    );

    // Calculate gas
    let gas_cost = gas_costs::SEMANTIC_RELATE_BASE;

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Lookup vector
    let store = state.vector_store.read()
        .map_err(|_| PrecompileError::other("Vector store lock poisoned"))?;

    let vector = store.get_vector(vector_id);

    // Encode output
    match vector {
        Some(vec) => {
            // (bool exists = true, dynamic array offset, array)
            let mut output = Vec::new();

            // exists = true (32 bytes)
            output.extend_from_slice(&[0u8; 31]);
            output.push(1);

            // Offset to array = 64
            output.extend_from_slice(&[0u8; 31]);
            output.push(64);

            // Array length
            output.extend_from_slice(&[0u8; 24]);
            output.extend_from_slice(&(vec.len() as u64).to_be_bytes());

            // Array data
            for val in vec {
                output.extend_from_slice(&u32::to_be_bytes(val.to_bits()));
            }

            Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
        }
        None => {
            // exists = false
            let mut output = vec![0u8; 64];
            output[31] = 0; // exists = false
            output[63] = 64; // offset (empty array)
            Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
        }
    }
}

// ==================== World Semantic Index Precompiles (0x27-0x28) ====================

/// REGISTER_VECTOR precompile handler (0x27)
///
/// Registers a vector in the global semantic registry with domain sharding.
/// Input format: abi.encode(domain: uint8, vector: float32[], tags: bytes32[], ttl: uint64)
/// Output format: (uint64 global_id, uint256 remaining_quota)
///
/// # Use Cases
/// - Register DeFi risk profiles for cross-protocol sharing
/// - Publish identity embeddings for universal access control
/// - Store gaming assets with semantic properties
pub fn register_vector_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
    caller: &[u8; 20],
    block_number: u64,
) -> PrecompileResult {
    if input.len() < 128 {
        return Err(PrecompileError::other("Invalid register input").into());
    }

    // Parse domain (first 32 bytes, only last byte matters)
    let domain_byte = input[31];
    let domain = SemanticDomain::from(domain_byte);

    // Parse vector offset
    let vector_offset = u32::from_be_bytes(
        input[60..64].try_into().map_err(|_| PrecompileError::other("Invalid offset"))?
    ) as usize;

    if input.len() < vector_offset + 32 {
        return Err(PrecompileError::other("Invalid vector offset").into());
    }

    let vector_len = u32::from_be_bytes(
        input[vector_offset + 28..vector_offset + 32].try_into()
            .map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    // Limit vector dimension for DoS protection
    if vector_len > 2048 {
        return Err(PrecompileError::other("Vector dimension too large").into());
    }

    // Parse tags offset and count
    let tags_offset = u32::from_be_bytes(
        input[92..96].try_into().map_err(|_| PrecompileError::other("Invalid tags offset"))?
    ) as usize;

    let tags_len = if input.len() >= tags_offset + 32 {
        u32::from_be_bytes(
            input[tags_offset + 28..tags_offset + 32].try_into().unwrap_or([0; 4])
        ) as usize
    } else {
        0
    };

    // Limit tags for DoS protection
    let tags_len = tags_len.min(10);

    // Parse TTL (at byte 96-128)
    let ttl = if input.len() >= 128 {
        u64::from_be_bytes(
            input[120..128].try_into().unwrap_or([0; 8])
        )
    } else {
        0 // Permanent
    };

    // Calculate gas
    let gas_cost = gas_costs::REGISTER_VECTOR_BASE +
        (vector_len as u64 * gas_costs::REGISTER_VECTOR_PER_DIM) +
        (tags_len as u64 * gas_costs::REGISTER_VECTOR_PER_TAG);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse vector data
    let vector_data_start = vector_offset + 32;
    if input.len() < vector_data_start + (vector_len * 4) {
        return Err(PrecompileError::other("Input too short for vector").into());
    }

    let mut vector = Vec::with_capacity(vector_len);
    for i in 0..vector_len {
        let start = vector_data_start + (i * 4);
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        vector.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    // Parse tags
    let mut tags = Vec::with_capacity(tags_len);
    let tags_data_start = tags_offset + 32;
    for i in 0..tags_len {
        let start = tags_data_start + (i * 32);
        if input.len() < start + 32 {
            break;
        }
        let mut tag = [0u8; 32];
        tag.copy_from_slice(&input[start..start + 32]);
        tags.push(tag);
    }

    // Register in global registry
    let mut registry = state.semantic_registry.write()
        .map_err(|_| PrecompileError::other("Registry lock poisoned"))?;

    let global_id = registry.register(
        *caller,
        domain,
        vector,
        tags,
        block_number,
        ttl,
    ).map_err(|e| match e {
        RegistryError::QuotaExceeded => PrecompileError::other("Storage quota exceeded"),
        RegistryError::DimensionMismatch { .. } => PrecompileError::other("Dimension mismatch"),
        _ => PrecompileError::other("Registration failed"),
    })?;

    // Get remaining quota
    let remaining = registry.get_remaining_quota(caller)
        .unwrap_or(0);

    // Encode output: (uint64 global_id, uint256 remaining_quota)
    let mut output = vec![0u8; 64];
    output[24..32].copy_from_slice(&global_id.to_be_bytes());
    output[48..64].copy_from_slice(&(remaining as u128).to_be_bytes());

    Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
}

/// GLOBAL_SEARCH precompile handler (0x28)
///
/// Searches across all domains in the World Semantic Index.
/// Input format: abi.encode(query: float32[], k: uint64, domains: uint8[])
/// Output format: (uint64[] ids, uint256[] scores, uint8[] domains)
///
/// # Use Cases
/// - Cross-domain semantic discovery
/// - Universal similarity matching
/// - Multi-protocol aggregation
pub fn global_search_precompile(
    input: &Bytes,
    gas_limit: u64,
    state: &AIPrecompileState,
) -> PrecompileResult {
    if input.len() < 64 {
        return Err(PrecompileError::other("Invalid search input").into());
    }

    // Parse query vector offset
    let query_offset = u32::from_be_bytes(
        input[28..32].try_into().map_err(|_| PrecompileError::other("Invalid offset"))?
    ) as usize;

    if input.len() < query_offset + 32 {
        return Err(PrecompileError::other("Invalid query offset").into());
    }

    let query_len = u32::from_be_bytes(
        input[query_offset + 28..query_offset + 32].try_into()
            .map_err(|_| PrecompileError::other("Invalid length"))?
    ) as usize;

    // Parse k
    let k = u64::from_be_bytes(
        input[56..64].try_into().map_err(|_| PrecompileError::other("Invalid k"))?
    ) as usize;

    // Limit k for DoS protection
    let k = k.min(100);

    // Count domains to search (0 = all domains)
    let num_domains = 4; // Default: search 4 common domains

    // Calculate gas
    let gas_cost = gas_costs::GLOBAL_SEARCH_BASE +
        (query_len as u64 * gas_costs::VECTOR_QUERY_PER_DIM) +
        (num_domains as u64 * gas_costs::GLOBAL_SEARCH_PER_DOMAIN);

    if gas_cost > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    // Parse query vector
    let query_data_start = query_offset + 32;
    if input.len() < query_data_start + (query_len * 4) {
        return Err(PrecompileError::other("Input too short for query").into());
    }

    let mut query = Vec::with_capacity(query_len);
    for i in 0..query_len {
        let start = query_data_start + (i * 4);
        let bytes: [u8; 4] = input[start..start + 4]
            .try_into()
            .map_err(|_| PrecompileError::other("Invalid float bytes"))?;
        query.push(f32::from_bits(u32::from_be_bytes(bytes)));
    }

    // Perform global search
    let registry = state.semantic_registry.read()
        .map_err(|_| PrecompileError::other("Registry lock poisoned"))?;

    let results = registry.search_global(&query, k)
        .map_err(|_| PrecompileError::other("Global search failed"))?;

    // Encode output: (uint64[] ids, uint256[] scores, uint8[] domains)
    // Dynamic array encoding
    let mut output = Vec::new();

    // Offsets for three dynamic arrays
    // ids offset = 96 (3 * 32)
    output.extend_from_slice(&[0u8; 31]); output.push(96);
    // scores offset = 96 + 32 + (results.len() * 32)
    let scores_offset = 96 + 32 + (results.len() * 32);
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&(scores_offset as u64).to_be_bytes());
    // domains offset
    let domains_offset = scores_offset + 32 + (results.len() * 32);
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&(domains_offset as u64).to_be_bytes());

    // ids array
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&(results.len() as u64).to_be_bytes());
    for (id, _, _) in &results {
        output.extend_from_slice(&[0u8; 24]);
        output.extend_from_slice(&id.to_be_bytes());
    }

    // scores array
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&(results.len() as u64).to_be_bytes());
    for (_, score, _) in &results {
        let scaled = (*score as f64 * 1e18) as u128;
        output.extend_from_slice(&[0u8; 16]);
        output.extend_from_slice(&scaled.to_be_bytes());
    }

    // domains array
    output.extend_from_slice(&[0u8; 24]);
    output.extend_from_slice(&(results.len() as u64).to_be_bytes());
    for (_, _, domain) in &results {
        output.extend_from_slice(&[0u8; 31]);
        output.push(*domain as u8);
    }

    Ok(PrecompileOutput::new(gas_cost, Bytes::from(output)))
}


// ========== HELPER FUNCTIONS ==========


/// RISC Zero proof verification — SECURITY: rejects until real verifier integrated
///
/// In production, this would:
/// 1. Parse the RISC Zero proof structure
/// 2. Call into the RISC Zero verifier
/// 3. Return verification result
///
/// SECURITY: Returning `true` without cryptographic verification would allow
/// any caller to claim verified AI computation without actually running it.
fn verify_risc_zero_proof(_input: &Bytes) -> bool {
    tracing::warn!(
        "RISC Zero proof verification: no verifier backend configured — proof REJECTED. \
         Enable the `risc0` feature and provide a valid verifier to accept proofs."
    );
    false
}

/// Groth16 proof verification — SECURITY: rejects until real verifier integrated
///
/// In production, this would:
/// 1. Parse the Groth16 proof (A, B, C points)
/// 2. Call bn256 pairing precompile (0x08)
/// 3. Return verification result
///
/// SECURITY: Returning `true` without cryptographic verification would allow
/// fake proofs to pass on-chain verification.
fn verify_groth16_proof(_input: &Bytes) -> bool {
    tracing::warn!(
        "Groth16 proof verification: no verifier backend configured — proof REJECTED. \
         Enable the `groth16` feature and provide a valid verifier to accept proofs."
    );
    false
}

/// Check if address is an AI precompile
pub fn is_ai_precompile(address: &[u8; 20]) -> bool {
    *address == precompiles::AI_REQUEST ||
    *address == precompiles::VERIFY_PROOF ||
    *address == precompiles::GET_RESULT ||
    *address == precompiles::COMPUTE_PAYMENT ||
    is_training_precompile(address) ||
    is_semantic_precompile(address) ||
    is_ai_primitives_precompile(address) ||
    is_registry_precompile(address)
}

/// Check if address is a training precompile
pub fn is_training_precompile(address: &[u8; 20]) -> bool {
    // TRAIN_REQUEST at 0x14
    let train_request_addr = [0u8; 20];
    let mut expected = train_request_addr;
    expected[19] = 0x14;
    *address == expected
}

/// Check if address is a semantic precompile (0x20 - 0x21)
pub fn is_semantic_precompile(address: &[u8; 20]) -> bool {
    let base = [0u8; 20];
    let mut expected_store = base; expected_store[19] = 0x20;
    let mut expected_query = base; expected_query[19] = 0x21;

    *address == expected_store || *address == expected_query
}

/// Check if address is an AI Primitives precompile (0x22 - 0x26)
pub fn is_ai_primitives_precompile(address: &[u8; 20]) -> bool {
    if address[0..19] != [0u8; 19] {
        return false;
    }
    let last_byte = address[19];
    (0x22..=0x26).contains(&last_byte)
}

/// Check if address is a World Semantic Registry precompile (0x27 - 0x28)
pub fn is_registry_precompile(address: &[u8; 20]) -> bool {
    if address[0..19] != [0u8; 19] {
        return false;
    }
    let last_byte = address[19];
    (0x27..=0x28).contains(&last_byte)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> AIPrecompileState {
        AIPrecompileState::new()
    }

    #[test]
    fn test_generate_request_id() {
        let state = create_test_state();
        let caller = [1u8; 20];
        let model_hash = [2u8; 32];

        let id1 = state.generate_request_id(&caller, &model_hash);
        let id2 = state.generate_request_id(&caller, &model_hash);

        // IDs should be unique
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_ai_request_precompile() {
        let state = create_test_state();
        let caller = [1u8; 20];

        // Create valid input: model_hash(32) + input_hash(32) + callback(20) + reward(32)
        let mut input = vec![0u8; 116];
        input[0..32].copy_from_slice(&[0xABu8; 32]); // model_hash
        input[32..64].copy_from_slice(&[0xCDu8; 32]); // input_hash
        input[64..84].copy_from_slice(&[0xEFu8; 20]); // callback
        input[100..116].copy_from_slice(&100u128.to_be_bytes()); // reward

        let result = ai_request_precompile(
            &Bytes::from(input),
            100_000,
            &state,
            caller,
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.bytes.len(), 32); // request_id
    }

    #[test]
    fn test_compute_payment() {
        // model_hash(32) + input_size(32) where input_size = 1000
        let mut input = vec![0u8; 64];
        input[56..64].copy_from_slice(&1000u64.to_be_bytes());

        let result = compute_payment_precompile(
            &Bytes::from(input),
            10_000,
        );

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.bytes.len(), 32);

        // Expected: 0.01 + (1000 * 0.00001) = 0.02 MDT = 2*10^16
        let expected: u128 = 20_000_000_000_000_000;
        let actual = u128::from_be_bytes(output.bytes[16..32].try_into().unwrap());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_is_ai_precompile() {
        assert!(is_ai_precompile(&precompiles::AI_REQUEST));
        assert!(is_ai_precompile(&precompiles::VERIFY_PROOF));
        assert!(is_ai_precompile(&precompiles::GET_RESULT));
        assert!(is_ai_precompile(&precompiles::COMPUTE_PAYMENT));

        // Standard precompiles should return false
        assert!(!is_ai_precompile(&precompiles::ECRECOVER));
        assert!(!is_ai_precompile(&precompiles::SHA256));
    }
}
