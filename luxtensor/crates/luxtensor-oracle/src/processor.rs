use crate::error::Result;
use ethers::types::{Bytes, H256};
use luxtensor_zkvm::{ZkProver, GuestInput, ImageId, ProverConfig};
use tracing::{info, warn, debug};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

/// Image ID for the AI model inference guest program.
/// In production, this MUST be set to the hash of the actual RISC-V ELF binary
/// via `initialize()` before `process_request()` is called.
const DEFAULT_AI_IMAGE_ID: [u8; 32] = [0xAA; 32];

pub struct RequestProcessor {
    /// ZkProver instance for generating proofs
    prover: Arc<RwLock<ZkProver>>,
    /// Image ID for the AI inference program
    ai_image_id: ImageId,
    /// Threshold (in gas units) below which optimistic mode is used
    optimistic_threshold: u64,
    /// Dispute window in blocks for optimistic results
    dispute_window_blocks: u64,
}

impl RequestProcessor {
    /// Create a new RequestProcessor in **development mode**.
    ///
    /// # Warning
    /// Dev mode uses a mock prover — proofs are NOT cryptographically valid.
    /// Use [`with_config(ProverConfig::default())`](Self::with_config) for production.
    pub fn new() -> Self {
        tracing::warn!(
            "RequestProcessor created in DEV MODE — proofs will not be valid. \
             Use RequestProcessor::with_config(ProverConfig::default()) for production."
        );
        let prover = ZkProver::dev_prover();
        let ai_image_id = ImageId::new(DEFAULT_AI_IMAGE_ID);

        Self {
            prover: Arc::new(RwLock::new(prover)),
            ai_image_id,
            optimistic_threshold: Self::DEFAULT_OPTIMISTIC_THRESHOLD,
            dispute_window_blocks: Self::DEFAULT_DISPUTE_WINDOW,
        }
    }

    /// Create a RequestProcessor with custom prover configuration
    pub fn with_config(config: ProverConfig) -> Self {
        let prover = ZkProver::new(config);
        let ai_image_id = ImageId::new(DEFAULT_AI_IMAGE_ID);

        Self {
            prover: Arc::new(RwLock::new(prover)),
            ai_image_id,
            optimistic_threshold: Self::DEFAULT_OPTIMISTIC_THRESHOLD,
            dispute_window_blocks: Self::DEFAULT_DISPUTE_WINDOW,
        }
    }

    /// Initialize the prover by registering the AI inference guest program.
    ///
    /// # Arguments
    /// * `elf_bytes` - The compiled RISC-V ELF binary of the AI inference guest program.
    ///   **Required for production.** If `None`, initialization will fail in production mode.
    pub async fn initialize(&self, elf_bytes: Option<Vec<u8>>) -> Result<()> {
        let prover = self.prover.read().await;

        let elf = match elf_bytes {
            Some(bytes) if !bytes.is_empty() => bytes,
            _ => {
                warn!("No ELF binary provided for AI inference guest program — \
                       oracle will not be able to generate valid proofs. \
                       Provide the ELF binary via initialize(Some(elf_bytes)).");
                return Err(crate::error::OracleError::ProofGeneration(
                    "ELF binary is required for oracle initialization. \
                     Cannot register a mock image in production."
                        .to_string(),
                ));
            }
        };

        prover.register_image(self.ai_image_id, elf).await
            .map_err(|e| crate::error::OracleError::ProofGeneration(e.to_string()))?;

        info!(image_id = %self.ai_image_id, "AI inference guest program registered");
        Ok(())
    }

    /// Maximum time allowed for a single AI inference + proof generation.
    const INFERENCE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

    /// Default gas threshold below which optimistic mode is used.
    /// Requests costing less than this skip ZK proof generation.
    pub const DEFAULT_OPTIMISTIC_THRESHOLD: u64 = 100_000;

    /// Default dispute window (in blocks) for optimistic results.
    /// During this window, any validator can challenge the result.
    pub const DEFAULT_DISPUTE_WINDOW: u64 = 50;

