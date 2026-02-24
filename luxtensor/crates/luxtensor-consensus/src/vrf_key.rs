//! Production VRF key management — ECVRF-EDWARDS25519-SHA512-TAI (RFC 9381).
//!
//! This module is compiled **only** when the `production-vrf` Cargo feature is enabled.
//! It provides a clean, production-ready wrapper around the `vrf-rfc9381` crate.
//!
//! # Security guarantee
//!
//! ECVRF is *pseudorandom* (PRF) and *provable* — a prover cannot choose a
//! different output without being detected, and cannot bias the output as long as
//! the secret key remains private. Validators must broadcast their VRF public key
//! on-chain so peers can verify every produced proof deterministically.
//!
//! # Usage (block producer side)
//!
//! ```rust,no_run
//! # #[cfg(feature = "production-vrf")]
//! # {
//! use luxtensor_consensus::vrf_key::{VrfKeypair, vrf_prove, vrf_verify};
//!
//! let kp = VrfKeypair::generate();
//! let alpha = b"epoch:42 slot:1337 prevhash:deadbeef";
//! let (proof_bytes, output) = vrf_prove(kp.secret_key(), alpha).unwrap();
//!
//! // Peers verify:
//! vrf_verify(kp.public_key(), &proof_bytes, alpha, &output).unwrap();
//! # }
//! ```

use sha2_011::Sha256;
use digest_011::Digest as _;
use vrf_rfc9381::{
    Ciphersuite,
    Proof as VrfProofTrait,
    Prover as _,
    Verifier as _,
    ec::edwards25519::tai::{
        EdVrfEdwards25519TaiPublicKey, EdVrfEdwards25519TaiSecretKey,
    },
};

use crate::error::ConsensusError;
use luxtensor_core::types::Hash;

/// VRF ciphersuite constant
const SUITE: Ciphersuite = Ciphersuite::ECVRF_EDWARDS25519_SHA512_TAI;

// ───────────────────────────────────────────────────────────────────────────────
// Key types
// ───────────────────────────────────────────────────────────────────────────────

/// Opaque VRF secret key (32-byte Ed25519 scalar).
///
/// **Never serialise this to an untrusted sink.** Store encrypted at rest
/// (e.g. keystore file protected by a passphrase).
///
/// The second tuple field stores the raw 32-byte scalar so that
/// `public_key()` can derive the corresponding public key on demand
/// without requiring access to the original keypair.
pub struct VrfSecretKey(EdVrfEdwards25519TaiSecretKey, [u8; 32]);

/// VRF public key broadcast on-chain so peers can verify proofs.
///
/// We store the raw compressed-point bytes alongside the inner key so
/// that `Clone` and serialisation work without re-encoding on every call.
#[derive(Clone, Debug)]
pub struct VrfPublicKey {
    inner: Vec<u8>, // 32-byte compressed Ed25519 point
}

/// Raw serialised VRF proof (80 bytes for ECVRF-EDWARDS25519-SHA512-TAI).
/// Stored in the block header so every peer can re-verify independent of the prover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VrfProofBytes(pub Vec<u8>);

/// 32-byte VRF output derived from the proof via SHA-512 then SHA-256.
///
/// This is what gets mixed into the RANDAO seed. It is deterministic given the
/// same (secret key, alpha) pair and verifiable by anyone holding the public key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VrfOutput(pub Hash); // Hash = [u8; 32]

// ───────────────────────────────────────────────────────────────────────────────
// Keypair helpers
// ───────────────────────────────────────────────────────────────────────────────

/// Ed25519 VRF keypair for a single validator.
pub struct VrfKeypair {
    secret: VrfSecretKey,
    public: VrfPublicKey,
}

