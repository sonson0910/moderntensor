use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),

    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Crypto error: {0}")]
    CryptoError(#[from] luxtensor_crypto::CryptoError),

    #[error("Invalid vector dimension: expected {0}, got {1}")]
    InvalidVectorDimension(usize, usize),

    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Nonce overflow")]
    NonceOverflow,

    #[error("Balance overflow")]
    BalanceOverflow,

    /// FIX-4: HNSW deserialization failure (e.g. restore from RocksDB bytes).
    #[error("HNSW deserialization error: {0}")]
    HnswDeserialization(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
