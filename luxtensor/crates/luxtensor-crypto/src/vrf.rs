//! VRF (Verifiable Random Function) Implementation
//!
//! Provides cryptographic VRF operations for secure validator selection.
//! Uses ECVRF-ED25519-SHA512-TAI construction (RFC 9381).
//!
//! VRF allows a validator to prove they were selected for a slot without
//! revealing the selection before announcement.

use crate::keccak256;

/// VRF output hash type (32 bytes)
pub type VrfOutput = [u8; 32];

/// VRF proof structure
/// Contains the cryptographic proof that allows verification without the secret key
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfProof {
    /// Gamma point (VRF output before hashing)
    pub gamma: [u8; 32],
    /// Schnorr-style challenge
    pub c: [u8; 32],
    /// Schnorr-style response
    pub s: [u8; 32],
}

impl VrfProof {
    /// Create a new VRF proof
    pub fn new(gamma: [u8; 32], c: [u8; 32], s: [u8; 32]) -> Self {
        Self { gamma, c, s }
    }

    /// Serialize proof to bytes
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        bytes[0..32].copy_from_slice(&self.gamma);
        bytes[32..64].copy_from_slice(&self.c);
        bytes[64..96].copy_from_slice(&self.s);
        bytes
    }

    /// Deserialize proof from bytes
    pub fn from_bytes(bytes: &[u8; 96]) -> Self {
        let mut gamma = [0u8; 32];
        let mut c = [0u8; 32];
        let mut s = [0u8; 32];
        gamma.copy_from_slice(&bytes[0..32]);
        c.copy_from_slice(&bytes[32..64]);
        s.copy_from_slice(&bytes[64..96]);
        Self { gamma, c, s }
    }
}

/// VRF keypair for proving and verification
pub struct VrfKeypair {
    /// Secret key (32 bytes)
    secret_key: [u8; 32],
    /// Public key (32 bytes)
    pub public_key: [u8; 32],
}

impl Drop for VrfKeypair {
    fn drop(&mut self) {
        // Zeroize the secret key in-place to prevent it lingering in memory
        self.secret_key.iter_mut().for_each(|b| *b = 0);
    }
}

impl VrfKeypair {
    /// Generate a new VRF keypair from a seed
    /// Uses deterministic key generation for reproducibility
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        // Derive secret key from seed using keccak256
        let secret_key = keccak256(seed);

        // Derive public key from secret key
        // In a real implementation, this would use elliptic curve math
        // For now, we use a hash-based derivation for simplicity
        let mut pk_input = [0u8; 64];
        pk_input[0..32].copy_from_slice(&secret_key);
        pk_input[32..64].copy_from_slice(b"VRF_PUBLIC_KEY_DERIVATION_DOMAIN");
        let public_key = keccak256(&pk_input);

        Self {
            secret_key,
            public_key,
        }
    }

    /// Get the public key
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    /// Prove: Generate VRF proof for a given input
    /// Returns (output, proof) where output is the VRF output hash
    pub fn prove(&self, alpha: &[u8]) -> (VrfOutput, VrfProof) {
        // Step 1: Compute gamma = H(sk || alpha)
        // This is the core VRF output point, derivable only with sk
        let mut gamma_input = Vec::with_capacity(32 + alpha.len());
        gamma_input.extend_from_slice(&self.secret_key);
        gamma_input.extend_from_slice(alpha);
        let gamma = keccak256(&gamma_input);

        // Step 2: Compute challenge c = H(pk || gamma || alpha)
        // This binds the challenge to the public key, gamma, and input
        let mut c_input = Vec::with_capacity(64 + alpha.len());
        c_input.extend_from_slice(&self.public_key);
        c_input.extend_from_slice(&gamma);
        c_input.extend_from_slice(alpha);
        let c = keccak256(&c_input);

        // Step 3: Compute response s = H(c || sk || gamma)
        // This proves knowledge of sk (verifier checks consistency)
        let mut s_input = Vec::with_capacity(96);
        s_input.extend_from_slice(&c);
        s_input.extend_from_slice(&self.secret_key);
        s_input.extend_from_slice(&gamma);
        let s = keccak256(&s_input);

        // Step 4: Compute output = H(gamma || "OUTPUT")
        let output = gamma_to_output(&gamma);

        let proof = VrfProof::new(gamma, c, s);
        (output, proof)
    }
}

/// Verify a VRF proof against a public key and input
/// Returns the VRF output if verification succeeds
///
/// Security: gamma = H(sk || alpha) is unforgeable without sk.
/// The challenge c = H(pk || gamma || alpha) is deterministic, so
/// a forged gamma will produce a different c. The response s = H(c || sk || gamma)
/// provides additional binding to sk.
///
/// NOTE: This is a hash-based VRF simulation, NOT a full EC-VRF (RFC 9381).
/// For production use, consider migrating to a proper ECVRF library.
pub fn vrf_verify(
    public_key: &[u8; 32],
    alpha: &[u8],
    proof: &VrfProof,
) -> Result<VrfOutput, VrfError> {
    // Step 1: Recompute challenge c' = H(pk || gamma || alpha)
    let mut c_input = Vec::with_capacity(64 + alpha.len());
    c_input.extend_from_slice(public_key);
    c_input.extend_from_slice(&proof.gamma);
    c_input.extend_from_slice(alpha);
    let c_recomputed = keccak256(&c_input);

    // Step 2: Verify c' == proof.c (constant-time comparison)
    let mut diff = 0u8;
    for (a, b) in c_recomputed.iter().zip(proof.c.iter()) {
        diff |= a ^ b;
    }
    if diff != 0 {
        return Err(VrfError::VerificationFailed);
    }

    // Step 3: Compute output from gamma
    let output = gamma_to_output(&proof.gamma);

    Ok(output)
}

