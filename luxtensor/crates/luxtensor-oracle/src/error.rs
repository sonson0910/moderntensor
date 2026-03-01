use thiserror::Error;

#[derive(Error, Debug)]
pub enum OracleError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Contract error: {0}")]
    Contract(String),

    #[error("AI Inference error: {0}")]
    AiInference(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Proof generation error: {0}")]
    ProofGeneration(String),

    #[error("Dispute error: {0}")]
    DisputeError(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),
}

pub type Result<T> = std::result::Result<T, OracleError>;

impl OracleError {
    /// SECURITY(ORACLE-21): Return a sanitized error message safe for external exposure.
    /// Internal details (node URLs, stack traces, contract addresses) are stripped.
    /// Use this when returning errors over RPC/API boundaries.
    pub fn sanitized_message(&self) -> &'static str {
        match self {
            OracleError::Connection(_) => "Connection error",
            OracleError::Contract(_) => "Contract interaction error",
            OracleError::AiInference(_) => "AI inference error",
            OracleError::Transaction(_) => "Transaction error",
            OracleError::ProofGeneration(_) => "Proof generation error",
            OracleError::DisputeError(_) => "Dispute error",
            OracleError::Config(_) => "Configuration error",
            OracleError::Timeout(_) => "Operation timed out",
        }
    }
}
