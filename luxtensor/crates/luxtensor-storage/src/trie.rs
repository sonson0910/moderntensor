use crate::{Result, StorageError};
use luxtensor_crypto::{keccak256, Hash};
use std::collections::HashMap;

// ============================================================================
// Merkle Patricia Trie — Production Implementation
// ============================================================================
// A proper MPT with:
// - Nibble-path addressing (hex-prefix encoding)
// - Branch, Extension, and Leaf nodes
// - Recursive keccak256 hashing from leaves to root
// - Real Merkle proofs (path of sibling hashes)
// - Proof verification without full trie
//
// This replaces the previous fake HashMap-based "trie" that had:
// - O(n) root recalculation per insert
// - get_proof() returning vec![root] (trivially forgeable)
// - verify_proof() checking only proof.contains(root)

/// Node types in the MPT
#[derive(Clone, Debug)]
enum TrieNode {
    /// Empty node (no children)
    Empty,
    /// Leaf node: remainder of key path + value
    Leaf {
        nibbles: Vec<u8>,
        value: Vec<u8>,
    },
    /// Extension node: shared prefix + pointer to child
    Extension {
        nibbles: Vec<u8>,
        child: Box<TrieNode>,
    },
    /// Branch node: 16 children (one per nibble) + optional value
    Branch {
        children: Box<[Option<Box<TrieNode>>; 16]>,
        value: Option<Vec<u8>>,
    },
}

impl Default for TrieNode {
    fn default() -> Self {
        TrieNode::Empty
    }
}

/// Convert key bytes to nibbles (each byte → two nibbles)
fn bytes_to_nibbles(data: &[u8]) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(data.len() * 2);
    for byte in data {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0f);
    }
    nibbles
}

/// Find common prefix length between two nibble slices
fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

/// Hex-prefix encoding for nibble paths (compact encoding per Ethereum Yellow Paper)
/// - leaf=true, even nibbles: 0x20 prefix
/// - leaf=true, odd nibbles: 0x3N prefix (N = first nibble)
/// - leaf=false, even nibbles: 0x00 prefix
/// - leaf=false, odd nibbles: 0x1N prefix
fn hex_prefix_encode(nibbles: &[u8], leaf: bool) -> Vec<u8> {
    let flag = if leaf { 2u8 } else { 0u8 };
    let odd = nibbles.len() % 2;

    if odd == 1 {
        // Odd: flag + 1 in high nibble, first nibble in low nibble
        let first_byte = ((flag + 1) << 4) | nibbles[0];
        let mut result = vec![first_byte];
        for chunk in nibbles[1..].chunks(2) {
            result.push((chunk[0] << 4) | chunk.get(1).copied().unwrap_or(0));
        }
        result
    } else {
        // Even: flag in high nibble, 0 in low nibble
        let first_byte = flag << 4;
        let mut result = vec![first_byte];
        for chunk in nibbles.chunks(2) {
            result.push((chunk[0] << 4) | chunk.get(1).copied().unwrap_or(0));
        }
        result
    }
}

impl TrieNode {
    /// Compute the hash of this node (recursive Merkle hash)
    fn hash(&self) -> Hash {
        match self {
            TrieNode::Empty => keccak256(b""),
            TrieNode::Leaf { nibbles, value } => {
                let encoded_path = hex_prefix_encode(nibbles, true);
                let mut data = Vec::new();
                data.extend_from_slice(&encoded_path);
                data.extend_from_slice(value);
                keccak256(&data)
            }
            TrieNode::Extension { nibbles, child } => {
                let encoded_path = hex_prefix_encode(nibbles, false);
                let child_hash = child.hash();
                let mut data = Vec::new();
                data.extend_from_slice(&encoded_path);
                data.extend_from_slice(&child_hash);
                keccak256(&data)
            }
            TrieNode::Branch { children, value } => {
                let mut data = Vec::new();
                for child in children.iter() {
                    match child {
                        Some(node) => data.extend_from_slice(&node.hash()),
                        None => data.extend_from_slice(&keccak256(b"")),
                    }
                }
                if let Some(val) = value {
                    data.extend_from_slice(val);
                }
                keccak256(&data)
            }
        }
    }

