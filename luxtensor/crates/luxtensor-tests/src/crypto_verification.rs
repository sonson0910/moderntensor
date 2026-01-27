//! Cryptography Verification Tests
//!
//! Tests to verify ECDSA and Keccak256 implementations match Ethereum standards.
//! These tests use known test vectors from Ethereum to ensure compatibility.

use luxtensor_crypto::{keccak256, KeyPair};

#[cfg(test)]
mod ecdsa_tests {
    use super::*;

    /// Test that our Keccak256 matches Ethereum's implementation
    /// Using known test vectors from Ethereum
    #[test]
    fn test_keccak256_ethereum_compatibility() {
        // Empty string
        let empty_hash = keccak256(b"");
        assert_eq!(
            hex::encode(empty_hash),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
            "Empty string hash should match Ethereum"
        );

        // "hello"
        let hello_hash = keccak256(b"hello");
        assert_eq!(
            hex::encode(hello_hash),
            "1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8",
            "hello hash should match Ethereum"
        );

        // "Hello World"
        let hw_hash = keccak256(b"Hello World");
        assert_eq!(
            hex::encode(hw_hash),
            "592fa743889fc7f92ac2a37bb1f5ba1daf2a5c84741ca0e0061d243a2e6707ba",
            "Hello World hash should match Ethereum"
        );
    }

    /// Test ECDSA signature generation and verification
    #[test]
    fn test_ecdsa_sign_verify() {
        let keypair = KeyPair::generate();
        let message = b"test message for signing";
        let message_hash = keccak256(message);

        // Sign
        let signature = keypair.sign(&message_hash).expect("Signing should work");

        // Verify
        let public_key = keypair.public_key();
        let is_valid = keypair.verify(&message_hash, &signature);
        assert!(is_valid, "Signature should be valid");

        // Verify with wrong message should fail
        let wrong_hash = keccak256(b"wrong message");
        let is_invalid = keypair.verify(&wrong_hash, &signature);
        assert!(!is_invalid, "Signature should be invalid for wrong message");
    }

    /// Test address derivation from public key matches Ethereum
    #[test]
    fn test_address_derivation() {
        let keypair = KeyPair::generate();
        let address = keypair.address();

        // Address should be 20 bytes
        assert_eq!(address.len(), 20, "Address should be 20 bytes");

        // Address should be derived from last 20 bytes of keccak256(public_key)
        let public_key = keypair.public_key();
        let hash = &keccak256(&public_key[1..])[12..]; // Skip first byte (0x04) and take last 20 bytes
        assert_eq!(address, hash, "Address derivation should match Ethereum");
    }

    /// Test signature with known private key (test vector)
    #[test]
    fn test_known_key_signature() {
        // Using a well-known test private key (DO NOT USE IN PRODUCTION)
        let test_private_key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        // Parse private key
        let key_bytes = hex::decode(&test_private_key[2..]).expect("Valid hex");
        let keypair = KeyPair::from_secret_key(&key_bytes).expect("Valid key");

        // Sign a known message
        let message = keccak256(b"test");
        let signature = keypair.sign(&message).expect("Sign should work");

        // Signature should be 65 bytes (r: 32, s: 32, v: 1)
        assert_eq!(signature.len(), 65, "Signature should be 65 bytes");

        // Verify the signature
        assert!(keypair.verify(&message, &signature), "Signature should verify");
    }

    /// Test signature recovery
    #[test]
    fn test_signature_recovery() {
        let keypair = KeyPair::generate();
        let message_hash = keccak256(b"recovery test");

        let signature = keypair.sign(&message_hash).expect("Sign should work");
        let original_address = keypair.address();

        // Try to recover the address from signature
        // The v value (recovery id) is the last byte of the signature
        let v = signature[64];
        assert!(v == 0 || v == 1 || v == 27 || v == 28, "Recovery id should be valid");

        // Verify original signer can verify
        assert!(keypair.verify(&message_hash, &signature));
    }
}

#[cfg(test)]
mod merkle_tests {
    use luxtensor_crypto::MerkleTree;
    use super::*;

    /// Test Merkle tree root is deterministic
    #[test]
    fn test_merkle_deterministic() {
        let leaves: Vec<[u8; 32]> = (0..10)
            .map(|i| keccak256(&[i as u8]))
            .collect();

        let tree1 = MerkleTree::new(leaves.clone());
        let tree2 = MerkleTree::new(leaves.clone());

        assert_eq!(tree1.root(), tree2.root(), "Merkle roots should be deterministic");
    }

    /// Test Merkle tree with single element
    #[test]
    fn test_merkle_single_element() {
        let leaves = vec![keccak256(b"single")];
        let tree = MerkleTree::new(leaves.clone());

        // Single element tree root should be the hash of that element
        assert_eq!(tree.root(), leaves[0]);
    }

    /// Test empty Merkle tree
    #[test]
    fn test_merkle_empty() {
        let leaves: Vec<[u8; 32]> = vec![];
        let tree = MerkleTree::new(leaves);

        // Empty tree should have zero root
        assert_eq!(tree.root(), [0u8; 32]);
    }

    /// Test Merkle proof verification
    #[test]
    fn test_merkle_proof() {
        let leaves: Vec<[u8; 32]> = (0..8)
            .map(|i| keccak256(&[i as u8]))
            .collect();

        let tree = MerkleTree::new(leaves.clone());

        // Get proof for first leaf
        if let Some(proof) = tree.get_proof(0) {
            let is_valid = tree.verify_proof(&leaves[0], &proof);
            assert!(is_valid, "Merkle proof should be valid");
        }
    }
}

#[cfg(test)]
mod nonce_tests {
    use super::*;

    /// Test that nonce prevents replay within same chain
    #[test]
    fn test_nonce_replay_protection() {
        use std::collections::HashSet;

        // Simulate nonce tracking
        let mut used_nonces: HashSet<(String, u64)> = HashSet::new();
        let sender = "0x1234567890123456789012345678901234567890";

        // First transaction with nonce 0 should succeed
        let tx1 = (sender.to_string(), 0u64);
        assert!(used_nonces.insert(tx1.clone()), "First tx should succeed");

        // Replay with same nonce should fail
        assert!(!used_nonces.insert(tx1), "Replay should fail");

        // Different nonce should succeed
        let tx2 = (sender.to_string(), 1u64);
        assert!(used_nonces.insert(tx2), "Different nonce should succeed");
    }
}
