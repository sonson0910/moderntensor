use luxtensor_core::types::Hash;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Network error: {0}")]
    General(String),
    
    #[error("Peer connection error")]
    PeerConnectionError,
    
    #[error("P2P error: {0}")]
    P2P(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    
    #[error("Block not found: {0:?}")]
    BlockNotFound(Hash),
    
    #[error("No peers available")]
    NoPeersAvailable,
    
    #[error("Already syncing")]
    AlreadySyncing,
    
    #[error("Invalid chain: {0}")]
    InvalidChain(String),
    
    #[error("Sync error: {0}")]
    Sync(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Invalid message")]
    InvalidMessage,
}

pub type Result<T> = std::result::Result<T, NetworkError>;
