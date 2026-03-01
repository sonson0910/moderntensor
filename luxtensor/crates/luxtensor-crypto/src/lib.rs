pub mod error;
pub mod hash;
pub mod merkle;
pub mod signature;
pub mod vrf;

// Explicit re-exports — hash
pub use hash::{blake3_hash, keccak256, sha256, Hash};

// Re-export recover_address (deprecated) alongside its replacement for backward compatibility
#[allow(deprecated)]
pub use signature::{
    address_from_public_key, recover_address, recover_address_strict, recover_public_key,
    verify_signature, CryptoAddress, KeyPair,
};

// Explicit re-exports — merkle
pub use merkle::{MerkleTree, ProofElement};

// Explicit re-exports — error
pub use error::{CryptoError, Result};

// Explicit re-exports — vrf
pub use vrf::{
    calculate_selection_threshold, vrf_output_below_threshold, vrf_verify, VrfError, VrfKeypair,
    VrfOutput, VrfProof,
};
