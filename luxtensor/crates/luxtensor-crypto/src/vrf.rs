//! VRF (Verifiable Random Function) Implementation — EC-VRF (secp256k1)
//!
//! Implements an Elliptic Curve VRF over secp256k1, inspired by RFC 9381
//! (ECVRF-SECP256K1-SHA256-TAI). Provides:
//!
//! - **Pseudorandomness**: Output is indistinguishable from random without the secret key.
//! - **Uniqueness**: Each (key, input) pair produces exactly one output.
//! - **Verifiability**: The proof can be verified using only the public key.
//! - **Unforgeability**: No one without the secret key can produce a valid proof.
//!
//! ## Implementation Details
//!
//! - Public key: compressed SEC1 encoding (33 bytes)
//! - Gamma: SK * H_to_curve(alpha) — actual EC scalar multiplication
//! - Challenge (c): Schnorr-style via Fiat-Shamir hash
//! - Response (s): k - c * sk (mod n)
//! - Output: keccak256(gamma_compressed)
//!
//! Uses the `k256` crate for constant-time secp256k1 arithmetic.

use crate::keccak256;
use k256::{
    elliptic_curve::{
        group::GroupEncoding,
        ops::Reduce,
        sec1::ToEncodedPoint,
    },
    ProjectivePoint, Scalar, AffinePoint,
};

/// VRF output hash type (32 bytes)
pub type VrfOutput = [u8; 32];

/// VRF proof structure (EC-VRF)
/// Contains gamma (EC point), Schnorr challenge (c), and response (s).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VrfProof {
    /// Gamma point — compressed SEC1 encoding (33 bytes, stored in 32+1)
    pub gamma: [u8; 32],
    /// Schnorr-style challenge (truncated to 16 bytes, zero-padded to 32)
    pub c: [u8; 32],
    /// Schnorr-style response (32-byte scalar)
    pub s: [u8; 32],
    /// Full gamma compressed SEC1 point (33 bytes) for EC verification
    gamma_compressed: [u8; 33],
}

impl VrfProof {
    /// Create a new VRF proof
    pub fn new(gamma: [u8; 32], c: [u8; 32], s: [u8; 32]) -> Self {
        // Reconstruct gamma_compressed from gamma (assume 0x02 prefix for legacy compat)
        let mut gc = [0u8; 33];
        gc[0] = 0x02;
        gc[1..33].copy_from_slice(&gamma);
        Self { gamma, c, s, gamma_compressed: gc }
    }

    /// Create proof with full EC data
    fn new_ec(gamma_compressed: [u8; 33], c: [u8; 32], s: [u8; 32]) -> Self {
        let mut gamma = [0u8; 32];
        gamma.copy_from_slice(&gamma_compressed[1..33]);
        Self { gamma, c, s, gamma_compressed }
    }

    /// Serialize proof to bytes (96 bytes for backward compat)
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
        Self::new(gamma, c, s)
    }
}

/// VRF keypair backed by secp256k1
pub struct VrfKeypair {
    /// Secret key scalar
    secret_key: Scalar,
    /// Compressed public key (33 bytes SEC1)
    pub public_key: [u8; 32],
    /// Full compressed SEC1 public key (33 bytes)
    public_key_compressed: [u8; 33],
}

impl Drop for VrfKeypair {
    fn drop(&mut self) {
        // Zeroize the secret key — Scalar doesn't impl Zeroize, so overwrite
        self.secret_key = Scalar::ZERO;
    }
}

