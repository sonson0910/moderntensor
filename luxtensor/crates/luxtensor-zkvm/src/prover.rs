//! Zero-Knowledge Prover implementation
//!
//! Provides the host-side prover that generates ZK proofs for guest program execution.

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};

use crate::{
    Result, ZkVmError,
    types::{ImageId, GuestInput, ProofReceipt, Proof, ProofType, ProofMetadata, ProverConfig},
};
use luxtensor_crypto::keccak256;

/// Maximum input data size (16 MB) to prevent OOM during proof generation.
const MAX_INPUT_SIZE: usize = 16 * 1024 * 1024;

/// Maximum number of inputs in a single batch_prove call.
const MAX_BATCH_SIZE: usize = 1_000;

/// Statistics about prover operations
#[derive(Debug, Clone, Default)]
pub struct ProverStats {
    /// Total proofs generated
    pub proofs_generated: u64,
    /// Total proving time in milliseconds
    pub total_proving_time_ms: u64,
    /// Average cycles per proof
    pub avg_cycles: u64,
    /// Failed proof attempts
    pub failed_attempts: u64,
    /// GPU proofs count
    pub gpu_proofs: u64,
}

/// Zero-Knowledge Prover
///
/// Generates ZK proofs for guest program execution using RISC Zero zkVM.
///
/// # Proving Modes
///
/// The prover supports multiple proving modes:
/// - **Dev Mode**: Fast mock proofs for development (no actual ZK)
/// - **STARK Mode**: Full RISC Zero STARK proofs (requires `risc0` feature)
/// - **Groth16 Mode**: SNARK-wrapped proofs for smaller size (requires `groth16` feature)
pub struct ZkProver {
    config: ProverConfig,
    stats: Arc<RwLock<ProverStats>>,
    /// Registered guest program images
    registered_images: Arc<RwLock<std::collections::HashMap<ImageId, Vec<u8>>>>,
    /// GPU detection cache
    gpu_detected: std::sync::OnceLock<bool>,
}

impl ZkProver {
    /// Create a new prover with the given configuration
    pub fn new(config: ProverConfig) -> Self {
        info!(
            gpu = config.use_gpu,
            threads = config.threads,
            "Initializing ZkProver"
        );

        Self {
            config,
            stats: Arc::new(RwLock::new(ProverStats::default())),
            registered_images: Arc::new(RwLock::new(std::collections::HashMap::new())),
            gpu_detected: std::sync::OnceLock::new(),
        }
    }

    /// Create a prover with default configuration
    pub fn default_prover() -> Self {
        Self::new(ProverConfig::default())
    }

    /// Create a development-mode prover (fast, no real proofs)
    pub fn dev_prover() -> Self {
        Self::new(ProverConfig::dev_mode())
    }

    /// Register a guest program image
    ///
    /// The image bytes are the compiled RISC-V ELF binary of the guest program.
    pub async fn register_image(&self, image_id: ImageId, elf_bytes: Vec<u8>) -> Result<()> {
        let mut images = self.registered_images.write().await;

        if images.contains_key(&image_id) {
            warn!(image_id = %image_id, "Image already registered, replacing");
        }

        info!(image_id = %image_id, size = elf_bytes.len(), "Registered guest image");
        images.insert(image_id, elf_bytes);
        Ok(())
    }

    /// Generate a ZK proof for the given guest program and input
    ///
    /// # Arguments
    /// * `image_id` - The ID of the registered guest program
    /// * `input` - The input data for the guest program
    ///
    /// # Returns
    /// A `ProofReceipt` containing the proof and public outputs
    ///
    /// # Proving Strategy
    ///
    /// When the `risc0` feature is enabled, this uses RISC Zero's zkVM to generate
    /// cryptographic proofs. Otherwise, it falls back to dev mode proofs.
    pub async fn prove(&self, image_id: ImageId, input: GuestInput) -> Result<ProofReceipt> {
        let start = Instant::now();
        debug!(image_id = %image_id, input_size = input.data.len(), "Starting proof generation");

        // SECURITY: Reject oversized inputs to prevent OOM
        if input.data.len() > MAX_INPUT_SIZE {
            return Err(ZkVmError::InvalidInput(format!(
                "Input data too large: {} bytes (max: {} bytes)",
                input.data.len(),
                MAX_INPUT_SIZE,
            )));
        }
        if let Some(ref priv_data) = input.private_data {
            if priv_data.len() > MAX_INPUT_SIZE {
                return Err(ZkVmError::InvalidInput(format!(
                    "Private data too large: {} bytes (max: {} bytes)",
                    priv_data.len(),
                    MAX_INPUT_SIZE,
                )));
            }
        }

        // Check if image is registered
        let images = self.registered_images.read().await;
        let elf_bytes = images.get(&image_id)
            .ok_or_else(|| ZkVmError::ImageNotFound(image_id.to_string()))?
            .clone();
        drop(images); // Release lock before proving

        // Generate proof, enforcing the configured timeout
        let receipt = if self.config.timeout_seconds > 0 {
            tokio::time::timeout(
                std::time::Duration::from_secs(self.config.timeout_seconds),
                self.generate_proof(image_id, elf_bytes, input),
            )
            .await
            .map_err(|_| ZkVmError::Timeout(self.config.timeout_seconds))?
            ?
        } else {
            self.generate_proof(image_id, elf_bytes, input).await?
        };

        let duration = start.elapsed();

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.proofs_generated += 1;
            stats.total_proving_time_ms = stats.total_proving_time_ms.saturating_add(duration.as_millis() as u64);
            if stats.proofs_generated > 0 {
                // Use saturating arithmetic to prevent overflow in running average
                stats.avg_cycles = stats.avg_cycles
                    .saturating_mul(stats.proofs_generated.saturating_sub(1))
                    .saturating_add(receipt.metadata.cycles)
                    / stats.proofs_generated;
            }
            if self.config.use_gpu && self.gpu_available() {
                stats.gpu_proofs += 1;
            }
        }

