use crate::{keccak256, CryptoError, Hash, Result};
use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1, SecretKey};
use zeroize::Zeroize;

/// A 20-byte Ethereum-style address derived from a public key.
/// This provides type safety over raw `[u8; 20]` arrays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CryptoAddress([u8; 20]);

impl CryptoAddress {
    /// Create a new `CryptoAddress` from a 20-byte array.
    pub fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Return a reference to the underlying 20-byte array.
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Consume self and return the underlying 20-byte array.
    pub fn into_bytes(self) -> [u8; 20] {
        self.0
    }
}

impl AsRef<[u8]> for CryptoAddress {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 20]> for CryptoAddress {
    fn from(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }
}

impl From<CryptoAddress> for [u8; 20] {
    fn from(addr: CryptoAddress) -> Self {
        addr.0
    }
}

impl std::fmt::Display for CryptoAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Key pair for signing and verification
///
/// # Security
/// The secret key is securely zeroed in memory when the KeyPair is dropped
/// to prevent secret material from lingering in memory.
pub struct KeyPair {
    secret_key: SecretKey,
    public_key: PublicKey,
}

impl Drop for KeyPair {
    fn drop(&mut self) {
        // SECURITY NOTE: secp256k1::SecretKey stores the 32-byte scalar internally
        // and `secret_bytes()` returns a COPY, so zeroizing that copy does not clear
        // the actual key material inside SecretKey.
        //
        // To properly zeroize, we overwrite the SecretKey in-place with a dummy value.
        // SecretKey::from_slice will accept any valid 32-byte scalar (non-zero, < curve order).
        // We overwrite self.secret_key with a well-known dummy to obliterate the real key.
        let dummy = [0x01u8; 32]; // Valid scalar (1)
        if let Ok(dummy_key) = SecretKey::from_slice(&dummy) {
            self.secret_key = dummy_key;
        }
        // Also zeroize the copy for defense in depth
        let mut secret_bytes = self.secret_key.secret_bytes();
        secret_bytes.zeroize();
    }
}

impl Clone for KeyPair {
    fn clone(&self) -> Self {
        // SAFETY: SecretKey::from_slice is infallible for bytes that were
        // already validated as a valid secp256k1 scalar. The only way this
        // can fail is memory corruption, in which case panic is appropriate.
        let sk = SecretKey::from_slice(&self.secret_key.secret_bytes())
            .unwrap_or_else(|_| panic!("FATAL: KeyPair clone failed — possible memory corruption"));
        let pk = self.public_key; // PublicKey is Copy
        Self { secret_key: sk, public_key: pk }
    }
}

impl KeyPair {
    /// Generate a new random key pair
    ///
    /// # Security
    /// Uses `OsRng` (OS-provided CSPRNG) directly for key generation.
    /// This is the recommended practice per all major crypto libraries.
    pub fn generate() -> Self {
        let secp = Secp256k1::new();
        let (secret_key, public_key) = secp.generate_keypair(&mut rand::rngs::OsRng);

        Self { secret_key, public_key }
    }

    /// Create key pair from secret key bytes
    pub fn from_secret(bytes: &[u8; 32]) -> Result<Self> {
        let secret_key =
            SecretKey::from_slice(bytes).map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        Ok(Self { secret_key, public_key })
    }

    /// Sign a message hash
    /// Returns signature or error if message hash is invalid
    ///
    /// # Security
    /// Enforces low-S normalization to prevent ECDSA signature malleability.
    /// Without this, `(r, s)` and `(r, n-s)` are both valid signatures,
    /// which can enable transaction malleability attacks.
    pub fn sign(&self, message_hash: &Hash) -> Result<[u8; 64]> {
        let secp = Secp256k1::new();
        let message = Message::from_digest_slice(message_hash)
            .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;
        let mut signature = secp.sign_ecdsa(&message, &self.secret_key);
        // Enforce low-S to prevent signature malleability (BIP-62)
        signature.normalize_s();
        let sig_bytes = signature.serialize_compact();

        let mut result = [0u8; 64];
        result.copy_from_slice(&sig_bytes);
        Ok(result)
    }

