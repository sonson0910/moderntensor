use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
    
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Server error: {0}")]
    ServerError(String),
}

impl From<luxtensor_storage::StorageError> for RpcError {
    fn from(err: luxtensor_storage::StorageError) -> Self {
        RpcError::StorageError(err.to_string())
    }
}

impl From<serde_json::Error> for RpcError {
    fn from(err: serde_json::Error) -> Self {
        RpcError::ParseError(err.to_string())
    }
}

impl From<std::io::Error> for RpcError {
    fn from(err: std::io::Error) -> Self {
        RpcError::ServerError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, RpcError>;
