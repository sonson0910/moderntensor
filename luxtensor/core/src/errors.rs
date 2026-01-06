use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Insufficient balance")]
    InsufficientBalance,
    
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    
    #[error("Gas limit exceeded")]
    GasLimitExceeded,
    
    #[error("State error: {0}")]
    StateError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Cryptography error: {0}")]
    CryptoError(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