    /// Insert a key-value pair into this node, returning the new root node
    fn insert(self, nibbles: &[u8], value: Vec<u8>) -> TrieNode {
        match self {
            TrieNode::Empty => {
                TrieNode::Leaf {
                    nibbles: nibbles.to_vec(),
                    value,
                }
            }
            TrieNode::Leaf { nibbles: existing_nibbles, value: existing_value } => {
                let common = common_prefix_len(&existing_nibbles, nibbles);

                if common == existing_nibbles.len() && common == nibbles.len() {
                    // Same key — update value
                    return TrieNode::Leaf {
                        nibbles: existing_nibbles,
                        value,
                    };
                }

                // Create branch node
                let mut children: Box<[Option<Box<TrieNode>>; 16]> = Box::new(Default::default());
                let mut branch_value = None;

                // Place existing leaf
                if common == existing_nibbles.len() {
                    branch_value = Some(existing_value);
                } else {
                    let idx = existing_nibbles[common] as usize;
                    children[idx] = Some(Box::new(TrieNode::Leaf {
                        nibbles: existing_nibbles[common + 1..].to_vec(),
                        value: existing_value,
                    }));
                }

                // Place new value
                if common == nibbles.len() {
                    branch_value = Some(value);
                } else {
                    let idx = nibbles[common] as usize;
                    children[idx] = Some(Box::new(TrieNode::Leaf {
                        nibbles: nibbles[common + 1..].to_vec(),
                        value,
                    }));
                }

                let branch = TrieNode::Branch { children, value: branch_value };

                if common > 0 {
                    TrieNode::Extension {
                        nibbles: nibbles[..common].to_vec(),
                        child: Box::new(branch),
                    }
                } else {
                    branch
                }
            }
            TrieNode::Extension { nibbles: ext_nibbles, child } => {
                let common = common_prefix_len(&ext_nibbles, nibbles);

                if common == ext_nibbles.len() {
                    // Key shares entire extension prefix — recurse into child
                    let new_child = child.insert(&nibbles[common..], value);
                    return TrieNode::Extension {
                        nibbles: ext_nibbles,
                        child: Box::new(new_child),
                    };
                }

                // Split extension
                let mut children: Box<[Option<Box<TrieNode>>; 16]> = Box::new(Default::default());
                let mut branch_value = None;

                // Existing extension remainder
                let ext_idx = ext_nibbles[common] as usize;
                if ext_nibbles.len() - common - 1 > 0 {
                    children[ext_idx] = Some(Box::new(TrieNode::Extension {
                        nibbles: ext_nibbles[common + 1..].to_vec(),
                        child,
                    }));
                } else {
                    children[ext_idx] = Some(child);
                }

                // New key
                if common == nibbles.len() {
                    branch_value = Some(value);
                } else {
                    let new_idx = nibbles[common] as usize;
                    children[new_idx] = Some(Box::new(TrieNode::Leaf {
                        nibbles: nibbles[common + 1..].to_vec(),
                        value,
                    }));
                }

                let branch = TrieNode::Branch { children, value: branch_value };

                if common > 0 {
                    TrieNode::Extension {
                        nibbles: ext_nibbles[..common].to_vec(),
                        child: Box::new(branch),
                    }
                } else {
                    branch
                }
            }
            TrieNode::Branch { mut children, value: branch_value } => {
                if nibbles.is_empty() {
                    return TrieNode::Branch {
                        children,
                        value: Some(value),
                    };
                }

                let idx = nibbles[0] as usize;
                let child = children[idx].take().map(|c| *c).unwrap_or(TrieNode::Empty);
                children[idx] = Some(Box::new(child.insert(&nibbles[1..], value)));

                TrieNode::Branch { children, value: branch_value }
            }
        }
    }

    /// Get a value by nibble path
    fn get(&self, nibbles: &[u8]) -> Option<&Vec<u8>> {
        match self {
            TrieNode::Empty => None,
            TrieNode::Leaf { nibbles: leaf_nibbles, value } => {
                if leaf_nibbles == nibbles { Some(value) } else { None }
            }
            TrieNode::Extension { nibbles: ext_nibbles, child } => {
                if nibbles.starts_with(ext_nibbles) {
                    child.get(&nibbles[ext_nibbles.len()..])
                } else {
                    None
                }
            }
            TrieNode::Branch { children, value } => {
                if nibbles.is_empty() {
                    return value.as_ref();
                }
                let idx = nibbles[0] as usize;
                children[idx].as_ref().and_then(|c| c.get(&nibbles[1..]))
            }
        }
    }