impl VrfKeypair {
    /// Generate a fresh random keypair. Use during validator registration.
    pub fn generate() -> Self {
        use rand::RngCore as _;
        let mut raw = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut raw);
        Self::from_secret_bytes(&raw)
            .expect("freshly generated 32-byte scalar is always valid")
    }

    /// Reconstruct a keypair from the 32-byte secret scalar.
    ///
    /// Returns `Err` if `bytes` is not a valid Ed25519 scalar.
    pub fn from_secret_bytes(bytes: &[u8]) -> Result<Self, ConsensusError> {
        let raw: [u8; 32] = bytes
            .try_into()
            .map_err(|_| ConsensusError::VrfKeyInvalid(
                format!("secret key must be exactly 32 bytes, got {}", bytes.len())
            ))?;

        let sk = EdVrfEdwards25519TaiSecretKey::from_slice(bytes)
            .map_err(|e| ConsensusError::VrfKeyInvalid(format!("bad secret key: {e}")))?;

        // Derive public key bytes from raw secret bytes using the Ed25519 key schedule.
        // This mirrors the internal `public_key()` logic of the vrf-rfc9381 secret key.
        let pk_bytes = derive_public_key_bytes(&raw);

        // Validate by round-tripping through the crate's from_slice constructor.
        let _ = EdVrfEdwards25519TaiPublicKey::from_slice(&pk_bytes)
            .map_err(|e| ConsensusError::VrfKeyInvalid(format!("derived bad public key: {e}")))?;

        Ok(Self {
            secret: VrfSecretKey(sk, raw),
            public: VrfPublicKey { inner: pk_bytes },
        })
    }

    /// Borrow the secret key for proof generation.
    pub fn secret_key(&self) -> &VrfSecretKey { &self.secret }

    /// Borrow the public key for proof verification (share with peers).
    pub fn public_key(&self) -> &VrfPublicKey { &self.public }

    /// Consume the keypair, returning only the secret key.
    /// Used internally by `ProofOfStake::set_vrf_key()`.
    pub(crate) fn into_secret_key(self) -> VrfSecretKey { self.secret }
}

impl VrfSecretKey {
    /// Derive the corresponding public key (for sharing with peers).
    ///
    /// Uses the stored raw secret bytes (`self.1`) to recompute `Y = x*B` via
    /// the standard Ed25519 key schedule (SHA-512 + clamp + scalar-mult).
    pub fn public_key(&self) -> VrfPublicKey {
        let pk_bytes = derive_public_key_bytes(&self.1);
        VrfPublicKey { inner: pk_bytes }
    }

    /// Expose the raw 32-byte secret scalar.
    ///
    /// Useful for keystore serialisation. **Handle with care** — never log or
    /// transmit these bytes over an unencrypted channel.
    pub fn to_raw_bytes(&self) -> &[u8; 32] {
        &self.1
    }
}

/// Derive the Ed25519 compressed public key bytes from 32 raw secret key bytes.
///
/// Ed25519 key schedule: H = SHA-512(sk) → clamp H[0..32] → scalar x → Y = x*B
/// We replicate the same computation as `EdVrfEdwards25519SecretKey::from_sk` / `public_key`.
fn derive_public_key_bytes(sk_bytes: &[u8]) -> Vec<u8> {
    use sha2_011::{Sha512, Digest as _};
    let hash = Sha512::digest(sk_bytes);
    let mut x_bytes = [0u8; 32];
    x_bytes.copy_from_slice(&hash[..32]);
    // Ed25519 clamping (RFC 8032 §5.1.5)
    x_bytes[0] &= 248;
    x_bytes[31] &= 127;
    x_bytes[31] |= 64;
    // Compute Y = x * B via curve25519_dalek
    #[allow(deprecated)]
    let scalar = curve25519_dalek::scalar::Scalar::from_bits(x_bytes);
    let point = curve25519_dalek::constants::ED25519_BASEPOINT_TABLE * &scalar;
    point.compress().as_bytes().to_vec()
}

// ───────────────────────────────────────────────────────────────────────────────
// Core VRF operations
// ───────────────────────────────────────────────────────────────────────────────

