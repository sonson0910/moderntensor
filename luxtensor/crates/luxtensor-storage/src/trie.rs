use crate::{Result, StorageError};
use luxtensor_crypto::{keccak256, Hash};
use std::collections::HashMap;

/// Simplified Merkle Patricia Trie using a HashMap backend
/// Note: This is a simplified implementation for Phase 4.
/// A full production implementation would use an actual trie structure.
pub struct MerkleTrie {
    data: HashMap<Vec<u8>, Vec<u8>>,
    root: Hash,
}

impl MerkleTrie {
    /// Create a new empty trie
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            root: keccak256(b""),
        }
    }

    /// Get the root hash
    pub fn root_hash(&self) -> Hash {
        self.root
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        self.data.insert(key.to_vec(), value.to_vec());
        self.update_root();
        Ok(())
    }

    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.data.get(key).cloned())
    }

    /// Update the root hash based on current data
    fn update_root(&mut self) {
        if self.data.is_empty() {
            self.root = keccak256(b"");
            return;
        }

        // Collect all key-value pairs and sort by key
        let mut items: Vec<_> = self.data.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));

        // Hash all items together
        let mut data = Vec::new();
        for (key, value) in items {
            data.extend_from_slice(key);
            data.extend_from_slice(value);
        }

        self.root = keccak256(&data);
    }

    /// Generate a Merkle proof for a key
    pub fn get_proof(&self, key: &[u8]) -> Result<Vec<Hash>> {
        // Simplified proof: just return the root hash if key exists
        if self.data.contains_key(key) {
            Ok(vec![self.root])
        } else {
            Err(StorageError::InvalidProof)
        }
    }

    /// Verify a Merkle proof
    pub fn verify_proof(root: &Hash, _key: &[u8], _value: &[u8], proof: &[Hash]) -> bool {
        if proof.is_empty() {
            return false;
        }
        
        // Simplified verification - just check that proof contains root
        proof.contains(root)
    }
}

impl Default for MerkleTrie {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_creation() {
        let trie = MerkleTrie::new();
        assert_ne!(trie.root_hash(), [0u8; 32]);
    }

    #[test]
    fn test_insert_and_get() {
        let mut trie = MerkleTrie::new();
        
        let key = b"hello";
        let value = b"world";
        
        trie.insert(key, value).unwrap();
        
        let retrieved = trie.get(key).unwrap();
        assert_eq!(retrieved, Some(value.to_vec()));
    }

    #[test]
    fn test_get_nonexistent() {
        let trie = MerkleTrie::new();
        
        let result = trie.get(b"nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_value() {
        let mut trie = MerkleTrie::new();
        
        trie.insert(b"key", b"value1").unwrap();
        trie.insert(b"key", b"value2").unwrap();
        
        let result = trie.get(b"key").unwrap();
        assert_eq!(result, Some(b"value2".to_vec()));
    }

    #[test]
    fn test_multiple_keys() {
        let mut trie = MerkleTrie::new();
        
        trie.insert(b"key1", b"value1").unwrap();
        trie.insert(b"key2", b"value2").unwrap();
        trie.insert(b"key3", b"value3").unwrap();
        
        assert_eq!(trie.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(trie.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(trie.get(b"key3").unwrap(), Some(b"value3".to_vec()));
    }

    #[test]
    fn test_root_changes_on_insert() {
        let mut trie = MerkleTrie::new();
        let root1 = trie.root_hash();
        
        trie.insert(b"key", b"value").unwrap();
        let root2 = trie.root_hash();
        
        assert_ne!(root1, root2);
    }

    #[test]
    fn test_proof_generation() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();
        
        let proof = trie.get_proof(b"key").unwrap();
        assert!(!proof.is_empty());
    }

    #[test]
    fn test_proof_verification() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();
        
        let root = trie.root_hash();
        let proof = trie.get_proof(b"key").unwrap();
        
        assert!(MerkleTrie::verify_proof(&root, b"key", b"value", &proof));
    }
}