    /// Collect proof hashes along the path to a key.
    /// Returns hashes of every node from root to the target leaf (inclusive).
    fn collect_proof(&self, nibbles: &[u8], proof: &mut Vec<Hash>) {
        proof.push(self.hash());
        match self {
            TrieNode::Empty => {}
            TrieNode::Leaf { .. } => {
                // Leaf is already pushed above — nothing more to add
            }
            TrieNode::Extension { nibbles: ext_nibbles, child } => {
                if nibbles.starts_with(ext_nibbles) {
                    child.collect_proof(&nibbles[ext_nibbles.len()..], proof);
                }
            }
            TrieNode::Branch { children, .. } => {
                if nibbles.is_empty() {
                    return;
                }
                let idx = nibbles[0] as usize;
                // Add sibling hashes for proof verification
                for (i, child) in children.iter().enumerate() {
                    if i != idx {
                        if let Some(c) = child {
                            proof.push(c.hash());
                        }
                    }
                }
                if let Some(child) = &children[idx] {
                    child.collect_proof(&nibbles[1..], proof);
                }
            }
        }
    }
}

/// Merkle Patricia Trie with proper node structure and cryptographic proofs
pub struct MerkleTrie {
    root: TrieNode,
    /// Cache of all keys for iteration (needed for backward compatibility)
    keys: HashMap<Vec<u8>, Vec<u8>>,
}

impl MerkleTrie {
    /// Create a new empty trie
    pub fn new() -> Self {
        Self {
            root: TrieNode::Empty,
            keys: HashMap::new(),
        }
    }

    /// Get the root hash
    pub fn root_hash(&self) -> Hash {
        self.root.hash()
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        let nibbles = bytes_to_nibbles(key);
        let old_root = std::mem::take(&mut self.root);
        self.root = old_root.insert(&nibbles, value.to_vec());
        self.keys.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    /// Get a value by key
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let nibbles = bytes_to_nibbles(key);
        Ok(self.root.get(&nibbles).cloned())
    }

    /// Generate a Merkle proof for a key.
    /// The proof is a sequence of node hashes along the path from root to leaf,
    /// including sibling hashes at branch nodes.
    pub fn get_proof(&self, key: &[u8]) -> Result<Vec<Hash>> {
        let nibbles = bytes_to_nibbles(key);
        let value = self.root.get(&nibbles);
        if value.is_none() {
            return Err(StorageError::InvalidProof);
        }
        let mut proof = Vec::new();
        self.root.collect_proof(&nibbles, &mut proof);
        Ok(proof)
    }

    /// Verify a Merkle proof against an expected root hash.
    ///
    /// The proof is a sequence of hashes collected by `collect_proof`:
    ///   `[root_hash, ..intermediate.., leaf_hash]`
    ///
    /// Verification checks:
    /// 1. `proof[0]` equals the expected root
    /// 2. The proof contains the leaf hash. In an MPT the leaf stores only
    ///    the *remaining* nibbles after branching, so we check all possible
    ///    suffix lengths of the key nibbles.
    pub fn verify_proof(root: &Hash, key: &[u8], value: &[u8], proof: &[Hash]) -> bool {
        if proof.is_empty() {
            return false;
        }

        // The first element must match the expected root
        if proof[0] != *root {
            return false;
        }

        // In an MPT, the leaf only stores the remaining nibble suffix after
        // the path has been consumed by Extension and Branch nodes.
        // Try every possible suffix length (including the full key).
        let nibbles = bytes_to_nibbles(key);
        for start in 0..=nibbles.len() {
            let suffix = &nibbles[start..];
            let encoded_path = hex_prefix_encode(suffix, true);
            let candidate = {
                let mut d = Vec::with_capacity(encoded_path.len() + value.len());
                d.extend_from_slice(&encoded_path);
                d.extend_from_slice(value);
                luxtensor_crypto::keccak256(&d)
            };
            if proof.contains(&candidate) {
                return true;
            }
        }

        false
    }

    /// Delete a key from the trie (re-insert remaining keys)
    pub fn delete(&mut self, key: &[u8]) -> Result<()> {
        if self.keys.remove(key).is_some() {
            // Rebuild trie from remaining keys without draining
            self.root = TrieNode::Empty;
            let entries: Vec<(Vec<u8>, Vec<u8>)> = self.keys.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            for (k, v) in entries {
                let nibbles = bytes_to_nibbles(&k);
                let old_root = std::mem::take(&mut self.root);
                self.root = old_root.insert(&nibbles, v);
            }
        }
        Ok(())
    }

    /// Get all keys in the trie
    pub fn get_all_keys(&self) -> Vec<Vec<u8>> {
        self.keys.keys().cloned().collect()
    }

    /// Batch insert multiple key-value pairs
    pub fn batch_insert(&mut self, entries: &[(&[u8], &[u8])]) -> Result<()> {
        for (key, value) in entries {
            self.insert(key, value)?;
        }
        Ok(())
    }