    /// Process an AI computation request and generate a ZK proof
    pub async fn process_request(
        &self,
        request_id: H256,
        model_hash: H256,
        input_data: Bytes,
    ) -> Result<(Bytes, H256)> {
        info!(request_id = ?request_id, model = ?model_hash, "Processing AI request with zkVM");

        // Prepare guest input: combine model hash and input data
        let mut guest_input_bytes = Vec::with_capacity(32 + input_data.len());
        guest_input_bytes.extend_from_slice(model_hash.as_bytes());
        guest_input_bytes.extend_from_slice(&input_data);

        let guest_input = GuestInput::new(guest_input_bytes);

        // Generate proof using zkVM
        let prover = self.prover.read().await;

        // Verify the AI inference image is registered
        if !prover.is_image_registered(&self.ai_image_id).await {
            return Err(crate::error::OracleError::ProofGeneration(
                "AI inference image not registered. Call initialize() with a valid ELF binary \
                 before processing requests."
                    .to_string(),
            ));
        }

        let receipt = tokio::time::timeout(
            Self::INFERENCE_TIMEOUT,
            prover.prove(self.ai_image_id, guest_input),
        )
        .await
        .map_err(|_| {
            crate::error::OracleError::ProofGeneration(format!(
                "AI inference timed out after {:?} for request {:?}",
                Self::INFERENCE_TIMEOUT,
                request_id
            ))
        })?
        .map_err(|e| crate::error::OracleError::ProofGeneration(e.to_string()))?;

        info!(
            cycles = receipt.metadata.cycles,
            time_ms = receipt.metadata.proving_time_ms,
            proof_type = ?receipt.proof.proof_type,
            "ZK proof generated successfully"
        );

        // The journal contains the output of the computation (keccak hash of input in dev mode)
        let result = Bytes::from(receipt.journal);

        // Generate proof hash from the seal
        let proof_hash = H256::from_slice(&luxtensor_crypto::keccak256(&receipt.proof.seal)[..]);

        debug!(
            request_id = ?request_id,
            proof_hash = ?proof_hash,
            result_len = result.len(),
            "AI request completed"
        );

        Ok((result, proof_hash))
    }

    /// Get prover statistics
    pub async fn get_stats(&self) -> luxtensor_zkvm::ProverStats {
        self.prover.read().await.stats().await
    }

    /// Check if GPU is available for accelerated proving
    pub async fn gpu_available(&self) -> bool {
        self.prover.read().await.gpu_available()
    }

    // ==================== Optimistic Mode (Fix 7) ====================

    /// Process a request in optimistic mode if below gas threshold.
    ///
    /// Low-value requests skip ZK proof generation entirely, returning
    /// a hash commitment instead. The result can be disputed within
    /// `dispute_window_blocks` blocks.
    ///
    /// Returns `OptimisticResult` with `needs_proof = false` for low-value,
    /// or delegates to full `process_request()` for high-value.
    pub async fn process_request_optimistic(
        &self,
        request_id: H256,
        model_hash: H256,
        input_data: Bytes,
        gas_value: u64,
        current_block: u64,
        miner_address: [u8; 20],
    ) -> Result<OptimisticResult> {
        if gas_value >= self.optimistic_threshold {
            // High-value: full ZK proof required
            let (result, proof_hash) = self.process_request(
                request_id, model_hash, input_data,
            ).await?;

            return Ok(OptimisticResult {
                result,
                proof_hash: Some(proof_hash),
                optimistic: false,
                dispute_deadline: 0, // No dispute window for proven results
                miner_address,
            });
        }

        // Low-value: optimistic mode — skip proof, return commitment hash
        let start = Instant::now();

        // Compute a commitment hash (H(request_id || model_hash || input_data))
        let commitment = {
            let mut data = Vec::with_capacity(64 + input_data.len());
            data.extend_from_slice(request_id.as_bytes());
            data.extend_from_slice(model_hash.as_bytes());
            data.extend_from_slice(&input_data);
            H256::from_slice(&luxtensor_crypto::keccak256(&data)[..])
        };

        info!(
            request_id = ?request_id,
            gas_value = gas_value,
            elapsed_us = start.elapsed().as_micros() as u64,
            "Optimistic mode: skipped ZK proof for low-value request"
        );

        Ok(OptimisticResult {
            result: Bytes::from(commitment.as_bytes().to_vec()),
            proof_hash: None,
            optimistic: true,
            dispute_deadline: current_block + self.dispute_window_blocks,
            miner_address,
        })
    }

    /// Get the current optimistic threshold.
    #[allow(dead_code)]
    pub fn optimistic_threshold(&self) -> u64 {
        self.optimistic_threshold
    }

    /// Set a custom optimistic threshold.
    #[allow(dead_code)]
    pub fn set_optimistic_threshold(&mut self, threshold: u64) {
        self.optimistic_threshold = threshold;
    }
}

/// Result of optimistic request processing.
///
/// For low-value requests, `optimistic = true` and `proof_hash = None`.
/// The result can be challenged before `dispute_deadline` block.
#[derive(Debug, Clone)]
pub struct OptimisticResult {
    /// Computation result (or commitment hash in optimistic mode)
    pub result: Bytes,
    /// ZK proof hash (None for optimistic results)
    pub proof_hash: Option<H256>,
    /// Whether this result was produced optimistically (no proof)
    pub optimistic: bool,
    /// Block number by which disputes must be filed (0 for proven results)
    pub dispute_deadline: u64,
    /// Address of the miner who produced this result (for potential slashing).
    /// 20-byte Ethereum-style address.
    pub miner_address: [u8; 20],
}

