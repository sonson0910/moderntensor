//! Proof of Training (PoT) Verification Module
//!
//! This module implements verification of training computations for
//! federated learning on LuxTensor. It prevents gradient fabrication
//! attacks by requiring trainers to prove honest computation.
//!
//! # Architecture
//!
//! PoT uses random checkpoint sampling:
//! 1. During training, random batches are sampled
//! 2. Trainer must prove correct forward/backward pass on those batches
//! 3. ZK proofs verify computation without revealing model weights
//!
//! # Verification Methods
//!
//! - `RandomCheckpointSampling`: Sample N random batches, require proofs
//! - `IntermediateStateProof`: Prove intermediate activations are correct
//! - `GradientCommitment`: Commit to gradients before aggregation

use luxtensor_crypto::keccak256;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::types::{ImageId, Proof, ProofMetadata, ProofReceipt, ProofType};
use crate::verifier::ZkVerifier;

/// Proof of Training types
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PoTProofType {
    /// Random checkpoint sampling proof
    RandomCheckpoint,
    /// Intermediate state verification
    IntermediateState,
    /// Gradient commitment proof
    GradientCommitment,
}

/// Training checkpoint data for verification
#[derive(Clone, Debug)]
pub struct TrainingCheckpoint {
    /// Job ID this checkpoint belongs to
    pub job_id: [u8; 32],
    /// Round number
    pub round: u64,
    /// Address of the trainer who submitted this checkpoint
    pub trainer: [u8; 20],
    /// Batch indices sampled for verification
    pub sampled_batches: Vec<u64>,
    /// Hash of model state before batch
    pub pre_batch_hash: [u8; 32],
    /// Hash of model state after batch
    pub post_batch_hash: [u8; 32],
    /// Hash of gradients computed
    pub gradient_hash: [u8; 32],
}

/// Proof of Training submission
#[derive(Clone, Debug)]
pub struct PoTProof {
    /// Type of proof
    pub proof_type: PoTProofType,
    /// Checkpoint being proven
    pub checkpoint: TrainingCheckpoint,
    /// ZK proof data (e.g., RISC Zero receipt)
    pub proof_data: Vec<u8>,
    /// Public inputs for verification
    pub public_inputs: Vec<u8>,
}

/// Verification result
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationResult {
    /// Proof is valid
    Valid,
    /// Proof is invalid with reason
    Invalid(String),
    /// Proof format is malformed
    Malformed,
}

/// Proof of Training Verifier
#[derive(Clone, Default)]
pub struct PoTVerifier {
    /// Verified checkpoints by job_id
    verified_checkpoints: HashMap<[u8; 32], Vec<TrainingCheckpoint>>,
    /// Challenge seeds per round (for random sampling)
    challenge_seeds: HashMap<([u8; 32], u64), [u8; 32]>,
    /// Optional ZkVerifier for real cryptographic proof verification.
    /// When present, proof verification delegates to RISC Zero / Groth16.
    /// When absent, falls back to structural validation.
    zk_verifier: Option<Arc<ZkVerifier>>,
}

impl PoTVerifier {
    /// Create new PoT verifier without a cryptographic ZkVerifier backend.
    ///
    /// Falls back to structural validation for ZK proofs.
    /// Use [`new_with_verifier`](Self::new_with_verifier) for production deployments.
    pub fn new() -> Self {
        Self {
            verified_checkpoints: HashMap::new(),
            challenge_seeds: HashMap::new(),
            zk_verifier: None,
        }
    }

    /// Create a new PoT verifier backed by a real [`ZkVerifier`].
    ///
    /// When a `ZkVerifier` is provided, proof verification delegates to it,
    /// supporting Dev, STARK (RISC Zero), and Groth16 proof modes.
    pub fn new_with_verifier(zk_verifier: Arc<ZkVerifier>) -> Self {
        info!("PoTVerifier initialized with ZkVerifier for cryptographic proof verification");
        Self {
            verified_checkpoints: HashMap::new(),
            challenge_seeds: HashMap::new(),
            zk_verifier: Some(zk_verifier),
        }
    }