/// Hash-to-curve: deterministically map arbitrary bytes to a secp256k1 point.
/// Uses try-and-increment (TAI) per RFC 9381 §5.4.1.
fn hash_to_curve(pk_bytes: &[u8], alpha: &[u8]) -> ProjectivePoint {
    // Prefix for domain separation
    let suite_string: &[u8] = b"ECVRF_secp256k1_SHA256_TAI";
    for ctr in 0u8..=255 {
        let mut input = Vec::with_capacity(suite_string.len() + 1 + pk_bytes.len() + alpha.len() + 1);
        input.extend_from_slice(suite_string);
        input.push(0x01); // hash_to_curve flag
        input.extend_from_slice(pk_bytes);
        input.extend_from_slice(alpha);
        input.push(ctr);
        let hash = keccak256(&input);

        // Try to decode as compressed x-coordinate with 0x02 prefix
        let mut compressed = [0u8; 33];
        compressed[0] = 0x02;
        compressed[1..33].copy_from_slice(&hash);

        let ct_opt = AffinePoint::from_bytes(&compressed.into());
        if bool::from(ct_opt.is_some()) {
            return ProjectivePoint::from(ct_opt.unwrap());
        }
    }
    // Fallback: use generator * hash (should never reach here in practice)
    let hash = keccak256(alpha);
    let scalar = <Scalar as Reduce<k256::U256>>::reduce_bytes(&hash.into());
    ProjectivePoint::GENERATOR * scalar
}

/// Compute Fiat-Shamir challenge: c = keccak256(pk || gamma || k*G || k*H) truncated to 16 bytes
fn compute_challenge(
    pk_compressed: &[u8; 33],
    h_point: &ProjectivePoint,
    gamma: &ProjectivePoint,
    k_g: &ProjectivePoint,
    k_h: &ProjectivePoint,
) -> Scalar {
    let encode = |p: &ProjectivePoint| -> Vec<u8> {
        p.to_affine().to_encoded_point(true).as_bytes().to_vec()
    };

    let mut input = Vec::with_capacity(200);
    input.extend_from_slice(b"ECVRF_secp256k1_challenge");
    input.extend_from_slice(pk_compressed);
    input.extend_from_slice(&encode(h_point));
    input.extend_from_slice(&encode(gamma));
    input.extend_from_slice(&encode(k_g));
    input.extend_from_slice(&encode(k_h));

    let hash = keccak256(&input);

    // Truncate to 16 bytes (128 bits) per RFC 9381 for challenge
    let mut c_bytes = [0u8; 32];
    c_bytes[16..32].copy_from_slice(&hash[0..16]);
    <Scalar as Reduce<k256::U256>>::reduce_bytes(&c_bytes.into())
}

impl VrfKeypair {
    /// Generate a new VRF keypair from a 32-byte seed.
    /// Derives a secp256k1 secret key deterministically.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        // Derive secret key scalar from seed
        let sk = <Scalar as Reduce<k256::U256>>::reduce_bytes(&(*seed).into());
        // Ensure non-zero (reduce_bytes can return zero for zero input)
        let sk = if sk.is_zero().into() {
            let fallback = keccak256(seed);
            <Scalar as Reduce<k256::U256>>::reduce_bytes(&fallback.into())
        } else {
            sk
        };

