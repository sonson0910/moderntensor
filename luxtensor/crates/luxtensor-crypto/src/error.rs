use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid public key")]
    InvalidPublicKey,
    
    #[error("Invalid private key")]
    InvalidPrivateKey,
    
    #[error("Secp256k1 error: {0}")]
    Secp256k1Error(String),
}

pub type Result<T> = std::result::Result<T, CryptoError>;
