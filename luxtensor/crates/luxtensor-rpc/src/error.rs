use thiserror::Error;

/// Standard JSON-RPC 2.0 error codes
pub mod error_codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;

    // Custom Luxtensor error codes (-32000 to -32099)
    pub const BLOCK_NOT_FOUND: i64 = -32001;
    pub const TRANSACTION_NOT_FOUND: i64 = -32002;
    pub const ACCOUNT_NOT_FOUND: i64 = -32003;
    pub const INSUFFICIENT_FUNDS: i64 = -32004;
    pub const INVALID_SIGNATURE: i64 = -32005;
    pub const NONCE_TOO_LOW: i64 = -32006;
    pub const NONCE_TOO_HIGH: i64 = -32007;
    pub const GAS_LIMIT_EXCEEDED: i64 = -32008;
    pub const CONTRACT_EXECUTION_ERROR: i64 = -32009;
    pub const RATE_LIMITED: i64 = -32010;
    pub const MEMPOOL_FULL: i64 = -32011;
    pub const STORAGE_ERROR: i64 = -32050;
}

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

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    // New detailed errors
    #[error("Insufficient funds: have {have}, need {need}")]
    InsufficientFunds { have: String, need: String },

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Nonce too low: expected {expected}, got {got}")]
    NonceTooLow { expected: u64, got: u64 },

    #[error("Gas limit exceeded: limit {limit}, required {required}")]
    GasLimitExceeded { limit: u64, required: u64 },

    #[error("Contract execution failed: {reason}")]
    ContractExecutionError { reason: String },

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("Mempool full: {current}/{max} transactions")]
    MempoolFull { current: usize, max: usize },
}

impl RpcError {
    /// Get JSON-RPC error code for this error
    pub fn code(&self) -> i64 {
        match self {
            RpcError::ParseError(_) => error_codes::PARSE_ERROR,
            RpcError::InvalidRequest(_) => error_codes::INVALID_REQUEST,
            RpcError::MethodNotFound(_) => error_codes::METHOD_NOT_FOUND,
            RpcError::InvalidParams(_) => error_codes::INVALID_PARAMS,
            RpcError::InternalError(_) => error_codes::INTERNAL_ERROR,
            RpcError::BlockNotFound(_) => error_codes::BLOCK_NOT_FOUND,
            RpcError::TransactionNotFound(_) => error_codes::TRANSACTION_NOT_FOUND,
            RpcError::AccountNotFound(_) => error_codes::ACCOUNT_NOT_FOUND,
            RpcError::InsufficientFunds { .. } => error_codes::INSUFFICIENT_FUNDS,
            RpcError::InvalidSignature => error_codes::INVALID_SIGNATURE,
            RpcError::NonceTooLow { .. } => error_codes::NONCE_TOO_LOW,
            RpcError::GasLimitExceeded { .. } => error_codes::GAS_LIMIT_EXCEEDED,
            RpcError::ContractExecutionError { .. } => error_codes::CONTRACT_EXECUTION_ERROR,
            RpcError::RateLimited(_) => error_codes::RATE_LIMITED,
            RpcError::MempoolFull { .. } => error_codes::MEMPOOL_FULL,
            RpcError::StorageError(_) => error_codes::STORAGE_ERROR,
            _ => error_codes::INTERNAL_ERROR,
        }
    }

    /// Convert to JSON-RPC error object
    pub fn to_json_rpc(&self) -> serde_json::Value {
        serde_json::json!({
            "code": self.code(),
            "message": self.to_string(),
            "data": self.additional_data()
        })
    }

    /// Get additional error data if available
    fn additional_data(&self) -> Option<serde_json::Value> {
        match self {
            RpcError::InsufficientFunds { have, need } => Some(serde_json::json!({
                "have": have,
                "need": need
            })),
            RpcError::NonceTooLow { expected, got } => Some(serde_json::json!({
                "expected": expected,
                "got": got
            })),
            RpcError::GasLimitExceeded { limit, required } => Some(serde_json::json!({
                "limit": limit,
                "required": required
            })),
            RpcError::MempoolFull { current, max } => Some(serde_json::json!({
                "current": current,
                "max": max
            })),
            _ => None,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = RpcError::BlockNotFound("0x123".to_string());
        assert_eq!(err.code(), error_codes::BLOCK_NOT_FOUND);
    }

    #[test]
    fn test_to_json_rpc() {
        let err = RpcError::NonceTooLow { expected: 5, got: 3 };
        let json = err.to_json_rpc();
        assert_eq!(json["code"], error_codes::NONCE_TOO_LOW);
        assert!(json["message"].as_str().unwrap().contains("expected 5"));
    }
}
