//! Indexer error types

use thiserror::Error;

/// Indexer error type
#[derive(Error, Debug)]
pub enum IndexerError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

/// Result type for indexer operations
pub type Result<T> = std::result::Result<T, IndexerError>;
