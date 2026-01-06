use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Invalid block: {0}")]
    InvalidBlock(String),
    
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
    
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Crypto error: {0}")]
    CryptoError(#[from] luxtensor_crypto::CryptoError),
}

pub type Result<T> = std::result::Result<T, CoreError>;
