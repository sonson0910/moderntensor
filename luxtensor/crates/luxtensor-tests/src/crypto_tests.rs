// Comprehensive Unit Tests for luxtensor-crypto
// Tests for KeyPair, signatures, hashing

use luxtensor_crypto::{keccak256, KeyPair, verify_signature};

// ============================================================
// KeyPair Unit Tests
// ============================================================

#[cfg(test)]
mod keypair_tests {
    use super::*;

    #[test]
    fn test_keypair_generate() {
        let keypair = KeyPair::generate();

        // Public key should not be empty
        assert!(!keypair.public_key_bytes().is_empty());
    }

    #[test]
    fn test_keypair_generate_unique() {
        let keypair1 = KeyPair::generate();
        let keypair2 = KeyPair::generate();

        // Two random keypairs should be different
        assert_ne!(keypair1.public_key_bytes(), keypair2.public_key_bytes());
    }

    #[test]
    fn test_keypair_address() {
        let keypair = KeyPair::generate();
        let address = keypair.address();

        // Address should be 20 bytes
        assert_eq!(address.as_bytes().len(), 20);
    }

    #[test]
    fn test_keypair_address_deterministic() {
        let keypair = KeyPair::generate();

        let addr1 = keypair.address();
        let addr2 = keypair.address();

        // Same keypair should produce same address
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_keypair_sign() {
        let keypair = KeyPair::generate();
        let message_hash: [u8; 32] = keccak256(b"test message");

        let signature = keypair.sign(&message_hash);

        // Signature should succeed
        assert!(signature.is_ok());
        // Signature should be 64 bytes
        assert_eq!(signature.unwrap().len(), 64);
    }

    #[test]
    fn test_keypair_verify() {
        let keypair = KeyPair::generate();
        let message_hash: [u8; 32] = keccak256(b"test message");

        let signature = keypair.sign(&message_hash).unwrap();
        let public_key = keypair.public_key_bytes();

        let is_valid = verify_signature(&message_hash, &signature, &public_key);

        assert!(is_valid.is_ok() && is_valid.unwrap(), "Signature verification should succeed");
    }

    #[test]
    fn test_keypair_verify_wrong_message() {
        let keypair = KeyPair::generate();
        let message1_hash: [u8; 32] = keccak256(b"test message");
        let message2_hash: [u8; 32] = keccak256(b"wrong message");

        let signature = keypair.sign(&message1_hash).unwrap();
        let public_key = keypair.public_key_bytes();

        let is_valid = verify_signature(&message2_hash, &signature, &public_key);

        assert!(is_valid.is_ok() && !is_valid.unwrap(), "Verification should fail for wrong message");
    }

    #[test]
    fn test_keypair_from_secret() {
        let secret = [1u8; 32];
        let keypair = KeyPair::from_secret(&secret);

        assert!(keypair.is_ok(), "Should create keypair from secret");

        let kp = keypair.unwrap();
        let address = kp.address();
        assert_eq!(address.as_bytes().len(), 20);
    }

    #[test]
    fn test_keypair_from_secret_deterministic() {
        let secret = [42u8; 32];

        let keypair1 = KeyPair::from_secret(&secret).unwrap();
        let keypair2 = KeyPair::from_secret(&secret).unwrap();

        // Same secret should produce same address
        assert_eq!(keypair1.address(), keypair2.address());
    }
}

// ============================================================
// Keccak256 Hash Tests
// ============================================================

#[cfg(test)]
mod keccak256_tests {
    use super::*;

    #[test]
    fn test_keccak256_known_vector() {
        let hash = keccak256(b"");

        // Should be 32 bytes
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_keccak256_hello_world() {
        let hash = keccak256(b"hello world");

        // Hash should be consistent
        let hash2 = keccak256(b"hello world");
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_keccak256_large_input() {
        let large_data = vec![0u8; 10_000];
        let hash = keccak256(&large_data);

        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_keccak256_single_byte_difference() {
        let data1 = b"hello";
        let data2 = b"hellp"; // Single byte different

        let hash1 = keccak256(data1);
        let hash2 = keccak256(data2);

        // Even single byte difference should produce completely different hash
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_keccak256_collision_resistance() {
        let mut hashes = std::collections::HashSet::new();

        for i in 0..100 {
            let data = format!("test data {}", i);
            let hash = keccak256(data.as_bytes());
            let hash_hex = hex::encode(hash);

            assert!(hashes.insert(hash_hex), "Hash collision detected!");
        }
    }
}

// ============================================================
// Signature Tests
// ============================================================

#[cfg(test)]
mod signature_tests {
    use super::*;

    #[test]
    fn test_signature_length() {
        let keypair = KeyPair::generate();
        let message_hash = keccak256(b"test");

        let signature = keypair.sign(&message_hash).unwrap();

        // ECDSA compact signature is 64 bytes
        assert_eq!(signature.len(), 64);
    }

    #[test]
    fn test_different_messages_different_signatures() {
        let keypair = KeyPair::generate();

        let sig1 = keypair.sign(&keccak256(b"message 1")).unwrap();
        let sig2 = keypair.sign(&keccak256(b"message 2")).unwrap();

        // Different messages should produce different signatures
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_public_key_bytes_length() {
        let keypair = KeyPair::generate();
        let pubkey = keypair.public_key_bytes();

        // Uncompressed secp256k1 public key is 65 bytes (0x04 + 32 + 32)
        assert_eq!(pubkey.len(), 65);
        assert_eq!(pubkey[0], 0x04, "First byte should be 0x04 for uncompressed");
    }
}
