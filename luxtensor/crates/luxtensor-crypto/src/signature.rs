use secp256k1::{Secp256k1, SecretKey, PublicKey, Message, ecdsa::Signature};
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

/// Verify a signature against a message hash and public key
pub fn verify_signature(message_hash: &Hash, signature: &[u8; 64], public_key: &[u8]) -> Result<bool> {
    let secp = Secp256k1::new();
    
    // Parse the signature
    let sig = Signature::from_compact(signature)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Parse the public key
    let pubkey = PublicKey::from_slice(public_key)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Parse the message
    let message = Message::from_digest_slice(message_hash)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Verify the signature
    match secp.verify_ecdsa(&message, &sig, &pubkey) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Recover public key from signature
pub fn recover_public_key(message_hash: &Hash, signature: &[u8; 64], recovery_id: u8) -> Result<Vec<u8>> {
    let secp = Secp256k1::new();
    
    // Parse the signature
    let sig = Signature::from_compact(signature)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Parse the message
    let message = Message::from_digest_slice(message_hash)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Create recovery ID
    let rec_id = secp256k1::ecdsa::RecoveryId::from_i32(recovery_id as i32)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Create recoverable signature
    let rec_sig = secp256k1::ecdsa::RecoverableSignature::from_compact(signature, rec_id)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    // Recover the public key
    let pubkey = secp.recover_ecdsa(&message, &rec_sig)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
    
    Ok(pubkey.serialize_uncompressed().to_vec())
}

/// Derive address from public key bytes
pub fn address_from_public_key(public_key: &[u8]) -> Result<[u8; 20]> {
    if public_key.len() != 65 || public_key[0] != 0x04 {
        return Err(CryptoError::InvalidPublicKey);
    }
    
    // Skip first byte (0x04 prefix)
    let hash = keccak256(&public_key[1..]);
    
    // Take last 20 bytes
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    
    Ok(address)
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
