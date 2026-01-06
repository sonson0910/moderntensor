use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Network error: {0}")]
    General(String),
    
    #[error("Peer connection error")]
    PeerConnectionError,
}

pub type Result<T> = std::result::Result<T, NetworkError>;
