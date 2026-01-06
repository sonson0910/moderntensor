//! # LuxTensor Types
//!
//! Core types used across all LuxTensor crates.
//!
//! ## Types
//! - `Hash` - 32-byte hash
//! - `Address` - 20-byte account address
//! - `BlockHeight` - Block height type
//! - `Signature` - 64-byte signature
//! - `LuxTensorError` - Error types
//! - `Result<T>` - Result type alias

use serde::{Deserialize, Serialize};

/// 32-byte hash (256-bit)
pub type Hash = [u8; 32];

/// 20-byte address (160-bit)
pub type Address = [u8; 20];

/// 64-byte signature
pub type Signature = [u8; 64];

/// Block height type
pub type BlockHeight = u64;

/// Custom error type for LuxTensor
#[derive(Debug, thiserror::Error)]
pub enum LuxTensorError {
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u128, available: u128 },
    
    #[error("Account not found: {0}")]
    AccountNotFound(String),
    
    #[error("Block not found: {0}")]
    BlockNotFound(String),
    
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),
    
    #[error("Invalid block producer")]
    InvalidBlockProducer,
    
    #[error("Invalid hash")]
    InvalidHash,
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Encoding error: {0}")]
    EncodingError(String),
    
    #[error("Decoding error: {0}")]
    DecodingError(String),
    
    #[error("Unknown RPC method")]
    UnknownMethod,
    
    #[error("Invalid parameters")]
    InvalidParams,
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, LuxTensorError>;

/// Chain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub block_time: u64,
    pub epoch_length: u64,
    pub max_validators: usize,
    pub min_stake: u128,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            // TODO: Update chain_id before mainnet launch
            // 9999 is used for development/testnet
            chain_id: 9999,
            block_time: 3, // 3 seconds
            epoch_length: 100,
            max_validators: 100,
            min_stake: 1_000_000, // 1M tokens
        }
    }
}

/// Helper function to format hash as hex string
pub fn hash_to_hex(hash: &Hash) -> String {
    hex::encode(hash)
}

/// Helper function to format address as hex string
pub fn address_to_hex(address: &Address) -> String {
    format!("0x{}", hex::encode(address))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_to_hex() {
        let hash: Hash = [0u8; 32];
        let hex = hash_to_hex(&hash);
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn test_address_to_hex() {
        let address: Address = [0u8; 20];
        let hex = address_to_hex(&address);
        assert!(hex.starts_with("0x"));
        assert_eq!(hex.len(), 42); // 0x + 40 chars
    }
}