impl Default for RequestProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: get the guest ELF bytes for tests.
    ///
    /// When the `risc0` feature is enabled, this returns the real compiled
    /// RISC-V ELF binary from `luxtensor-methods`. When disabled (default),
    /// it returns mock bytes that work with the dev-mode prover.
    fn guest_elf_bytes() -> Vec<u8> {
        #[cfg(feature = "risc0")]
        {
            luxtensor_methods::LUXTENSOR_GUEST_ELF.to_vec()
        }
        #[cfg(not(feature = "risc0"))]
        {
            // Dev-mode prover accepts any bytes as ELF — it hashes
            // inputs deterministically instead of executing real RISC-V.
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        }
    }

    #[tokio::test]
    async fn test_request_processor_creation() {
        let processor = RequestProcessor::new();
        // Should create without panic
        assert!(!processor.gpu_available().await);
    }

    #[tokio::test]
    async fn test_process_request() {
        let processor = RequestProcessor::new();

        // Register the guest ELF image via initialize()
        processor
            .initialize(Some(guest_elf_bytes()))
            .await
            .expect("failed to initialize with guest ELF");

        let request_id = H256::from([1u8; 32]);
        let model_hash = H256::from([2u8; 32]);
        let input_data = Bytes::from(vec![1, 2, 3, 4]);

        let (result, proof_hash) = processor.process_request(
            request_id,
            model_hash,
            input_data.clone(),
        ).await.unwrap();

        // Result should be the journal (keccak hash in dev mode, sha256 in risc0 mode)
        assert!(!result.is_empty());
        // Proof hash should be non-zero
        assert_ne!(proof_hash, H256::zero());
    }

    #[tokio::test]
    async fn test_prover_stats() {
        let processor = RequestProcessor::new();

        // Register the guest ELF image via initialize()
        processor
            .initialize(Some(guest_elf_bytes()))
            .await
            .expect("failed to initialize with guest ELF");

        // Process a request to update stats
        let _ = processor.process_request(
            H256::zero(),
            H256::zero(),
            Bytes::from(vec![1, 2, 3]),
        ).await;

        let stats = processor.get_stats().await;
        assert_eq!(stats.proofs_generated, 1);
    }

    // ==================== Optimistic Mode Tests ====================

    #[tokio::test]
    async fn test_optimistic_low_value() {
        let processor = RequestProcessor::new();
        let request_id = H256::from([1u8; 32]);
        let model_hash = H256::from([2u8; 32]);
        let input_data = Bytes::from(vec![1, 2, 3]);

        // Gas below threshold → optimistic
        let result = processor.process_request_optimistic(
            request_id, model_hash, input_data, 50_000, 100, [0xAA; 20],
        ).await.unwrap();

        assert!(result.optimistic);
        assert!(result.proof_hash.is_none());
        assert_eq!(result.dispute_deadline, 100 + RequestProcessor::DEFAULT_DISPUTE_WINDOW);
        assert!(!result.result.is_empty());
    }

    #[tokio::test]
    async fn test_optimistic_deterministic_commitment() {
        let processor = RequestProcessor::new();
        let request_id = H256::from([1u8; 32]);
        let model_hash = H256::from([2u8; 32]);
        let input_data = Bytes::from(vec![1, 2, 3]);

        let r1 = processor.process_request_optimistic(
            request_id, model_hash, input_data.clone(), 50_000, 100, [0xAA; 20],
        ).await.unwrap();

        let r2 = processor.process_request_optimistic(
            request_id, model_hash, input_data, 50_000, 200, [0xAA; 20],
        ).await.unwrap();

        // Same inputs → same commitment (result), different deadlines
        assert_eq!(r1.result, r2.result);
        assert_ne!(r1.dispute_deadline, r2.dispute_deadline);
    }

    #[tokio::test]
    async fn test_optimistic_threshold_boundary() {
        let processor = RequestProcessor::new();

        // Register ELF so at-threshold requests can generate a real proof
        processor
            .initialize(Some(guest_elf_bytes()))
            .await
            .expect("failed to initialize with guest ELF");

        let request_id = H256::from([1u8; 32]);
        let model_hash = H256::from([2u8; 32]);
        let input_data = Bytes::from(vec![1, 2]);

        // Exactly at threshold → NOT optimistic (goes to full proof path)
        let threshold = RequestProcessor::DEFAULT_OPTIMISTIC_THRESHOLD;
        let result = processor.process_request_optimistic(
            request_id, model_hash, input_data, threshold, 100, [0xAA; 20],
        ).await;

        // With ELF registered, full proof should succeed
        assert!(result.is_ok(), "At-threshold request should succeed with registered ELF");
        let result = result.unwrap();
        assert!(!result.optimistic, "At-threshold should use full proof, not optimistic");
        assert!(result.proof_hash.is_some(), "Full proof should produce a proof hash");
    }
}
