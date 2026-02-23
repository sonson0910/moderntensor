use thiserror::Error;

/// Contract execution errors
#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Contract not found")]
    ContractNotFound,

    #[error("Invalid contract code: {0}")]
    InvalidCode(String),

    #[error("Code size too large (max 24KB)")]
    CodeSizeTooLarge,

    #[error("Out of gas")]
    OutOfGas,

    #[error("Gas limit exceeded")]
    GasLimitExceeded,

    #[error("Execution reverted: {0}")]
    ExecutionReverted(String),

    #[error("Storage key not found")]
    StorageKeyNotFound,

    #[error("Invalid ABI: {0}")]
    InvalidABI(String),

    #[error("Contract execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Arithmetic overflow")]
    ArithmeticOverflow,
}

impl From<luxtensor_core::CoreError> for ContractError {
    fn from(err: luxtensor_core::CoreError) -> Self {
        ContractError::ExecutionFailed(err.to_string())
    }
}

impl From<luxtensor_storage::StorageError> for ContractError {
    fn from(err: luxtensor_storage::StorageError) -> Self {
        ContractError::ExecutionFailed(format!("Storage: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, ContractError>;