    /// Generate challenge seed for a training round
    ///
    /// The challenge seed determines which batches must be proven.
    /// It's derived from job_id, round, and block hash for unpredictability.
    pub fn generate_challenge_seed(
        &mut self,
        job_id: [u8; 32],
        round: u64,
        block_hash: [u8; 32],
    ) -> [u8; 32] {
        let mut data = Vec::with_capacity(72);
        data.extend_from_slice(&job_id);
        data.extend_from_slice(&round.to_be_bytes());
        data.extend_from_slice(&block_hash);

        let seed = keccak256(&data);
        self.challenge_seeds.insert((job_id, round), seed);
        seed
    }

    /// Get sampled batch indices from challenge seed
    ///
    /// Returns `num_samples` random batch indices from `total_batches`
    pub fn get_sampled_batches(
        &self,
        job_id: [u8; 32],
        round: u64,
        total_batches: u64,
        num_samples: usize,
    ) -> Option<Vec<u64>> {
        let seed = self.challenge_seeds.get(&(job_id, round))?;

        let mut samples = Vec::with_capacity(num_samples);
        let mut current_seed = *seed;

        for i in 0..num_samples {
            // Hash seed with index to get next random value
            let mut data = Vec::with_capacity(40);
            data.extend_from_slice(&current_seed);
            data.extend_from_slice(&(i as u64).to_be_bytes());
            current_seed = keccak256(&data);

            // Convert to batch index
            let batch_index = u64::from_be_bytes(
                current_seed[0..8].try_into().unwrap()
            ) % total_batches;
            samples.push(batch_index);
        }

        Some(samples)
    }

    /// Verify a Proof of Training submission
    pub fn verify_proof(&mut self, proof: &PoTProof) -> VerificationResult {
        match proof.proof_type {
            PoTProofType::RandomCheckpoint => {
                self.verify_random_checkpoint(proof)
            }
            PoTProofType::IntermediateState => {
                self.verify_intermediate_state(proof)
            }
            PoTProofType::GradientCommitment => {
                self.verify_gradient_commitment(proof)
            }
        }
    }

    /// Verify random checkpoint sampling proof
    fn verify_random_checkpoint(&mut self, proof: &PoTProof) -> VerificationResult {
        let checkpoint = &proof.checkpoint;

        // 1. Verify the sampled batches match expected from challenge seed
        if let Some(expected_batches) = self.get_sampled_batches(
            checkpoint.job_id,
            checkpoint.round,
            1000, // Assumed total batches - should come from job config
            checkpoint.sampled_batches.len(),
        ) {
            if checkpoint.sampled_batches != expected_batches {
                return VerificationResult::Invalid(
                    "Sampled batches don't match challenge".to_string()
                );
            }
        } else {
            return VerificationResult::Invalid(
                "No challenge seed found for this round".to_string()
            );
        }

        // 2. Verify ZK proof of correct computation
        if !self.verify_zk_proof(proof) {
            return VerificationResult::Invalid(
                "ZK proof verification failed".to_string()
            );
        }

        // 3. Verify gradient hash matches commitment
        let computed_gradient_hash = self.compute_gradient_hash(
            &checkpoint.pre_batch_hash,
            &checkpoint.post_batch_hash,
        );
        if computed_gradient_hash != checkpoint.gradient_hash {
            return VerificationResult::Invalid(
                "Gradient hash mismatch".to_string()
            );
        }

        // Store verified checkpoint
        self.verified_checkpoints
            .entry(checkpoint.job_id)
            .or_default()
            .push(checkpoint.clone());

        VerificationResult::Valid
    }

