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
}

pub type Result<T> = std::result::Result<T, ContractError>;
