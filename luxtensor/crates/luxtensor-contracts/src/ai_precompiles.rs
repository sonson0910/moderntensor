//! AI Precompiled Contract Handlers for LuxTensor
//!
//! This module implements custom precompiled contracts for native AI integration.
//! Precompile addresses range from 0x10 to 0x13.
//!
//! # Precompiles
//!
//! - `0x10` AI_REQUEST: Submit AI inference request
//! - `0x11` VERIFY_PROOF: Verify ZK proof for AI computation
//! - `0x12` GET_RESULT: Retrieve completed AI result
//! - `0x13` COMPUTE_PAYMENT: Calculate required payment for request

use crate::revm_integration::precompiles;
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

/// AI Precompile state manager
#[derive(Clone, Default)]
pub struct AIPrecompileState {
    requests: Arc<RwLock<HashMap<[u8; 32], AIRequestEntry>>>,
    request_counter: Arc<RwLock<u64>>,
}

impl AIPrecompileState {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(RwLock::new(HashMap::new())),
            request_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Generate unique request ID
    fn generate_request_id(&self, caller: &[u8; 20], model_hash: &[u8; 32]) -> [u8; 32] {
        let mut counter = self.request_counter.write().unwrap();
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
        let mut requests = self.requests.write().unwrap();
        requests.insert(entry.request_id, entry);
    }

    /// Get request by ID
    pub fn get_request(&self, request_id: &[u8; 32]) -> Option<AIRequestEntry> {
        let requests = self.requests.read().unwrap();
        requests.get(request_id).cloned()
    }

    /// Update request result
    pub fn fulfill_request(
        &self,
        request_id: &[u8; 32],
        fulfiller: [u8; 20],
        result: Vec<u8>,
    ) -> bool {
        let mut requests = self.requests.write().unwrap();
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
    let max_reward = u128::from_be_bytes(reward_bytes[16..32].try_into().unwrap());

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
        2 => true, // Dev mode - always valid for testing
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
    let input_size = u64::from_be_bytes(input_size_bytes[24..32].try_into().unwrap());

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

// ========== HELPER FUNCTIONS ==========

/// Placeholder for RISC Zero proof verification
fn verify_risc_zero_proof(_input: &Bytes) -> bool {
    // In production, this would:
    // 1. Parse the RISC Zero proof structure
    // 2. Call into the RISC Zero verifier
    // 3. Return verification result

    // For now, return true for development
    true
}

/// Placeholder for Groth16 proof verification
fn verify_groth16_proof(_input: &Bytes) -> bool {
    // In production, this would:
    // 1. Parse the Groth16 proof (A, B, C points)
    // 2. Call bn256 pairing precompile (0x08)
    // 3. Return verification result

    // For now, return true for development
    true
}

/// Check if address is an AI precompile
pub fn is_ai_precompile(address: &[u8; 20]) -> bool {
    *address == precompiles::AI_REQUEST ||
    *address == precompiles::VERIFY_PROOF ||
    *address == precompiles::GET_RESULT ||
    *address == precompiles::COMPUTE_PAYMENT
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
