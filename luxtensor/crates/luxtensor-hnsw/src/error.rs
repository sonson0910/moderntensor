//! Error types for the HNSW module.

use thiserror::Error;

/// Result type alias for HNSW operations.
pub type Result<T> = std::result::Result<T, HnswError>;

/// Errors that can occur during HNSW operations.
#[derive(Error, Debug)]
pub enum HnswError {
    /// Vector dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Graph is empty (no nodes to search)
    #[error("Graph is empty, cannot perform search")]
    EmptyGraph,

    /// Invalid node ID
    #[error("Invalid node ID: {0}")]
    InvalidNodeId(usize),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// Fixed-point overflow during calculation
    #[error("Fixed-point arithmetic overflow")]
    FixedPointOverflow,

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

impl From<bincode::Error> for HnswError {
    fn from(err: bincode::Error) -> Self {
        HnswError::SerializationError(err.to_string())
    }
}