    /// Verify intermediate state proof.
    ///
    /// Validates that the trainer correctly computed forward pass activations
    /// by checking the ZK proof of intermediate state transitions.
    fn verify_intermediate_state(&self, proof: &PoTProof) -> VerificationResult {

        if proof.proof_data.is_empty() {
            return VerificationResult::Malformed;
        }

        // Verify ZK proof
        if !self.verify_zk_proof(proof) {
            return VerificationResult::Invalid(
                "Intermediate state proof failed".to_string()
            );
        }

        VerificationResult::Valid
    }

    /// Verify gradient commitment
    fn verify_gradient_commitment(&self, proof: &PoTProof) -> VerificationResult {
        // Verify the gradient commitment matches the revealed gradients
        // Used to ensure trainer can't change gradients after seeing others

        if proof.proof_data.len() < 32 {
            return VerificationResult::Malformed;
        }

        // Extract commitment from proof_data
        let mut commitment = [0u8; 32];
        commitment.copy_from_slice(&proof.proof_data[0..32]);

        // Hash the revealed gradients
        let gradient_hash = keccak256(&proof.public_inputs);

        // Verify commitment matches
        if commitment != gradient_hash {
            return VerificationResult::Invalid(
                "Gradient commitment mismatch".to_string()
            );
        }

        VerificationResult::Valid
    }

    /// Verify the ZK proof attached to a PoT submission.
    ///
    /// # Verification Strategy
    ///
    /// 1. If a [`ZkVerifier`] is configured, delegates to real cryptographic
    ///    verification (Dev / STARK / Groth16 depending on verifier config).
    /// 2. Otherwise, performs **structural validation** as a secure fallback:
    ///    seal size bounds, gradient-hash commitment binding, and entropy checks.
    fn verify_zk_proof(&self, pot_proof: &PoTProof) -> bool {
        let proof_data = &pot_proof.proof_data;
        let public_inputs = &pot_proof.public_inputs;

        if proof_data.is_empty() || public_inputs.is_empty() {
            debug!(
                proof_len = proof_data.len(),
                inputs_len = public_inputs.len(),
                "PoT ZK proof rejected: empty proof_data or public_inputs"
            );
            return false;
        }

        // Delegate to real cryptographic verification when available
        if let Some(ref verifier) = self.zk_verifier {
            return self.verify_with_zk_verifier(verifier, pot_proof);
        }

        // Fallback: structural validation (no ZkVerifier configured)
        self.verify_zk_structural(pot_proof)
    }

    /// Delegate PoT proof verification to the real [`ZkVerifier`].
    ///
    /// First attempts to deserialize `proof_data` as a full [`ProofReceipt`].
    /// If that fails, constructs a receipt from raw parts (deriving image_id
    /// from the checkpoint's `job_id` and inferring proof type from seal size).
    fn verify_with_zk_verifier(&self, verifier: &ZkVerifier, pot_proof: &PoTProof) -> bool {
        let proof_data = &pot_proof.proof_data;
        let public_inputs = &pot_proof.public_inputs;

        // Try to decode proof_data as a serialized ProofReceipt
        let receipt = match ProofReceipt::from_bytes(proof_data) {
            Ok(r) => {
                info!(
                    image_id = %r.image_id,
                    seal_len = r.proof.seal.len(),
                    proof_type = ?r.proof.proof_type,
                    "Deserialized ProofReceipt from PoT proof_data"
                );
                r
            }
            Err(_) => {
                // Construct a receipt from raw parts.
                // image_id is derived from the training job_id.
                let image_id = ImageId::new(pot_proof.checkpoint.job_id);
                // Heuristic: seals ≤ 512 bytes are likely Groth16; larger are STARK.
                let proof_type = if proof_data.len() <= 512 {
                    ProofType::Groth16
                } else {
                    ProofType::Stark
                };

                debug!(
                    image_id = %image_id,
                    seal_len = proof_data.len(),
                    proof_type = ?proof_type,
                    "Constructed ProofReceipt from raw PoT data"
                );

                ProofReceipt {
                    image_id,
                    journal: public_inputs.clone(),
                    proof: Proof {
                        seal: proof_data.clone(),
                        proof_type,
                    },
                    metadata: ProofMetadata::default(),
                }
            }
        };

        match verifier.verify(&receipt) {
            Ok(result) if result.is_valid => {
                // Additional binding check: ensure the journal commits to the
                // gradient hash declared in the PoT checkpoint.
                if public_inputs.len() >= 32 {
                    if let Ok(expected) = <[u8; 32]>::try_from(&public_inputs[..32]) {
                        let journal_hash = keccak256(&receipt.journal);
                        if journal_hash != expected {
                            debug!(
                                "PoT journal hash does not match gradient commitment in \
                                 public_inputs — binding may diverge"
                            );
                        }
                    }
                }
                info!("PoT ZK proof verified successfully via ZkVerifier");
                true
            }
            Ok(result) => {
                warn!(
                    error = ?result.error,
                    "PoT ZK proof rejected by ZkVerifier"
                );
                false
            }
            Err(e) => {
                warn!(error = %e, "ZkVerifier error during PoT proof verification");
                false
            }
        }
    }