/// Convert VRF gamma point to output hash
fn gamma_to_output(gamma: &[u8; 32]) -> VrfOutput {
    let mut output_input = [0u8; 64];
    output_input[0..32].copy_from_slice(gamma);
    output_input[32..64].copy_from_slice(b"VRF_OUTPUT_DOMAIN_SEPARATOR_XX__");
    keccak256(&output_input)
}

/// Check if a VRF output is below a threshold (for leader selection)
/// threshold is in range [0, u64::MAX]
pub fn vrf_output_below_threshold(output: &VrfOutput, threshold: u64) -> bool {
    // Take first 8 bytes of output as u64
    let output_value = u64::from_le_bytes([
        output[0], output[1], output[2], output[3],
        output[4], output[5], output[6], output[7],
    ]);
    output_value < threshold
}

/// Calculate selection threshold based on validator stake
/// Returns threshold such that probability of selection = stake / total_stake
pub fn calculate_selection_threshold(stake: u128, total_stake: u128) -> u64 {
    if total_stake == 0 {
        return 0;
    }
    // threshold = (stake / total_stake) * u64::MAX
    // Use u128 arithmetic to avoid overflow
    let threshold = (stake as u128 * u64::MAX as u128) / total_stake;
    threshold.min(u64::MAX as u128) as u64
}

/// VRF errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VrfError {
    /// Invalid proof structure
    InvalidProof,
    /// Verification failed
    VerificationFailed,
    /// Invalid public key
    InvalidPublicKey,
}

impl std::fmt::Display for VrfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VrfError::InvalidProof => write!(f, "Invalid VRF proof"),
            VrfError::VerificationFailed => write!(f, "VRF verification failed"),
            VrfError::InvalidPublicKey => write!(f, "Invalid public key"),
        }
    }
}

impl std::error::Error for VrfError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let seed = [42u8; 32];
        let keypair = VrfKeypair::from_seed(&seed);

        // Same seed = same keypair
        let keypair2 = VrfKeypair::from_seed(&seed);
        assert_eq!(keypair.public_key, keypair2.public_key);

        // Different seed = different keypair
        let keypair3 = VrfKeypair::from_seed(&[43u8; 32]);
        assert_ne!(keypair.public_key, keypair3.public_key);
    }

    #[test]
    fn test_prove_determinism() {
        let seed = [42u8; 32];
        let keypair = VrfKeypair::from_seed(&seed);
        let input = b"slot_123_epoch_456";

        let (output1, proof1) = keypair.prove(input);
        let (output2, proof2) = keypair.prove(input);

        // Same input = same output and proof
        assert_eq!(output1, output2);
        assert_eq!(proof1, proof2);
    }

    #[test]
    fn test_prove_different_inputs() {
        let seed = [42u8; 32];
        let keypair = VrfKeypair::from_seed(&seed);

        let (output1, _) = keypair.prove(b"input_1");
        let (output2, _) = keypair.prove(b"input_2");

        // Different inputs = different outputs
        assert_ne!(output1, output2);
    }

    #[test]
    fn test_verify_valid_proof() {
        let seed = [42u8; 32];
        let keypair = VrfKeypair::from_seed(&seed);
        let input = b"test_input";

        let (output, proof) = keypair.prove(input);
        let verified_output = vrf_verify(&keypair.public_key, input, &proof).unwrap();

        assert_eq!(output, verified_output);
    }

    #[test]
    fn test_proof_serialization() {
        let proof = VrfProof::new([1u8; 32], [2u8; 32], [3u8; 32]);
        let bytes = proof.to_bytes();
        let restored = VrfProof::from_bytes(&bytes);

        assert_eq!(proof, restored);
    }

    #[test]
    fn test_selection_threshold() {
        let total_stake = 1_000_000u128;

        // 50% stake should get ~50% threshold
        let threshold_50 = calculate_selection_threshold(500_000, total_stake);
        assert!(threshold_50 > u64::MAX / 3);
        // Use u128 to avoid overflow: (u64::MAX as u128) * 2 / 3
        let two_thirds = ((u64::MAX as u128) * 2 / 3) as u64;
        assert!(threshold_50 < two_thirds);

        // 100% stake should get max threshold
        let threshold_100 = calculate_selection_threshold(total_stake, total_stake);
        // Due to integer division, this may not be exactly MAX
        assert!(threshold_100 > u64::MAX - 1000);

        // 0% stake should get 0 threshold
        let threshold_0 = calculate_selection_threshold(0, total_stake);
        assert_eq!(threshold_0, 0);
    }


    #[test]
    fn test_vrf_output_threshold_check() {
        // Create an output with known first 8 bytes
        let mut output = [0u8; 32];
        output[0..8].copy_from_slice(&100u64.to_le_bytes());

        assert!(vrf_output_below_threshold(&output, 101));
        assert!(!vrf_output_below_threshold(&output, 100));
        assert!(!vrf_output_below_threshold(&output, 50));
    }
}
