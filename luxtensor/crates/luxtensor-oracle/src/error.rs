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
}

pub type Result<T> = std::result::Result<T, OracleError>;