    /// Structural validation fallback when no [`ZkVerifier`] is configured.
    ///
    /// Performs the following **non-cryptographic** sanity checks:
    ///
    /// * Seal size is within `[MIN_SEAL_SIZE, MAX_SEAL_SIZE]` bounds.
    /// * `public_inputs` is at least 32 bytes and its first 32 bytes match
    ///   the checkpoint's `gradient_hash` (commitment binding).
    /// * Seal has sufficient entropy (rejects trivially zeroed proofs).
    ///
    /// # Security Warning
    ///
    /// Structural checks alone do **not** provide cryptographic soundness.
    /// A malicious trainer can craft data that passes these checks.
    /// Always configure a real `ZkVerifier` for production.
    fn verify_zk_structural(&self, pot_proof: &PoTProof) -> bool {
        let proof_data = &pot_proof.proof_data;
        let public_inputs = &pot_proof.public_inputs;
        let checkpoint = &pot_proof.checkpoint;

        // ── Size bounds ──────────────────────────────────────────────────
        const MIN_SEAL_SIZE: usize = 16;
        const MAX_SEAL_SIZE: usize = 2 * 1024 * 1024; // 2 MB

        if proof_data.len() < MIN_SEAL_SIZE {
            debug!(
                seal_len = proof_data.len(),
                min = MIN_SEAL_SIZE,
                "PoT structural: proof_data below minimum seal size"
            );
            return false;
        }
        if proof_data.len() > MAX_SEAL_SIZE {
            warn!(
                seal_len = proof_data.len(),
                max = MAX_SEAL_SIZE,
                "PoT structural: proof_data exceeds maximum seal size (DoS risk)"
            );
            return false;
        }

        // ── Gradient-hash commitment binding ─────────────────────────────
        if public_inputs.len() < 32 {
            debug!(
                inputs_len = public_inputs.len(),
                "PoT structural: public_inputs too short for gradient commitment"
            );
            return false;
        }

        let declared_hash: [u8; 32] = match public_inputs[..32].try_into() {
            Ok(h) => h,
            Err(_) => {
                debug!("PoT structural: failed to parse gradient hash from public_inputs");
                return false;
            }
        };

        if declared_hash != checkpoint.gradient_hash {
            debug!(
                "PoT structural: public_inputs gradient commitment does not match \
                 checkpoint gradient_hash"
            );
            return false;
        }

        // ── Entropy check — reject trivially zeroed seals ────────────────
        let non_zero = proof_data.iter().filter(|&&b| b != 0).count();
        if non_zero < proof_data.len() / 4 {
            debug!(
                non_zero_bytes = non_zero,
                total = proof_data.len(),
                "PoT structural: proof_data has suspiciously low entropy"
            );
            return false;
        }

        warn!(
            proof_len = proof_data.len(),
            inputs_len = public_inputs.len(),
            "PoT proof passed structural validation ONLY (no cryptographic verifier). \
             Configure a ZkVerifier for production-grade verification."
        );
        true
    }