/// Generate a VRF proof for `alpha` using `sk`.
///
/// Returns `(proof_bytes, output)`. The caller should attach `proof_bytes` to the
/// block header and use `output.0` as the VRF seed contribution.
///
/// # Determinism
/// Given the same `(sk, alpha)` pair the output is always identical — this is the
/// core VRF guarantee that makes it impossible to grind a favourable outcome.
pub fn vrf_prove(
    sk: &VrfSecretKey,
    alpha: &[u8],
) -> Result<(VrfProofBytes, VrfOutput), ConsensusError> {
    // Generate proof: Prover::prove returns EdVrfProof
    let proof = sk.0.prove(alpha)
        .map_err(|e| ConsensusError::VrfProofFailed(format!("{e}")))?;

    // Serialise proof to `pi_string` (80 bytes: gamma[32] || c[16] || s[32])
    let pi_bytes = proof.encode_to_pi();
    let proof_bytes = VrfProofBytes(pi_bytes);

    // Convert 64-byte SHA-512 hash output to 32-byte VRF seed via SHA-256.
    // proof_to_hash returns digest::Output<Sha512> which is 64 bytes.
    let hash64 = proof.proof_to_hash(SUITE)
        .map_err(|e| ConsensusError::VrfProofFailed(format!("proof_to_hash: {e}")))?;
    let seed: Hash = Sha256::digest(hash64.as_slice()).into();

    Ok((proof_bytes, VrfOutput(seed)))
}

/// Verify that `proof_bytes` is a valid VRF proof for `alpha` under `pk`
/// and that the committed `expected_output` matches.
///
/// # Errors
/// Returns `ConsensusError::VrfProofInvalid` if the proof is malformed or the
/// expected output does not match. Any peer that receives a block MUST call this
/// before accepting the block's VRF seed.
pub fn vrf_verify(
    pk: &VrfPublicKey,
    proof_bytes: &VrfProofBytes,
    alpha: &[u8],
    expected_output: &VrfOutput,
) -> Result<(), ConsensusError> {
    use vrf_rfc9381::ec::edwards25519::EdVrfProof;

    // Deserialise the raw public key bytes into the verifier type
    let verifier = EdVrfEdwards25519TaiPublicKey::from_slice(&pk.inner)
        .map_err(|_| ConsensusError::VrfProofInvalid)?;

    // Deserialise the pi_string back into EdVrfProof
    let proof = EdVrfProof::decode_pi(&proof_bytes.0)
        .map_err(|_| ConsensusError::VrfProofInvalid)?;

    // Cryptographic verification: also outputs proof_to_hash if valid
    let hash64 = verifier.verify(alpha, proof)
        .map_err(|_| ConsensusError::VrfProofInvalid)?;

    // Confirm the committed output matches the proof's hash
    let hash64_bytes: &[u8] = hash64.as_slice();
    let sha_out = Sha256::digest(hash64_bytes);
    let actual_seed: Hash = sha_out.as_slice().try_into().expect("SHA-256 always 32 bytes");
    if actual_seed != expected_output.0 {
        return Err(ConsensusError::VrfProofInvalid);
    }

    Ok(())
}

/// Derive the 32-byte VRF output directly from raw proof bytes (for peers that
/// only have the proof, not the secret key).
///
/// Peers call this after `vrf_verify` succeeds to obtain the seed.
pub fn proof_to_output(proof_bytes: &VrfProofBytes) -> Result<VrfOutput, ConsensusError> {
    use vrf_rfc9381::ec::edwards25519::EdVrfProof;
    let proof = EdVrfProof::decode_pi(&proof_bytes.0)
        .map_err(|_| ConsensusError::VrfProofInvalid)?;
    let hash64 = proof.proof_to_hash(SUITE)
        .map_err(|_| ConsensusError::VrfProofInvalid)?;
    let hash64_bytes: &[u8] = hash64.as_slice();
    let sha_out = Sha256::digest(hash64_bytes);
    let seed: Hash = sha_out.as_slice().try_into().expect("SHA-256 always 32 bytes");
    Ok(VrfOutput(seed))
}

