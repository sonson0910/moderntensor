use sha3::{Digest, Keccak256};

pub type Hash = [u8; 32];

/// Keccak256 hash function (Ethereum-style)
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

    // ── Known Answer Tests (KATs) ──
    // These verify the exact output of each hash function against independently
    // computed reference values, catching accidental algorithm substitution
    // (e.g., SHA3-256 vs Keccak-256 — they differ by padding). (L-1)

    #[test]
    fn test_keccak256_kat() {
        // "hello world" → Keccak-256 (NOT SHA3-256)
        // Reference: https://emn178.github.io/online-tools/keccak_256.html
        let hash = keccak256(b"hello world");
        assert_eq!(
            hex::encode(hash),
            "47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad"
        );
    }

    #[test]
    fn test_keccak256_empty() {
        // Empty input → Keccak-256
        let hash = keccak256(b"");
        assert_eq!(
            hex::encode(hash),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[test]
    fn test_blake3_kat() {
        // "hello world" → BLAKE3
        // Reference: `b3sum <<< "hello world"` or BLAKE3 reference implementation
        let hash = blake3_hash(b"hello world");
        assert_eq!(
            hex::encode(hash),
            "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
        );
    }

    #[test]
    fn test_sha256_kat() {
        // "hello world" → SHA-256
        // Reference: `echo -n "hello world" | sha256sum`
        let hash = sha256(b"hello world");
        assert_eq!(
            hex::encode(hash),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_keccak256_length() {
        let hash = keccak256(b"test");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_blake3_length() {
        let hash = blake3_hash(b"test");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_sha256_length() {
        let hash = sha256(b"test");
        assert_eq!(hash.len(), 32);
    }
}