        let pk_point = ProjectivePoint::GENERATOR * sk;
        let pk_affine = pk_point.to_affine();
        let pk_encoded = pk_affine.to_encoded_point(true);
        let pk_bytes_full = pk_encoded.as_bytes();

        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&pk_bytes_full[1..33]);

        let mut public_key_compressed = [0u8; 33];
        public_key_compressed.copy_from_slice(pk_bytes_full);

        Self {
            secret_key: sk,
            public_key,
            public_key_compressed,
        }
    }

    /// Get the public key (x-coordinate, 32 bytes)
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }

    /// Prove: Generate EC-VRF proof for a given input alpha.
    /// Returns (output, proof) where output = keccak256(gamma_compressed).
    pub fn prove(&self, alpha: &[u8]) -> (VrfOutput, VrfProof) {
        // Step 1: H = hash_to_curve(pk, alpha)
        let h = hash_to_curve(&self.public_key_compressed, alpha);

        // Step 2: Gamma = sk * H
        let gamma = h * self.secret_key;

        // Step 3: Choose random nonce k (deterministic: k = H(sk || alpha || "nonce"))
        let mut k_input = Vec::with_capacity(64 + alpha.len());
        let sk_bytes: [u8; 32] = self.secret_key.to_bytes().into();
        k_input.extend_from_slice(&sk_bytes);
        k_input.extend_from_slice(alpha);
        k_input.extend_from_slice(b"ECVRF_nonce");
        let k_hash = keccak256(&k_input);
        let k = <Scalar as Reduce<k256::U256>>::reduce_bytes(&k_hash.into());

        // Step 4: U = k * G, V = k * H
        let u = ProjectivePoint::GENERATOR * k;
        let v = h * k;

        // Step 5: c = challenge(pk, H, Gamma, U, V)
        let c = compute_challenge(&self.public_key_compressed, &h, &gamma, &u, &v);

        // Step 6: s = k - c * sk (mod n)
        let s = k - c * self.secret_key;

        // Step 7: output = keccak256(gamma_compressed)
        let gamma_encoded = gamma.to_affine().to_encoded_point(true);
        let gamma_bytes = gamma_encoded.as_bytes();
        let mut gamma_compressed = [0u8; 33];
        gamma_compressed.copy_from_slice(gamma_bytes);

        let output = gamma_to_output(&gamma_compressed);

        // Encode c and s as 32-byte arrays
        let c_bytes: [u8; 32] = c.to_bytes().into();
        let s_bytes: [u8; 32] = s.to_bytes().into();

        let proof = VrfProof::new_ec(gamma_compressed, c_bytes, s_bytes);
        (output, proof)
    }
}

/// Verify an EC-VRF proof against a public key and input alpha.
/// Returns the VRF output if verification succeeds.
///
/// Verification:
///   1. Decode public key Y, gamma Γ from compressed form
///   2. H = hash_to_curve(pk, alpha)
///   3. U = s*G + c*Y
///   4. V = s*H + c*Γ
///   5. c' = challenge(pk, H, Γ, U, V)
///   6. Verify c' == c
///   7. Output = keccak256(Γ_compressed)
pub fn vrf_verify(
    public_key: &[u8; 32],
    alpha: &[u8],
    proof: &VrfProof,
) -> Result<VrfOutput, VrfError> {
    // Reject trivial proof components
    if proof.gamma == [0u8; 32] {
        return Err(VrfError::InvalidProof);
    }
    if proof.s == [0u8; 32] && proof.c == [0u8; 32] {
        return Err(VrfError::InvalidProof);
    }

    // Try both compressed key prefixes (0x02 = even y, 0x03 = odd y)
    // and accept whichever yields a valid challenge match.
    for prefix in [0x02u8, 0x03u8] {
        let mut compressed_pk = [0u8; 33];
        compressed_pk[0] = prefix;
        compressed_pk[1..33].copy_from_slice(public_key);

        let opt = AffinePoint::from_bytes(&compressed_pk.into());
        if !bool::from(opt.is_some()) {
            continue;
        }
        let pk_point = ProjectivePoint::from(opt.unwrap());

        // Reconstruct the full compressed public key
        let pk_affine = pk_point.to_affine();
        let pk_enc = pk_affine.to_encoded_point(true);
        let pk_bytes = pk_enc.as_bytes();
        let mut pk_full = [0u8; 33];
        pk_full.copy_from_slice(pk_bytes);

        // Decode gamma Γ
        let gamma = {
            let gc = &proof.gamma_compressed;
            let gopt = AffinePoint::from_bytes(&(*gc).into());
            if !bool::from(gopt.is_some()) {
                return Err(VrfError::InvalidProof);
            }
            ProjectivePoint::from(gopt.unwrap())
        };

        // Decode c and s as scalars
        let c = <Scalar as Reduce<k256::U256>>::reduce_bytes(&proof.c.into());
        let s = <Scalar as Reduce<k256::U256>>::reduce_bytes(&proof.s.into());

        // H = hash_to_curve(pk, alpha)
        let h = hash_to_curve(&pk_full, alpha);

        // U = s*G + c*Y
        let u = ProjectivePoint::GENERATOR * s + pk_point * c;

        // V = s*H + c*Γ
        let v = h * s + gamma * c;

        // c' = challenge(pk, H, Γ, U, V)
        let c_prime = compute_challenge(&pk_full, &h, &gamma, &u, &v);

        // Verify c' == c (constant-time comparison via scalar equality)
        let c_prime_bytes: [u8; 32] = c_prime.to_bytes().into();
        let c_bytes: [u8; 32] = c.to_bytes().into();

        let mut diff = 0u8;
        for (a, b) in c_prime_bytes.iter().zip(c_bytes.iter()) {
            diff |= a ^ b;
        }
        if diff == 0 {
            // Output = keccak256(gamma_compressed)
            let output = gamma_to_output(&proof.gamma_compressed);
            return Ok(output);
        }
        // Wrong prefix — try the other one
    }

    Err(VrfError::VerificationFailed)
}