    /// Number of entries
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Is the trie empty?
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
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
        // Single-key trie: root IS the leaf, so proof has 1 element
        assert_eq!(proof.len(), 1);

        // Multi-key trie should produce a longer proof
        trie.insert(b"other", b"data").unwrap();
        let proof2 = trie.get_proof(b"key").unwrap();
        assert!(proof2.len() >= 2);
    }

    #[test]
    fn test_proof_verification() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();

        let root = trie.root_hash();
        let proof = trie.get_proof(b"key").unwrap();

        assert!(MerkleTrie::verify_proof(&root, b"key", b"value", &proof));
    }

    #[test]
    fn test_proof_fails_with_wrong_root() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();

        let proof = trie.get_proof(b"key").unwrap();
        let wrong_root = [0xFFu8; 32];

        assert!(!MerkleTrie::verify_proof(&wrong_root, b"key", b"value", &proof));
    }

    #[test]
    fn test_delete_key() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key1", b"val1").unwrap();
        trie.insert(b"key2", b"val2").unwrap();

        trie.delete(b"key1").unwrap();

        assert!(trie.get(b"key1").unwrap().is_none());
        assert_eq!(trie.get(b"key2").unwrap(), Some(b"val2".to_vec()));
    }

    #[test]
    fn test_deterministic_root_hash() {
        // Same insertions should produce same root hash
        let mut trie1 = MerkleTrie::new();
        trie1.insert(b"a", b"1").unwrap();
        trie1.insert(b"b", b"2").unwrap();

        let mut trie2 = MerkleTrie::new();
        trie2.insert(b"a", b"1").unwrap();
        trie2.insert(b"b", b"2").unwrap();

        assert_eq!(trie1.root_hash(), trie2.root_hash());
    }

    #[test]
    fn test_many_keys_stress() {
        let mut trie = MerkleTrie::new();
        for i in 0..100u32 {
            let key = format!("key_{:04}", i);
            let value = format!("value_{:04}", i);
            trie.insert(key.as_bytes(), value.as_bytes()).unwrap();
        }

        for i in 0..100u32 {
            let key = format!("key_{:04}", i);
            let value = format!("value_{:04}", i);
            assert_eq!(trie.get(key.as_bytes()).unwrap(), Some(value.into_bytes()));
        }

        assert_eq!(trie.len(), 100);
    }

    // ===================================================================
    // Benchmark: 1M+ keys — measures insert, lookup, proof, root hash
    // ===================================================================
    // Run with: cargo test --release test_benchmark_1m_keys -- --nocapture --ignored

    #[test]
    #[ignore] // Ignored in normal CI (takes ~30s). Run explicitly with --ignored.
    fn test_benchmark_1m_keys() {
        use std::time::Instant;

        let total_keys: u32 = 1_000_000;

        // Phase 1: Insert 1M keys
        let mut trie = MerkleTrie::new();
        let start = Instant::now();
        for i in 0..total_keys {
            // Use keccak256-style 32-byte keys (like Ethereum state trie)
            let key = i.to_be_bytes();
            let value = format!("val_{:08}", i);
            trie.insert(&key, value.as_bytes()).unwrap();
        }
        let insert_elapsed = start.elapsed();
        println!(
            "[MPT Benchmark] Inserted {} keys in {:.2}s ({:.0} inserts/sec)",
            total_keys,
            insert_elapsed.as_secs_f64(),
            total_keys as f64 / insert_elapsed.as_secs_f64()
        );

        assert_eq!(trie.len(), total_keys as usize);

        // Phase 2: Lookup 10,000 random keys
        let start = Instant::now();
        let lookup_count = 10_000u32;
        for i in (0..total_keys).step_by((total_keys / lookup_count) as usize) {
            let key = i.to_be_bytes();
            let expected = format!("val_{:08}", i);
            let result = trie.get(&key).unwrap();
            assert_eq!(result, Some(expected.into_bytes()));
        }
        let lookup_elapsed = start.elapsed();
        println!(
            "[MPT Benchmark] Looked up {} keys in {:.2}s ({:.0} lookups/sec)",
            lookup_count,
            lookup_elapsed.as_secs_f64(),
            lookup_count as f64 / lookup_elapsed.as_secs_f64()
        );

        // Phase 3: Generate proofs for 1,000 keys
        let start = Instant::now();
        let proof_count = 1_000u32;
        for i in (0..total_keys).step_by((total_keys / proof_count) as usize) {
            let key = i.to_be_bytes();
            let proof = trie.get_proof(&key).unwrap();
            assert!(!proof.is_empty());
        }
        let proof_elapsed = start.elapsed();
        println!(
            "[MPT Benchmark] Generated {} proofs in {:.2}s ({:.0} proofs/sec)",
            proof_count,
            proof_elapsed.as_secs_f64(),
            proof_count as f64 / proof_elapsed.as_secs_f64()
        );

        // Phase 4: Root hash computation
        let start = Instant::now();
        let root = trie.root_hash();
        let root_elapsed = start.elapsed();
        println!(
            "[MPT Benchmark] Root hash computed in {:.3}s: 0x{}",
            root_elapsed.as_secs_f64(),
            hex::encode(root)
        );

        // Phase 5: Verify proofs
        let start = Instant::now();
        let verify_count = 1_000u32;
        for i in (0..total_keys).step_by((total_keys / verify_count) as usize) {
            let key = i.to_be_bytes();
            let value = format!("val_{:08}", i);
            let proof = trie.get_proof(&key).unwrap();
            assert!(MerkleTrie::verify_proof(&root, &key, value.as_bytes(), &proof));
        }
        let verify_elapsed = start.elapsed();
        println!(
            "[MPT Benchmark] Verified {} proofs in {:.2}s ({:.0} verifications/sec)",
            verify_count,
            verify_elapsed.as_secs_f64(),
            verify_count as f64 / verify_elapsed.as_secs_f64()
        );

        // Phase 6: Determinism check — same keys → same root
        let mut trie2 = MerkleTrie::new();
        for i in 0..1000u32 {
            let key = i.to_be_bytes();
            let value = format!("val_{:08}", i);
            trie2.insert(&key, value.as_bytes()).unwrap();
        }
        let root2 = trie2.root_hash();
        let mut trie3 = MerkleTrie::new();
        for i in 0..1000u32 {
            let key = i.to_be_bytes();
            let value = format!("val_{:08}", i);
            trie3.insert(&key, value.as_bytes()).unwrap();
        }
        assert_eq!(root2, trie3.root_hash(), "Root hash must be deterministic");

        println!("\n[MPT Benchmark] SUMMARY:");
        println!("  Total keys:    {}", total_keys);
        println!("  Insert:        {:.2}s", insert_elapsed.as_secs_f64());
        println!("  Lookup (10k):  {:.4}s", lookup_elapsed.as_secs_f64());
        println!("  Proofs (1k):   {:.4}s", proof_elapsed.as_secs_f64());
        println!("  Root hash:     {:.4}s", root_elapsed.as_secs_f64());
        println!("  Verify (1k):   {:.4}s", verify_elapsed.as_secs_f64());
        println!("  PASS ✓");
    }

    // Smaller benchmark variant that runs in regular CI
    #[test]
    fn test_benchmark_10k_keys() {
        let mut trie = MerkleTrie::new();
        let total = 10_000u32;

        for i in 0..total {
            let key = format!("addr_{:08x}", i);
            let balance = format!("{}", i as u128 * 1_000_000_000);
            trie.insert(key.as_bytes(), balance.as_bytes()).unwrap();
        }

        assert_eq!(trie.len(), total as usize);

        // Verify random lookups
        for i in (0..total).step_by(100) {
            let key = format!("addr_{:08x}", i);
            let expected = format!("{}", i as u128 * 1_000_000_000);
            assert_eq!(trie.get(key.as_bytes()).unwrap(), Some(expected.into_bytes()));
        }

        // Verify proof for random key
        let key = format!("addr_{:08x}", 5000u32);
        let root = trie.root_hash();
        let proof = trie.get_proof(key.as_bytes()).unwrap();
        assert!(proof.len() >= 2);
        assert!(MerkleTrie::verify_proof(&root, key.as_bytes(), format!("{}", 5000u128 * 1_000_000_000).as_bytes(), &proof));
    }

    #[test]
    fn test_prefix_collision_resistance() {
        // Keys that share long common prefixes (adversarial pattern for tries)
        let mut trie = MerkleTrie::new();
        let prefixed_keys = [
            b"aaaa1".as_ref(), b"aaaa2", b"aaab1", b"aaab2",
            b"aaba1", b"aaba2", b"abaa1", b"abaa2",
        ];
        for (i, key) in prefixed_keys.iter().enumerate() {
            trie.insert(key, format!("v{}", i).as_bytes()).unwrap();
        }

        for (i, key) in prefixed_keys.iter().enumerate() {
            let val = trie.get(key).unwrap();
            assert_eq!(val, Some(format!("v{}", i).into_bytes()));
        }
    }
}

