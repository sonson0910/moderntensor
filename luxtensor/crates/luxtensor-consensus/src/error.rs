use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConsensusError {
    #[error("Invalid validator")]
    InvalidValidator,
    
    #[error("Consensus error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, ConsensusError>;