        info!(
            image_id = %image_id,
            cycles = receipt.metadata.cycles,
            time_ms = duration.as_millis(),
            proof_type = ?receipt.proof.proof_type,
            "Proof generation complete"
        );

        Ok(receipt)
    }

    /// Internal proof generation dispatcher
    async fn generate_proof(
        &self,
        image_id: ImageId,
        _elf_bytes: Vec<u8>,
        input: GuestInput
    ) -> Result<ProofReceipt> {
        // When risc0 feature is enabled, use real prover
        #[cfg(feature = "risc0")]
        {
            return self.generate_risc0_proof(image_id, _elf_bytes, input).await;
        }

        // Default to dev mode proof
        #[cfg(not(feature = "risc0"))]
        {
            self.generate_dev_proof(image_id, input).await
        }
    }

    /// Generate a RISC Zero STARK proof (when feature enabled)
    #[cfg(feature = "risc0")]
    async fn generate_risc0_proof(
        &self,
        image_id: ImageId,
        elf_bytes: Vec<u8>,
        input: GuestInput,
    ) -> Result<ProofReceipt> {
        use risc0_zkvm::{ExecutorEnv, default_prover};

        let start = Instant::now();

        // Build executor environment with input
        let env = ExecutorEnv::builder()
            .write_slice(&input.data)
            .map_err(|e| ZkVmError::ExecutionFailed(e.to_string()))?
            .build()
            .map_err(|e| ZkVmError::ExecutionFailed(e.to_string()))?;

        // Get prover based on configuration
        let prover = if self.config.use_gpu && self.gpu_available() {
            // GPU-accelerated prover
            default_prover()
        } else {
            // CPU prover
            default_prover()
        };

        // Generate proof
        let prove_info = prover
            .prove(env, &elf_bytes)
            .map_err(|e| ZkVmError::ProofGenerationFailed(e.to_string()))?;

        let receipt = prove_info.receipt;
        let cycles = prove_info.stats.total_cycles;

        // Optionally wrap to Groth16 for smaller proofs
        let (seal, proof_type) = if self.config.wrap_to_groth16 {
            #[cfg(feature = "groth16")]
            {
                let groth16_receipt = risc0_groth16::prove(&receipt)
                    .map_err(|e| ZkVmError::ProofGenerationFailed(e.to_string()))?;
                (groth16_receipt.seal, ProofType::Groth16)
            }
            #[cfg(not(feature = "groth16"))]
            {
                (bincode::serialize(&receipt.inner).unwrap_or_default(), ProofType::Stark)
            }
        } else {
            (bincode::serialize(&receipt.inner).unwrap_or_default(), ProofType::Stark)
        };

        let metadata = ProofMetadata {
            cycles,
            proving_time_ms: start.elapsed().as_millis() as u64,
            memory_bytes: 0, // RISC Zero doesn't expose this directly
            gpu_used: self.config.use_gpu && self.gpu_available(),
            segments: prove_info.stats.segments as u32,
        };

        Ok(ProofReceipt {
            image_id,
            journal: receipt.journal.bytes.clone(),
            proof: Proof { seal, proof_type },
            metadata,
        })
    }

    /// Generate a development-mode proof (no actual ZK)
    ///
    /// This creates a deterministic mock proof that can be verified in dev mode.
    /// The proof is based on hashing the input and image ID.
    async fn generate_dev_proof(&self, image_id: ImageId, input: GuestInput) -> Result<ProofReceipt> {
        // Simulate execution by hashing input
        let journal = keccak256(&input.data).to_vec();

        // Create deterministic seal (hash of image_id + journal)
        let seal = keccak256(&[&image_id.0[..], &journal[..]].concat()).to_vec();

        // Simulate realistic cycle count based on input size
        let base_cycles = 10_000u64;
        let per_byte_cycles = 100u64;
        let simulated_cycles = base_cycles + (input.data.len() as u64 * per_byte_cycles);

        let metadata = ProofMetadata {
            cycles: simulated_cycles,
            proving_time_ms: 1,
            memory_bytes: 1024 * 1024, // 1MB simulated
            gpu_used: false,
            segments: 1,
        };

        Ok(ProofReceipt {
            image_id,
            journal,
            proof: Proof {
                seal,
                proof_type: ProofType::Dev,
            },
            metadata,
        })
    }

    /// Batch prove multiple inputs with parallelization
    ///
    /// Generates proofs for multiple inputs. Uses parallel execution when
    /// the prover supports it and there are enough inputs.
    pub async fn batch_prove(
        &self,
        image_id: ImageId,
        inputs: Vec<GuestInput>,
    ) -> Result<Vec<ProofReceipt>> {
        info!(
            image_id = %image_id,
            count = inputs.len(),
            "Starting batch proof generation"
        );

        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        // SECURITY: Reject oversized batches to prevent resource exhaustion
        if inputs.len() > MAX_BATCH_SIZE {
            return Err(ZkVmError::InvalidInput(format!(
                "Batch too large: {} inputs (max: {})",
                inputs.len(),
                MAX_BATCH_SIZE,
            )));
        }

        // For small batches or dev mode, use sequential processing
        if inputs.len() <= 2 || !cfg!(feature = "risc0") {
            let mut receipts = Vec::with_capacity(inputs.len());
            for input in inputs {
                let receipt = self.prove(image_id, input).await?;
                receipts.push(receipt);
            }
            return Ok(receipts);
        }

        // For larger batches with risc0, use parallel proving
        self.parallel_batch_prove(image_id, inputs).await
    }

    /// Parallel batch proving implementation
    async fn parallel_batch_prove(
        &self,
        image_id: ImageId,
        inputs: Vec<GuestInput>,
    ) -> Result<Vec<ProofReceipt>> {
        use tokio::task::JoinSet;

        let mut join_set = JoinSet::new();
        let images = self.registered_images.clone();
        let config = self.config.clone();

        for input in inputs {
            let images = images.clone();
            let config = config.clone();

            join_set.spawn(async move {
                // Create a temporary prover for this task
                let prover = ZkProver::new(config);

                // Get the elf bytes
                let images_guard = images.read().await;
                if let Some(_elf) = images_guard.get(&image_id) {
                    // We need to register before proving in this worker
                    drop(images_guard);
                    let elf_clone = {
                        let g = images.read().await;
                        g.get(&image_id).cloned()
                    };
                    if let Some(elf_bytes) = elf_clone {
                        prover.register_image(image_id, elf_bytes).await?;
                        prover.prove(image_id, input).await
                    } else {
                        Err(ZkVmError::ImageNotFound(image_id.to_string()))
                    }
                } else {
                    Err(ZkVmError::ImageNotFound(image_id.to_string()))
                }
            });
        }

        let mut receipts = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(receipt)) => receipts.push(receipt),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(ZkVmError::InternalError(e.to_string())),
            }
        }

        Ok(receipts)
    }

    /// Get current prover statistics
    pub async fn stats(&self) -> ProverStats {
        self.stats.read().await.clone()
    }

    /// Check if GPU is available for proving
    ///
    /// Detects GPU availability based on:
    /// - CUDA feature enabled and CUDA device present
    /// - Metal feature enabled and Metal device present (macOS)
    pub fn gpu_available(&self) -> bool {
        *self.gpu_detected.get_or_init(|| {
            self.detect_gpu()
        })
    }

    /// Detect available GPU
    fn detect_gpu(&self) -> bool {
        // Check CUDA availability
        #[cfg(feature = "cuda")]
        {
            // Try to detect CUDA devices
            if std::env::var("CUDA_VISIBLE_DEVICES").is_ok() {
                return true;
            }
            // Check for nvidia-smi
            if std::process::Command::new("nvidia-smi")
                .arg("--query-gpu=name")
                .arg("--format=csv,noheader")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return true;
            }
        }

        // Check Metal availability (macOS)
        #[cfg(feature = "metal")]
        {
            #[cfg(target_os = "macos")]
            {
                // On macOS with Metal feature, assume GPU is available
                return true;
            }
        }

        // No GPU detected
        false
    }

    /// Get the prover configuration
    pub fn config(&self) -> &ProverConfig {
        &self.config
    }

    /// Get the number of registered images
    pub async fn registered_image_count(&self) -> usize {
        self.registered_images.read().await.len()
    }

    /// Check if an image is registered
    pub async fn is_image_registered(&self, image_id: &ImageId) -> bool {
        self.registered_images.read().await.contains_key(image_id)
    }

    /// Unregister a guest program image
    pub async fn unregister_image(&self, image_id: &ImageId) -> bool {
        self.registered_images.write().await.remove(image_id).is_some()
    }

    /// Clear all registered images
    pub async fn clear_images(&self) {
        self.registered_images.write().await.clear();
    }

    /// Reset prover statistics
    pub async fn reset_stats(&self) {
        *self.stats.write().await = ProverStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prover_creation() {
        let prover = ZkProver::default_prover();
        assert!(!prover.config.use_gpu);
    }

    #[tokio::test]
    async fn test_register_image() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);
        let elf_bytes = vec![0u8; 100];

        prover.register_image(image_id, elf_bytes).await.unwrap();
        assert!(prover.is_image_registered(&image_id).await);

        // Should not error on second registration
        prover.register_image(image_id, vec![1u8; 50]).await.unwrap();
    }

    #[tokio::test]
    async fn test_prove_unregistered_image() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);
        let input = GuestInput::new(vec![1, 2, 3]);

        let result = prover.prove(image_id, input).await;
        assert!(matches!(result, Err(ZkVmError::ImageNotFound(_))));
    }

    #[tokio::test]
    async fn test_dev_proof_generation() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);
        let elf_bytes = vec![0u8; 100];

        prover.register_image(image_id, elf_bytes).await.unwrap();

        let input = GuestInput::new(vec![1, 2, 3, 4]);
        let receipt = prover.prove(image_id, input).await.unwrap();

        assert_eq!(receipt.image_id, image_id);
        assert!(receipt.metadata.cycles > 0);
        assert_eq!(receipt.proof.proof_type, ProofType::Dev);
    }

    #[tokio::test]
    async fn test_batch_prove() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);

        prover.register_image(image_id, vec![0u8; 100]).await.unwrap();

        let inputs = vec![
            GuestInput::new(vec![1, 2]),
            GuestInput::new(vec![3, 4]),
            GuestInput::new(vec![5, 6]),
        ];

        let receipts = prover.batch_prove(image_id, inputs).await.unwrap();
        assert_eq!(receipts.len(), 3);
    }

    #[tokio::test]
    async fn test_batch_prove_empty() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);

        let receipts = prover.batch_prove(image_id, vec![]).await.unwrap();
        assert!(receipts.is_empty());
    }

    #[tokio::test]
    async fn test_stats_update() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);

        prover.register_image(image_id, vec![0u8; 100]).await.unwrap();

        let initial_stats = prover.stats().await;
        assert_eq!(initial_stats.proofs_generated, 0);

        prover.prove(image_id, GuestInput::new(vec![1, 2, 3])).await.unwrap();

        let updated_stats = prover.stats().await;
        assert_eq!(updated_stats.proofs_generated, 1);
    }

    #[tokio::test]
    async fn test_unregister_image() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);

        prover.register_image(image_id, vec![0u8; 100]).await.unwrap();
        assert!(prover.is_image_registered(&image_id).await);

        let removed = prover.unregister_image(&image_id).await;
        assert!(removed);
        assert!(!prover.is_image_registered(&image_id).await);
    }

    #[tokio::test]
    async fn test_clear_images() {
        let prover = ZkProver::dev_prover();

        for i in 0..5 {
            let image_id = ImageId::new([i; 32]);
            prover.register_image(image_id, vec![0u8; 100]).await.unwrap();
        }

        assert_eq!(prover.registered_image_count().await, 5);

        prover.clear_images().await;
        assert_eq!(prover.registered_image_count().await, 0);
    }

    #[tokio::test]
    async fn test_reset_stats() {
        let prover = ZkProver::dev_prover();
        let image_id = ImageId::new([1u8; 32]);

        prover.register_image(image_id, vec![0u8; 100]).await.unwrap();
        prover.prove(image_id, GuestInput::new(vec![1, 2, 3])).await.unwrap();

        let stats = prover.stats().await;
        assert_eq!(stats.proofs_generated, 1);

        prover.reset_stats().await;

        let stats = prover.stats().await;
        assert_eq!(stats.proofs_generated, 0);
    }

    #[test]
    fn test_gpu_detection() {
        let prover = ZkProver::dev_prover();
        // Should not panic
        let _ = prover.gpu_available();
    }
}