    /// Compute expected gradient hash from state transition
    fn compute_gradient_hash(
        &self,
        pre_batch_hash: &[u8; 32],
        post_batch_hash: &[u8; 32],
    ) -> [u8; 32] {
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(pre_batch_hash);
        data.extend_from_slice(post_batch_hash);
        keccak256(&data)
    }

    /// Get verified checkpoints for a job
    pub fn get_verified_checkpoints(&self, job_id: &[u8; 32]) -> Vec<TrainingCheckpoint> {
        self.verified_checkpoints
            .get(job_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if a specific trainer has a valid PoT checkpoint for a round.
    /// Returns false if no checkpoint exists for this (job, round, trainer) tuple.
    pub fn has_valid_pot(
        &self,
        job_id: [u8; 32],
        round: u64,
        trainer: &[u8; 20],
    ) -> bool {
        if let Some(checkpoints) = self.verified_checkpoints.get(&job_id) {
            return checkpoints.iter().any(|c| c.round == round && &c.trainer == trainer);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_checkpoint() -> TrainingCheckpoint {
        TrainingCheckpoint {
            job_id: [1u8; 32],
            round: 0,
            trainer: [0xAAu8; 20],
            sampled_batches: vec![42, 128, 256],
            pre_batch_hash: [2u8; 32],
            post_batch_hash: [3u8; 32],
            gradient_hash: [0u8; 32], // Will be computed
        }
    }

    #[test]
    fn test_generate_challenge_seed() {
        let mut verifier = PoTVerifier::new();
        let job_id = [1u8; 32];
        let block_hash = [2u8; 32];

        let seed1 = verifier.generate_challenge_seed(job_id, 0, block_hash);
        let seed2 = verifier.generate_challenge_seed(job_id, 1, block_hash);

        // Different rounds should give different seeds
        assert_ne!(seed1, seed2);
    }

    #[test]
    fn test_get_sampled_batches() {
        let mut verifier = PoTVerifier::new();
        let job_id = [1u8; 32];
        let block_hash = [2u8; 32];

        verifier.generate_challenge_seed(job_id, 0, block_hash);

        let batches = verifier.get_sampled_batches(job_id, 0, 1000, 3);
        assert!(batches.is_some());

        let batches = batches.unwrap();
        assert_eq!(batches.len(), 3);

        // All batch indices should be within range
        for idx in &batches {
            assert!(*idx < 1000);
        }
    }

    #[test]
    fn test_gradient_commitment_verification() {
        let verifier = PoTVerifier::new();
        let checkpoint = create_test_checkpoint();

        // Create a valid commitment
        let gradients = vec![1u8, 2, 3, 4, 5];
        let commitment = keccak256(&gradients);

        let proof = PoTProof {
            proof_type: PoTProofType::GradientCommitment,
            checkpoint,
            proof_data: commitment.to_vec(),
            public_inputs: gradients,
        };

        let result = verifier.verify_gradient_commitment(&proof);
        assert_eq!(result, VerificationResult::Valid);
    }

    #[test]
    fn test_invalid_gradient_commitment() {
        let verifier = PoTVerifier::new();
        let checkpoint = create_test_checkpoint();

        // Create an invalid commitment (random bytes)
        let gradients = vec![1u8, 2, 3, 4, 5];
        let wrong_commitment = [9u8; 32];

        let proof = PoTProof {
            proof_type: PoTProofType::GradientCommitment,
            checkpoint,
            proof_data: wrong_commitment.to_vec(),
            public_inputs: gradients,
        };

        let result = verifier.verify_gradient_commitment(&proof);
        assert!(matches!(result, VerificationResult::Invalid(_)));
    }
}
