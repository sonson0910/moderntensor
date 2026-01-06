use secp256k1::{Secp256k1, SecretKey, PublicKey, Message};
use crate::{Hash, Result, CryptoError, keccak256};

/// Key pair for signing and verification
pub struct KeyPair {
    secret_key: SecretKey,
    public_key: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let mut rng = rand::thread_rng();
        let (secret_key, public_key) = secp.generate_keypair(&mut rng);
        
        Self {
            secret_key,
            public_key,
        }
    }
    
    /// Create key pair from secret key bytes
    pub fn from_secret(bytes: &[u8; 32]) -> Result<Self> {
        let secret_key = SecretKey::from_slice(bytes)
            .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);
        
        Ok(Self {
            secret_key,
            public_key,
        })
    }
    
    /// Sign a message hash
    pub fn sign(&self, message_hash: &Hash) -> [u8; 64] {
        let secp = Secp256k1::new();
        let message = Message::from_digest_slice(message_hash).unwrap();
        let signature = secp.sign_ecdsa(&message, &self.secret_key);
        let sig_bytes = signature.serialize_compact();
        
        let mut result = [0u8; 64];
        result.copy_from_slice(&sig_bytes);
        result
    }
    
    /// Get public key bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public_key.serialize_uncompressed().to_vec()
    }
    
    /// Derive address from public key (Ethereum-style)
    pub fn address(&self) -> [u8; 20] {
        let pubkey_bytes = self.public_key.serialize_uncompressed();
        // Skip first byte (0x04 prefix)
        let hash = keccak256(&pubkey_bytes[1..]);
        // Take last 20 bytes
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);
        address
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
    fn test_keypair_from_secret() {
        let secret = [1u8; 32];
        let keypair = KeyPair::from_secret(&secret).unwrap();
        let address = keypair.address();
        assert_eq!(address.len(), 20);
    }
    
    #[test]
    fn test_sign() {
        let keypair = KeyPair::generate();
        let message = [0u8; 32];
        let signature = keypair.sign(&message);
        assert_eq!(signature.len(), 64);
    }
}
