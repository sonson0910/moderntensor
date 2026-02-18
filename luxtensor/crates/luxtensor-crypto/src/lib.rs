pub mod hash;
pub mod signature;
pub mod merkle;
pub mod error;
pub mod vrf;

// Explicit re-exports — hash
pub use hash::{Hash, keccak256, blake3_hash, sha256};

// Explicit re-exports — signature
pub use signature::{
    CryptoAddress, KeyPair, verify_signature, recover_public_key,
    address_from_public_key, recover_address, recover_address_strict,
};

// Explicit re-exports — merkle
pub use merkle::{MerkleTree, ProofElement};

// Explicit re-exports — error
pub use error::{CryptoError, Result};

// Explicit re-exports — vrf
pub use vrf::{
    VrfOutput, VrfProof, VrfKeypair, VrfError,
    vrf_verify, vrf_output_below_threshold, calculate_selection_threshold,
};