/// Convert VRF gamma point to output hash
fn gamma_to_output(gamma_compressed: &[u8]) -> VrfOutput {
    // Use full compressed point (33 bytes) + domain separator
    let mut output_input = Vec::with_capacity(gamma_compressed.len() + 32);
    output_input.extend_from_slice(gamma_compressed);
    output_input.extend_from_slice(b"ECVRF_secp256k1_output");
    keccak256(&output_input)
}

/// Check if a VRF output is below a threshold (for leader selection)
/// threshold is in range [0, u64::MAX]
pub fn vrf_output_below_threshold(output: &VrfOutput, threshold: u64) -> bool {
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

        // Same input = same output and proof (deterministic nonce)
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
    fn test_verify_wrong_input_fails() {
        let seed = [42u8; 32];
        let keypair = VrfKeypair::from_seed(&seed);

        let (_output, proof) = keypair.prove(b"correct_input");
        let result = vrf_verify(&keypair.public_key, b"wrong_input", &proof);

        assert!(result.is_err());
    }

    #[test]
    fn test_verify_wrong_key_fails() {
        let keypair1 = VrfKeypair::from_seed(&[42u8; 32]);
        let keypair2 = VrfKeypair::from_seed(&[43u8; 32]);

        let (_output, proof) = keypair1.prove(b"test");
        let result = vrf_verify(&keypair2.public_key, b"test", &proof);

        assert!(result.is_err());
    }

    #[test]
    fn test_proof_serialization() {
        let proof = VrfProof::new([1u8; 32], [2u8; 32], [3u8; 32]);
        let bytes = proof.to_bytes();
        let restored = VrfProof::from_bytes(&bytes);

        assert_eq!(proof.gamma, restored.gamma);
        assert_eq!(proof.c, restored.c);
        assert_eq!(proof.s, restored.s);
    }

    #[test]
    fn test_selection_threshold() {
        let total_stake = 1_000_000u128;

        let threshold_50 = calculate_selection_threshold(500_000, total_stake);
        assert!(threshold_50 > u64::MAX / 3);
        let two_thirds = ((u64::MAX as u128) * 2 / 3) as u64;
        assert!(threshold_50 < two_thirds);

        let threshold_100 = calculate_selection_threshold(total_stake, total_stake);
        assert!(threshold_100 > u64::MAX - 1000);

        let threshold_0 = calculate_selection_threshold(0, total_stake);
        assert_eq!(threshold_0, 0);
    }

    #[test]
    fn test_vrf_output_threshold_check() {
        let mut output = [0u8; 32];
        output[0..8].copy_from_slice(&100u64.to_le_bytes());

        assert!(vrf_output_below_threshold(&output, 101));
        assert!(!vrf_output_below_threshold(&output, 100));
        assert!(!vrf_output_below_threshold(&output, 50));
    }
}
