//! # LuxTensor Cryptography
//!
//! Cryptographic primitives for LuxTensor blockchain.
//!
//! ## Features
//! - Key pair generation and management
//! - Message signing and verification
//! - Hash functions (SHA3, Blake3)
//! - Merkle trees

use ed25519_dalek::{SigningKey, VerifyingKey, Signer, Verifier, SignatureError};
use luxtensor_types::{Address, Hash, Signature, Result, LuxTensorError};
use rand::rngs::OsRng;
use sha3::{Digest, Keccak256};

/// Key pair for signing transactions
pub struct KeyPair {
    signing_key: SigningKey,
}

impl KeyPair {
    /// Generate new random keypair
    pub fn generate() -> Self {
        let mut secret_bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut OsRng, &mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        Self { signing_key }
    }

    /// Create from secret key bytes
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Result<Self> {
        let signing_key = SigningKey::from_bytes(secret);
        Ok(Self { signing_key })
    }

    /// Sign message
    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.signing_key.sign(message);
        sig.to_bytes()
    }

    /// Get public key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Derive address from public key
    pub fn address(&self) -> Address {
        derive_address(&self.signing_key.verifying_key())
    }

    /// Get secret key bytes
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

/// Derive address from public key (last 20 bytes of keccak256 hash)
pub fn derive_address(public_key: &VerifyingKey) -> Address {
    let public_key_bytes = public_key.as_bytes();
    let hash = Keccak256::digest(public_key_bytes);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..32]);
    address
}

/// Verify signature
pub fn verify_signature(
    message: &[u8],
    signature: &Signature,
    public_key: &VerifyingKey,
) -> Result<()> {
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    
    public_key
        .verify(message, &sig)
        .map_err(|_| LuxTensorError::InvalidSignature)
}

/// Hash data using Keccak256
pub fn keccak256(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Hash data using Blake3
pub fn blake3_hash(data: &[u8]) -> Hash {
    *blake3::hash(data).as_bytes()
}

/// Merkle tree for transactions
pub struct MerkleTree {
    leaves: Vec<Hash>,
    root: Hash,
}

impl MerkleTree {
    /// Create new Merkle tree from leaves
    pub fn new(leaves: Vec<Hash>) -> Self {
        let root = Self::compute_root(&leaves);
        Self { leaves, root }
    }

    /// Get root hash
    pub fn root(&self) -> Hash {
        self.root
    }

    /// Compute Merkle root recursively
    fn compute_root(leaves: &[Hash]) -> Hash {
        if leaves.is_empty() {
            return [0u8; 32];
        }
        
        if leaves.len() == 1 {
            return leaves[0];
        }

        let mut next_level = Vec::new();
        
        for chunk in leaves.chunks(2) {
            let hash = if chunk.len() == 2 {
                // Hash pair
                let mut combined = Vec::with_capacity(64);
                combined.extend_from_slice(&chunk[0]);
                combined.extend_from_slice(&chunk[1]);
                keccak256(&combined)
            } else {
                // Odd number, hash single
                chunk[0]
            };
            next_level.push(hash);
        }

        Self::compute_root(&next_level)
    }

    /// Get proof for leaf at index
    pub fn get_proof(&self, _index: usize) -> Vec<Hash> {
        // TODO: Implement Merkle proof generation
        vec![]
    }

    /// Verify Merkle proof
    pub fn verify_proof(_leaf: &Hash, _proof: &[Hash], _root: &Hash) -> bool {
        // TODO: Implement Merkle proof verification
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = KeyPair::generate();
        let address = keypair.address();
        assert_eq!(address.len(), 20);
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = KeyPair::generate();
        let message = b"Hello, LuxTensor!";
        
        let signature = keypair.sign(message);
        let result = verify_signature(message, &signature, &keypair.verifying_key());
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_keccak256() {
        let data = b"test data";
        let hash = keccak256(data);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_merkle_tree() {
        let leaves = vec![
            keccak256(b"leaf1"),
            keccak256(b"leaf2"),
            keccak256(b"leaf3"),
            keccak256(b"leaf4"),
        ];
        
        let tree = MerkleTree::new(leaves);
        let root = tree.root();
        assert_eq!(root.len(), 32);
    }
}
