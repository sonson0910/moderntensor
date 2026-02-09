use crate::error::Result;
use ethers::types::{Bytes, H256};
use luxtensor_zkvm::{ZkProver, GuestInput, ImageId, ProverConfig};
use tracing::{info, warn, debug};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Image ID for the AI model inference guest program.
/// In production, this MUST be set to the hash of the actual RISC-V ELF binary
/// via `initialize()` before `process_request()` is called.
const DEFAULT_AI_IMAGE_ID: [u8; 32] = [0xAA; 32];

pub struct RequestProcessor {
    /// ZkProver instance for generating proofs
    prover: Arc<RwLock<ZkProver>>,
    /// Image ID for the AI inference program
    ai_image_id: ImageId,
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
        }
    }

    /// Create a RequestProcessor with custom prover configuration
    pub fn with_config(config: ProverConfig) -> Self {
        let prover = ZkProver::new(config);
        let ai_image_id = ImageId::new(DEFAULT_AI_IMAGE_ID);

        Self {
            prover: Arc::new(RwLock::new(prover)),
            ai_image_id,
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

        let receipt = prover.prove(self.ai_image_id, guest_input).await
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
}

impl Default for RequestProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_request_processor_creation() {
        let processor = RequestProcessor::new();
        // Should create without panic
        assert!(!processor.gpu_available().await);
    }

    #[tokio::test]
    #[ignore = "requires zkVM ELF binary and risc0-zkvm setup"]
    async fn test_process_request() {
        let processor = RequestProcessor::new();

        // Initialize with mock ELF
        processor.initialize(None).await.unwrap();

        let request_id = H256::from([1u8; 32]);
        let model_hash = H256::from([2u8; 32]);
        let input_data = Bytes::from(vec![1, 2, 3, 4]);

        let (result, proof_hash) = processor.process_request(
            request_id,
            model_hash,
            input_data.clone(),
        ).await.unwrap();

        // Result should be the journal (keccak hash in dev mode)
        assert!(!result.is_empty());
        // Proof hash should be non-zero
        assert_ne!(proof_hash, H256::zero());
    }

    #[tokio::test]
    #[ignore = "requires zkVM ELF binary and risc0-zkvm setup"]
    async fn test_prover_stats() {
        let processor = RequestProcessor::new();
        processor.initialize(None).await.unwrap();

        // Process a request to update stats
        let _ = processor.process_request(
            H256::zero(),
            H256::zero(),
            Bytes::from(vec![1, 2, 3]),
        ).await;

        let stats = processor.get_stats().await;
        assert_eq!(stats.proofs_generated, 1);
    }
}
