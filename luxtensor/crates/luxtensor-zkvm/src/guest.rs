//! Guest program framework
//!
//! This module provides the framework for writing guest programs that run inside the zkVM.
//! Guest programs are compiled to RISC-V and executed with ZK proof generation.

use serde::{Deserialize, Serialize};

/// Trait for guest program inputs/outputs
pub trait GuestIO: Serialize + for<'de> Deserialize<'de> {}

// Blanket implementation for all types that satisfy the bounds
impl<T: Serialize + for<'de> Deserialize<'de>> GuestIO for T {}

/// Standard AI inference input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIInferenceInput {
    /// Model identifier (hash of model weights)
    pub model_id: [u8; 32],
    /// Input tensor (flattened)
    pub input_data: Vec<f32>,
    /// Input shape
    pub input_shape: Vec<usize>,
    /// Model configuration hash
    pub config_hash: [u8; 32],
}

/// Standard AI inference output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIInferenceOutput {
    /// Output tensor (flattened)
    pub output_data: Vec<f32>,
    /// Output shape
    pub output_shape: Vec<usize>,
    /// Execution metadata
    pub cycles_used: u64,
}

/// Weight update input for AI models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightUpdateInput {
    /// Subnet ID
    pub subnet_id: u16,
    /// Miner addresses
    pub miners: Vec<[u8; 20]>,
    /// New weights (normalized 0.0-1.0)
    pub weights: Vec<f64>,
    /// Previous weight hash
    pub prev_hash: [u8; 32],
    /// Validator signature (64 bytes)
    pub validator_sig: Vec<u8>,
}

/// Weight update output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightUpdateOutput {
    /// New weight hash
    pub new_hash: [u8; 32],
    /// Total weight sum (should be 1.0)
    pub weight_sum: f64,
    /// Validation passed
    pub is_valid: bool,
}

/// Generic computation input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeInput {
    /// Operation type
    pub operation: String,
    /// Input data bytes
    pub data: Vec<u8>,
    /// Optional parameters
    pub params: std::collections::HashMap<String, Vec<u8>>,
}

/// Generic computation output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeOutput {
    /// Result data bytes
    pub result: Vec<u8>,
    /// Status code
    pub status: u32,
    /// Error message (if any)
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_inference_input_serialization() {
        let input = AIInferenceInput {
            model_id: [1u8; 32],
            input_data: vec![1.0, 2.0, 3.0],
            input_shape: vec![1, 3],
            config_hash: [2u8; 32],
        };

        let bytes = bincode::serialize(&input).unwrap();
        let decoded: AIInferenceInput = bincode::deserialize(&bytes).unwrap();

        assert_eq!(decoded.model_id, input.model_id);
        assert_eq!(decoded.input_data, input.input_data);
    }

    #[test]
    fn test_weight_update_validation() {
        let input = WeightUpdateInput {
            subnet_id: 1,
            miners: vec![[1u8; 20], [2u8; 20]],
            weights: vec![0.5, 0.5],
            prev_hash: [0u8; 32],
            validator_sig: vec![0u8; 64],
        };

        let weight_sum: f64 = input.weights.iter().sum();
        assert!((weight_sum - 1.0).abs() < 0.001);
    }
}
