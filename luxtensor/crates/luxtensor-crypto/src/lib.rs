//! # LuxTensor Cryptography
//!
//! Cryptographic primitives for LuxTensor blockchain.
//!
//! ## Features
//! - Key pair generation and management
//! - Message signing and verification
//! - Hash functions (SHA3, Blake3)
//! - Merkle trees

use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signer, Verifier};
use luxtensor_types::{Address, Hash, Signature, Result, LuxTensorError};
use rand::rngs::OsRng;
use sha3::{Digest, Keccak256};

/// Key pair for signing transactions
pub struct KeyPair {
    keypair: Keypair,
}

impl KeyPair {
    /// Generate new random keypair
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        Self { keypair }
    }

    /// Create from secret key bytes
    pub fn from_secret_bytes(secret: &[u8; 32]) -> Result<Self> {
        let secret_key = SecretKey::from_bytes(secret)
            .map_err(|e| LuxTensorError::InternalError(e.to_string()))?;
        let public_key = PublicKey::from(&secret_key);
        let keypair = Keypair {
            secret: secret_key,
            public: public_key,
        };
        Ok(Self { keypair })
    }

    /// Sign message
    pub fn sign(&self, message: &[u8]) -> Signature {
        let sig = self.keypair.sign(message);
        sig.to_bytes()
    }

    /// Get public key
    pub fn public_key(&self) -> &PublicKey {
        &self.keypair.public
    }

    /// Derive address from public key
    pub fn address(&self) -> Address {
        derive_address(&self.keypair.public)
    }

    /// Get secret key bytes
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.keypair.secret.to_bytes()
    }
}

/// Derive address from public key (last 20 bytes of keccak256 hash)
pub fn derive_address(public_key: &PublicKey) -> Address {
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
    public_key: &PublicKey,
) -> Result<()> {
    let sig = ed25519_dalek::Signature::from_bytes(signature)
        .map_err(|_| LuxTensorError::InvalidSignature)?;
    
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
    pub fn get_proof(&self, index: usize) -> Vec<Hash> {
        // TODO: Implement Merkle proof generation
        vec![]
    }

    /// Verify Merkle proof
    pub fn verify_proof(leaf: &Hash, proof: &[Hash], root: &Hash) -> bool {
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
        let result = verify_signature(message, &signature, keypair.public_key());
        
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
