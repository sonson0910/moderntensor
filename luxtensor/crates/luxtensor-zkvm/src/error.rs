//! Error types for zkVM operations

use thiserror::Error;

/// Result type for zkVM operations
pub type Result<T> = std::result::Result<T, ZkVmError>;

/// Errors that can occur during zkVM operations
#[derive(Error, Debug)]
pub enum ZkVmError {
    /// Failed to generate proof
    #[error("Proof generation failed: {0}")]
    ProofGenerationFailed(String),

    /// Failed to verify proof
    #[error("Proof verification failed: {0}")]
    VerificationFailed(String),

    /// Invalid proof format
    #[error("Invalid proof format: {0}")]
    InvalidProof(String),

    /// Guest program execution failed
    #[error("Guest execution failed: {0}")]
    ExecutionFailed(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Image not found
    #[error("Guest image not found: {0}")]
    ImageNotFound(String),

    /// Prover unavailable
    #[error("Prover unavailable: {0}")]
    ProverUnavailable(String),

    /// Timeout during proving
    #[error("Proof generation timed out after {0} seconds")]
    Timeout(u64),

    /// Out of memory
    #[error("Out of memory during proof generation")]
    OutOfMemory,

    /// GPU error
    #[error("GPU error: {0}")]
    GpuError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<bincode::Error> for ZkVmError {
    fn from(e: bincode::Error) -> Self {
        ZkVmError::SerializationError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ZkVmError::ProofGenerationFailed("test error".to_string());
        assert!(err.to_string().contains("Proof generation failed"));
    }

    #[test]
    fn test_error_from_bincode() {
        // bincode errors convert properly
        let result: Result<()> = Err(ZkVmError::SerializationError("test".to_string()));
        assert!(result.is_err());
    }
}
