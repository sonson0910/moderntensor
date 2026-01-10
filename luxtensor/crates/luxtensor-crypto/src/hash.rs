use sha3::{Keccak256, Digest};

pub type Hash = [u8; 32];

/// Keccak256 hash function
pub fn keccak256(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Blake3 hash function
pub fn blake3_hash(data: &[u8]) -> Hash {
    let hash = blake3::hash(data);
    *hash.as_bytes()
}

/// SHA256 hash function
pub fn sha256(data: &[u8]) -> Hash {
    use sha2::Sha256;
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_keccak256() {
        let data = b"hello world";
        let hash = keccak256(data);
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_blake3() {
        let data = b"hello world";
        let hash = blake3_hash(data);
        assert_eq!(hash.len(), 32);
    }
    
    #[test]
    fn test_sha256() {
        let data = b"hello world";
        let hash = sha256(data);
        assert_eq!(hash.len(), 32);
    }
}