    /// Get public key bytes
    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.public_key.serialize_uncompressed().to_vec()
    }

    /// Derive address from public key (Ethereum-style)
    pub fn address(&self) -> CryptoAddress {
        let pubkey_bytes = self.public_key.serialize_uncompressed();
        // Skip first byte (0x04 prefix)
        let hash = keccak256(&pubkey_bytes[1..]);
        // Take last 20 bytes
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..]);
        CryptoAddress(address)
    }
}

/// Verify a signature against a message hash and public key
pub fn verify_signature(
    message_hash: &Hash,
    signature: &[u8; 64],
    public_key: &[u8],
) -> Result<bool> {
    let secp = Secp256k1::new();

    // Parse the signature
    let sig = Signature::from_compact(signature)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;

    // Reject high-S signatures to prevent malleability (BIP-62 / EIP-2)
    {
        let mut normalized = sig;
        normalized.normalize_s();
        if normalized != sig {
            // Signature had high-S; reject it
            return Ok(false);
        }
    }

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
pub fn recover_public_key(
    message_hash: &Hash,
    signature: &[u8; 64],
    recovery_id: u8,
) -> Result<Vec<u8>> {
    let secp = Secp256k1::new();

    // Parse the signature
    let _sig = Signature::from_compact(signature)
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
    let pubkey = secp
        .recover_ecdsa(&message, &rec_sig)
        .map_err(|e| CryptoError::Secp256k1Error(e.to_string()))?;

    Ok(pubkey.serialize_uncompressed().to_vec())
}

/// Recover the 20-byte Ethereum-style address from a message hash and signature.
///
/// Tries recovery_id 0 first, then 1.  Returns the first address that
/// successfully recovers.  The signature must be exactly 64 bytes (r‖s)
/// or 65 bytes (r‖s‖v) — if 65 bytes, the last byte is used as the
/// recovery id directly.
pub fn recover_address(message_hash: &Hash, signature: &[u8]) -> Result<CryptoAddress> {
    let (sig_bytes, recovery_ids): ([u8; 64], Vec<u8>) = if signature.len() == 65 {
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&signature[..64]);
        (sig, vec![signature[64]])
    } else if signature.len() == 64 {
        let mut sig = [0u8; 64];
        sig.copy_from_slice(signature);
        (sig, vec![0, 1])
    } else {
        return Err(CryptoError::Secp256k1Error(format!(
            "Invalid signature length: expected 64 or 65, got {}",
            signature.len()
        )));
    };

    for rid in recovery_ids {
        if let Ok(pubkey) = recover_public_key(message_hash, &sig_bytes, rid) {
            if let Ok(addr) = address_from_public_key(&pubkey) {
                return Ok(addr);
            }
        }
    }

    Err(CryptoError::Secp256k1Error("Failed to recover address with any recovery id".into()))
}

/// Derive address from public key bytes
pub fn address_from_public_key(public_key: &[u8]) -> Result<CryptoAddress> {
    if public_key.len() != 65 || public_key[0] != 0x04 {
        return Err(CryptoError::InvalidPublicKey);
    }

    // Skip first byte (0x04 prefix)
    let hash = keccak256(&public_key[1..]);

    // Take last 20 bytes
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);

    Ok(CryptoAddress(address))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = KeyPair::generate();
        let address = keypair.address();
        assert_eq!(address.as_bytes().len(), 20);
    }

    #[test]
    fn test_keypair_from_secret() {
        let secret = [1u8; 32];
        let keypair = KeyPair::from_secret(&secret).unwrap();
        let address = keypair.address();
        assert_eq!(address.as_bytes().len(), 20);
    }

    #[test]
    fn test_sign() {
        let keypair = KeyPair::generate();
        let message = [0u8; 32];
        let signature = keypair.sign(&message).unwrap();
        assert_eq!(signature.len(), 64);
    }
}