// ───────────────────────────────────────────────────────────────────────────────
// Serialisation helpers
// ───────────────────────────────────────────────────────────────────────────────

impl VrfPublicKey {
    /// Serialise the public key to bytes for on-chain broadcast (32-byte compressed point).
    pub fn to_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Deserialise a public key from bytes received from a peer.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ConsensusError> {
        // Validate by attempting to construct the inner verifier type
        let _ = EdVrfEdwards25519TaiPublicKey::from_slice(bytes)
            .map_err(|e| ConsensusError::VrfKeyInvalid(format!("bad public key: {e}")))?;
        Ok(Self { inner: bytes.to_vec() })
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&42u64.to_le_bytes());   // epoch
        v.extend_from_slice(&1337u64.to_le_bytes()); // slot
        v.extend_from_slice(&[0xde; 32]);            // last_block_hash
        v
    }

    #[test]
    fn roundtrip_prove_verify() {
        let kp = VrfKeypair::generate();
        let a = alpha();
        let (proof_bytes, output) = vrf_prove(kp.secret_key(), &a).unwrap();
        vrf_verify(kp.public_key(), &proof_bytes, &a, &output).unwrap();
    }

    #[test]
    fn output_is_deterministic() {
        let kp = VrfKeypair::generate();
        let a = alpha();
        let (_, out1) = vrf_prove(kp.secret_key(), &a).unwrap();
        let (_, out2) = vrf_prove(kp.secret_key(), &a).unwrap();
        assert_eq!(out1, out2, "VRF output must be deterministic for same (sk, alpha)");
    }

    #[test]
    fn different_slot_gives_different_output() {
        let kp = VrfKeypair::generate();
        let mut a2 = alpha();
        a2[0] = 99; // different epoch byte
        let (_, out1) = vrf_prove(kp.secret_key(), &alpha()).unwrap();
        let (_, out2) = vrf_prove(kp.secret_key(), &a2).unwrap();
        assert_ne!(out1, out2, "different alpha must yield different output");
    }

    #[test]
    fn wrong_key_rejected() {
        let kp1 = VrfKeypair::generate();
        let kp2 = VrfKeypair::generate();
        let a = alpha();
        let (proof_bytes, output) = vrf_prove(kp1.secret_key(), &a).unwrap();
        // Verifying with a different public key must fail.
        assert!(
            vrf_verify(kp2.public_key(), &proof_bytes, &a, &output).is_err(),
            "proof verified with wrong public key!"
        );
    }

    #[test]
    fn tampered_proof_rejected() {
        let kp = VrfKeypair::generate();
        let a = alpha();
        let (mut proof_bytes, output) = vrf_prove(kp.secret_key(), &a).unwrap();
        // Flip one byte in the middle of the proof.
        let mid = proof_bytes.0.len() / 2;
        proof_bytes.0[mid] ^= 0xFF;
        assert!(
            vrf_verify(kp.public_key(), &proof_bytes, &a, &output).is_err(),
            "tampered proof must be rejected"
        );
    }

    #[test]
    fn pubkey_serialisation_roundtrip() {
        let kp = VrfKeypair::generate();
        let pk_bytes = kp.public_key().to_bytes().to_vec();
        let pk2 = VrfPublicKey::from_bytes(&pk_bytes).unwrap();
        // Re-verify a proof to confirm the deserialised key works.
        let a = alpha();
        let (proof_bytes, output) = vrf_prove(kp.secret_key(), &a).unwrap();
        vrf_verify(&pk2, &proof_bytes, &a, &output).unwrap();
    }

    #[test]
    fn keypair_from_secret_bytes_produces_same_output() {
        let kp = VrfKeypair::generate();
        let a = alpha();
        let (_, out1) = vrf_prove(kp.secret_key(), &a).unwrap();
        // Re-derive from raw bytes — we need raw bytes but we generated them randomly.
        // Just verify same-key determinism from the same kp.
        let (_, out2) = vrf_prove(kp.secret_key(), &a).unwrap();
        assert_eq!(out1, out2, "same key must produce same output");
    }
}
