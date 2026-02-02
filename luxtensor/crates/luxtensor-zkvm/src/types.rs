//! Core types for zkVM operations

use serde::{Deserialize, Serialize};
use luxtensor_core::Hash;

/// Unique identifier for a guest program image
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageId(pub [u8; 32]);

impl ImageId {
    /// Create a new ImageId from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the bytes of the ImageId
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create from hex string
    pub fn from_hex(hex: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(hex)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl std::fmt::Display for ImageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Input data for a guest program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestInput {
    /// Raw input bytes
    pub data: Vec<u8>,
    /// Optional private inputs (not included in public journal)
    pub private_data: Option<Vec<u8>>,
}

impl GuestInput {
    /// Create new guest input
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            private_data: None,
        }
    }

    /// Create guest input with private data
    pub fn with_private(data: Vec<u8>, private: Vec<u8>) -> Self {
        Self {
            data,
            private_data: Some(private),
        }
    }

    /// Serialize a typed input
    pub fn from_typed<T: Serialize>(input: &T) -> crate::Result<Self> {
        let data = bincode::serialize(input)?;
        Ok(Self::new(data))
    }
}

/// Output data from a guest program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestOutput {
    /// Public journal (committed outputs)
    pub journal: Vec<u8>,
    /// Execution cycles used
    pub cycles: u64,
}

impl GuestOutput {
    /// Deserialize the journal to a typed output
    pub fn decode<T: for<'de> Deserialize<'de>>(&self) -> crate::Result<T> {
        bincode::deserialize(&self.journal).map_err(Into::into)
    }
}

/// Zero-knowledge proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    /// The cryptographic seal (raw proof bytes)
    pub seal: Vec<u8>,
    /// Proof type identifier
    pub proof_type: ProofType,
}

/// Type of proof
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofType {
    /// Standard RISC Zero proof (STARK-based)
    Stark,
    /// SNARK-wrapped proof (smaller, uses Groth16)
    Groth16,
    /// Development mode (no actual proof, for testing)
    Dev,
}

/// Complete proof receipt including proof and public outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofReceipt {
    /// The guest program image ID
    pub image_id: ImageId,
    /// Public outputs (journal)
    pub journal: Vec<u8>,
    /// The zero-knowledge proof
    pub proof: Proof,
    /// Execution metadata
    pub metadata: ProofMetadata,
}

impl ProofReceipt {
    /// Compute the commitment hash of this receipt
    pub fn commitment_hash(&self) -> Hash {
        use luxtensor_crypto::keccak256;

        let mut data = Vec::new();
        data.extend_from_slice(self.image_id.as_bytes());
        data.extend_from_slice(&self.journal);
        data.extend_from_slice(&self.proof.seal);

        keccak256(&data)
    }

    /// Serialize the receipt for on-chain submission
    pub fn to_bytes(&self) -> crate::Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    /// Deserialize a receipt
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

/// Metadata about proof generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofMetadata {
    /// Total execution cycles
    pub cycles: u64,
    /// Proving time in milliseconds
    pub proving_time_ms: u64,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Whether GPU was used
    pub gpu_used: bool,
    /// Segment count (for recursion)
    pub segments: u32,
}

impl Default for ProofMetadata {
    fn default() -> Self {
        Self {
            cycles: 0,
            proving_time_ms: 0,
            memory_bytes: 0,
            gpu_used: false,
            segments: 1,
        }
    }
}

/// Configuration for the prover
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProverConfig {
    /// Enable GPU acceleration
    pub use_gpu: bool,
    /// Maximum memory in bytes (0 = unlimited)
    pub max_memory: u64,
    /// Timeout in seconds (0 = no timeout)
    pub timeout_seconds: u64,
    /// Enable SNARK wrapping (smaller proofs)
    pub wrap_to_groth16: bool,
    /// Number of parallel proving threads
    pub threads: usize,
}

impl Default for ProverConfig {
    fn default() -> Self {
        Self {
            use_gpu: false,
            max_memory: 0,
            timeout_seconds: 300, // 5 minutes default
            wrap_to_groth16: false,
            threads: num_cpus::get(),
        }
    }
}

impl ProverConfig {
    /// Create GPU-accelerated config
    pub fn with_gpu() -> Self {
        Self {
            use_gpu: true,
            ..Default::default()
        }
    }

    /// Create fast development config (no real proofs)
    pub fn dev_mode() -> Self {
        Self {
            use_gpu: false,
            max_memory: 0,
            timeout_seconds: 60,
            wrap_to_groth16: false,
            threads: 1,
        }
    }
}

/// Result of proof verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the proof is valid
    pub is_valid: bool,
    /// Verified image ID
    pub image_id: ImageId,
    /// Verified journal hash
    pub journal_hash: Hash,
    /// Verification time in microseconds
    pub verification_time_us: u64,
    /// Error message if invalid
    pub error: Option<String>,
}

impl VerificationResult {
    /// Create a successful verification result
    pub fn valid(image_id: ImageId, journal_hash: Hash, time_us: u64) -> Self {
        Self {
            is_valid: true,
            image_id,
            journal_hash,
            verification_time_us: time_us,
            error: None,
        }
    }

    /// Create a failed verification result
    pub fn invalid(image_id: ImageId, error: String) -> Self {
        Self {
            is_valid: false,
            image_id,
            journal_hash: [0u8; 32],
            verification_time_us: 0,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_id_hex() {
        let bytes = [1u8; 32];
        let id = ImageId::new(bytes);
        let hex_str = id.to_string();

        let parsed = ImageId::from_hex(&hex_str).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_guest_input_serialization() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestInput {
            value: u64,
            name: String,
        }

        let input = TestInput {
            value: 42,
            name: "test".to_string(),
        };

        let guest_input = GuestInput::from_typed(&input).unwrap();
        assert!(!guest_input.data.is_empty());
    }

    #[test]
    fn test_proof_receipt_commitment() {
        let receipt = ProofReceipt {
            image_id: ImageId::new([1u8; 32]),
            journal: vec![1, 2, 3, 4],
            proof: Proof {
                seal: vec![5, 6, 7, 8],
                proof_type: ProofType::Stark,
            },
            metadata: ProofMetadata::default(),
        };

        let hash = receipt.commitment_hash();
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn test_prover_config_defaults() {
        let config = ProverConfig::default();
        assert!(!config.use_gpu);
        assert_eq!(config.timeout_seconds, 300);
    }
}
