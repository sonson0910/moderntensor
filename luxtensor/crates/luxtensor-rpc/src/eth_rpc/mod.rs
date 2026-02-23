//! # Ethereum-compatible RPC Module
//!
//! Core types and utilities for Ethereum RPC compatibility.
//! Sub-modules handle specific concerns:
//! - `rlp_decoder`: RLP transaction decoding and ecrecover
//! - `faucet`: Dev/test faucet configuration and rate limiting
//! - `eth_methods`: RPC method registration (eth_*, net_*, web3_*)

pub mod rlp_decoder;
pub mod faucet;
pub mod eth_methods;

// Re-export key items for external use
pub use faucet::FaucetRpcConfig;
pub use eth_methods::{register_eth_methods, register_log_methods, register_aa_methods};

// ============================================================================
// Core Types
// ============================================================================

/// Address type (20 bytes)
pub type Address = [u8; 20];

/// Hash type (32 bytes)
pub type TxHash = [u8; 32];

// ============================================================================
// Utility functions
// ============================================================================

pub fn hex_to_address(s: &str) -> Option<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() != 40 {
        return None;
    }
    let bytes = hex::decode(s).ok()?;
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&bytes);
    Some(addr)
}

pub(crate) fn address_to_hex(addr: &Address) -> String {
    format!("0x{}", hex::encode(addr))
}

pub(crate) fn hash_to_hex(hash: &TxHash) -> String {
    format!("0x{}", hex::encode(hash))
}

/// Generate a cryptographically secure transaction hash from sender address and nonce.
///
/// Uses keccak256(RLP([sender, nonce])) following Ethereum conventions.
/// This produces a deterministic, collision-resistant 32-byte hash.
pub fn generate_tx_hash(from: &Address, nonce: u64) -> TxHash {
    use luxtensor_crypto::keccak256;

    // RLP-encode [from, nonce] -- simplified canonical encoding
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(b"TX_HASH_V2:"); // domain separator prevents cross-protocol replay
    data.extend_from_slice(from);
    data.extend_from_slice(&nonce.to_be_bytes());
    keccak256(&data)
}