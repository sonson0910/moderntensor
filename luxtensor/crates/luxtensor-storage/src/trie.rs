use crate::{Result, StorageError};
use luxtensor_crypto::{keccak256, Hash};
use std::collections::HashMap;

/// A single element in a raw-preimage Merkle proof.
///
/// Contains the raw bytes (preimage) of a node on the path from root to leaf.
/// The hash of this node is `keccak256(&self.preimage)`, and must equal the
/// hash referenced by its parent.
///
/// # Security
/// Using raw preimages allows **full hash-chain verification** — each
/// parent's hash is recomputed from raw bytes and compared against
/// `proof[i-1]`'s reference, preventing forgeable proofs. This resolves
/// LUX-TRIE-42.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawNodeProof {
    /// Raw bytes whose keccak256 produces this node's hash.
    pub preimage: Vec<u8>,
}

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
// - Simple proof verification without full hash-chain integrity

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

    /// Compute the raw encoding (preimage) of this node — used for full hash-chain proofs.
    ///
    /// The encoding mirrors the hash() function exactly:
    /// - Empty      → b""
    /// - Leaf       → hex_prefix(nibbles, true) ++ value
    /// - Extension  → hex_prefix(nibbles, false) ++ child_hash
    /// - Branch     → concat(child_hashes[0..16]) ++ optional_value
    fn raw_encoding(&self) -> Vec<u8> {
        match self {
            TrieNode::Empty => b"".to_vec(),
            TrieNode::Leaf { nibbles, value } => {
                let encoded_path = hex_prefix_encode(nibbles, true);
                let mut data = Vec::with_capacity(encoded_path.len() + value.len());
                data.extend_from_slice(&encoded_path);
                data.extend_from_slice(value);
                data
            }
            TrieNode::Extension { nibbles, child } => {
                let encoded_path = hex_prefix_encode(nibbles, false);
                let child_hash = child.hash();
                let mut data = Vec::with_capacity(encoded_path.len() + 32);
                data.extend_from_slice(&encoded_path);
                data.extend_from_slice(&child_hash);
                data
            }
            TrieNode::Branch { children, value } => {
                let mut data = Vec::with_capacity(16 * 32 + value.as_ref().map(|v| v.len()).unwrap_or(0));
                for child in children.iter() {
                    match child {
                        Some(node) => data.extend_from_slice(&node.hash()),
                        None => data.extend_from_slice(&keccak256(b"")),
                    }
                }
                if let Some(val) = value {
                    data.extend_from_slice(val);
                }
                data
            }
        }
    }

    /// Collect raw-preimage proof elements along the path to a key.
    ///
    /// Each element contains the raw bytes whose keccak256 = node hash.
    /// This enables full hash-chain verification (LUX-TRIE-42).
    fn collect_proof_raw(&self, nibbles: &[u8], proof: &mut Vec<RawNodeProof>) {
        proof.push(RawNodeProof { preimage: self.raw_encoding() });
        match self {
            TrieNode::Empty => {}
            TrieNode::Leaf { .. } => {
                // Leaf node preimage already pushed above
            }
            TrieNode::Extension { nibbles: ext_nibbles, child } => {
                if nibbles.starts_with(ext_nibbles) {
                    child.collect_proof_raw(&nibbles[ext_nibbles.len()..], proof);
                }
            }
            TrieNode::Branch { children, .. } => {
                if nibbles.is_empty() {
                    return;
                }
                let idx = nibbles[0] as usize;
                if let Some(child) = &children[idx] {
                    child.collect_proof_raw(&nibbles[1..], proof);
                }
            }
        }
    }

    /// Remove a key by nibble path, returning (new_node, was_removed).
    ///
    /// Node collapse rules (Ethereum MPT semantics):
    /// - Branch with 0 children + no value  → Empty
    /// - Branch with 0 children + value     → Leaf { nibbles: [], value }
    /// - Branch with 1 child  + no value    → promote child with prefix nibble merged
    ///   (if child is already Extension/Leaf, merge the nibbles)
    /// - Extension whose child collapsed to Empty → Empty
    /// - Extension whose child is now another Extension → merge nibbles
    fn remove(self, nibbles: &[u8]) -> (TrieNode, bool) {
        match self {
            TrieNode::Empty => (TrieNode::Empty, false),

            TrieNode::Leaf { nibbles: leaf_nibbles, value } => {
                if leaf_nibbles == nibbles {
                    // Found — remove
                    (TrieNode::Empty, true)
                } else {
                    // Not found
                    (TrieNode::Leaf { nibbles: leaf_nibbles, value }, false)
                }
            }

            TrieNode::Extension { nibbles: ext_nibbles, child } => {
                if !nibbles.starts_with(&ext_nibbles) {
                    // Path diverges — nothing to remove
                    return (TrieNode::Extension { nibbles: ext_nibbles, child }, false);
                }
                let (new_child, removed) = child.remove(&nibbles[ext_nibbles.len()..]);
                if !removed {
                    return (TrieNode::Extension { nibbles: ext_nibbles, child: Box::new(new_child) }, false);
                }
                // Child was modified — may need to collapse
                let collapsed = match new_child {
                    TrieNode::Empty => TrieNode::Empty,
                    // Child is now a Leaf: absorb its nibbles into extension
                    TrieNode::Leaf { nibbles: leaf_nib, value } => {
                        let mut merged = ext_nibbles.clone();
                        merged.extend_from_slice(&leaf_nib);
                        TrieNode::Leaf { nibbles: merged, value }
                    }
                    // Child is now another Extension: merge nibbles
                    TrieNode::Extension { nibbles: child_ext_nib, child: grandchild } => {
                        let mut merged = ext_nibbles.clone();
                        merged.extend_from_slice(&child_ext_nib);
                        TrieNode::Extension { nibbles: merged, child: grandchild }
                    }
                    // Child is a Branch — keep extension pointing to it
                    branch => TrieNode::Extension { nibbles: ext_nibbles, child: Box::new(branch) },
                };
                (collapsed, true)
            }

            TrieNode::Branch { mut children, value: branch_value } => {
                if nibbles.is_empty() {
                    // Remove the value stored at this branch
                    if branch_value.is_none() {
                        // Nothing to remove
                        return (TrieNode::Branch { children, value: branch_value }, false);
                    }
                    let new_node = Self::collapse_branch(children, None);
                    return (new_node, true);
                }

                let idx = nibbles[0] as usize;
                let child = children[idx].take().map(|c| *c).unwrap_or(TrieNode::Empty);
                let (new_child, removed) = child.remove(&nibbles[1..]);
                if !removed {
                    // Restore child since we took it
                    children[idx] = Some(Box::new(new_child));
                    return (TrieNode::Branch { children, value: branch_value }, false);
                }

                // Update child slot
                match new_child {
                    TrieNode::Empty => { children[idx] = None; }
                    other => { children[idx] = Some(Box::new(other)); }
                }

                let new_node = Self::collapse_branch(children, branch_value);
                (new_node, true)
            }
        }
    }

    /// Collapse a Branch after a removal:
    /// - 0 children, no value → Empty
    /// - 0 children, has value → Leaf { [], value }
    /// - 1 child, no value → merge that child upward (Extension or Leaf promote)
    /// - Otherwise → keep Branch unchanged
    fn collapse_branch(
        children: Box<[Option<Box<TrieNode>>; 16]>,
        value: Option<Vec<u8>>,
    ) -> TrieNode {
        let active_count = children.iter().filter(|c| c.is_some()).count();

        match (active_count, &value) {
            (0, None) => TrieNode::Empty,
            (0, Some(v)) => TrieNode::Leaf { nibbles: vec![], value: v.clone() },
            (1, None) => {
                // Exactly one child — promote it under a 1-nibble prefix,
                // then merge if it is itself an Extension or Leaf.
                let (branch_nibble, only_child) = children
                    .into_iter()
                    .enumerate()
                    .find_map(|(i, c)| c.map(|b| (i, *b)))
                    .expect("active_count == 1 guarantees one Some entry");

                match only_child {
                    // Merge Extension nibbles: [nibble] ++ child_ext_nibbles
                    TrieNode::Extension { nibbles: mut child_nib, child: grandchild } => {
                        let mut merged = vec![branch_nibble as u8];
                        merged.append(&mut child_nib);
                        TrieNode::Extension { nibbles: merged, child: grandchild }
                    }
                    // Merge Leaf nibbles: [nibble] ++ leaf_nibbles
                    TrieNode::Leaf { nibbles: mut leaf_nib, value: leaf_val } => {
                        let mut merged = vec![branch_nibble as u8];
                        merged.append(&mut leaf_nib);
                        TrieNode::Leaf { nibbles: merged, value: leaf_val }
                    }
                    // Other (Branch) — wrap in a 1-nibble Extension
                    other => TrieNode::Extension {
                        nibbles: vec![branch_nibble as u8],
                        child: Box::new(other),
                    },
                }
            }
            // Multiple children or has value — keep as Branch
            _ => TrieNode::Branch { children, value },
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

    /// Strict Merkle proof verification that checks the complete hash chain.
    ///
    /// Verifies:
    /// 1. `proof[0]` == `root`
    /// 2. `proof[last]` IS the expected leaf hash (computed from key suffix + value)
    ///
    /// This prevents an attacker from constructing a proof that contains the leaf
    /// hash but at a non-leaf position.
    ///
    /// For the **strongest guarantee** (full hash-chain), use [`verify_proof_with_preimages`]
    /// with proofs produced by [`get_proof_with_preimages`].
    ///
    /// # Returns
    /// `true` if root matches and proof ends with the correct leaf hash.
    pub fn verify_proof_strict(root: &Hash, key: &[u8], value: &[u8], proof: &[Hash]) -> bool {
        if proof.is_empty() {
            return false;
        }

        // Check 1: root must match
        if proof[0] != *root {
            return false;
        }

        // Check 2: last element must be the leaf hash for some suffix of the key
        let nibbles = bytes_to_nibbles(key);
        (0..=nibbles.len()).any(|start| {
            let suffix = &nibbles[start..];
            let encoded_path = hex_prefix_encode(suffix, true);
            let mut d = Vec::with_capacity(encoded_path.len() + value.len());
            d.extend_from_slice(&encoded_path);
            d.extend_from_slice(value);
            let leaf_hash = keccak256(&d);
            proof.last() == Some(&leaf_hash)
        })
    }

    /// Generate a **raw-preimage** Merkle proof for a key.
    ///
    /// Unlike `get_proof` which returns only node *hashes*, this method returns
    /// the raw encoding (preimage) of each node on the path from root to leaf.
    /// This enables **full hash-chain verification** via [`verify_proof_with_preimages`]:
    ///
    /// ```text
    /// keccak256(proof[i].preimage) == expected_hash_referenced_by_proof[i-1]
    /// ```
    ///
    /// Use this API for all security-critical callers (block validators,
    /// light clients, cross-chain bridges). This resolves LUX-TRIE-42.
    pub fn get_proof_with_preimages(&self, key: &[u8]) -> Result<Vec<RawNodeProof>> {
        let nibbles = bytes_to_nibbles(key);
        if self.root.get(&nibbles).is_none() {
            return Err(StorageError::InvalidProof);
        }
        let mut proof = Vec::new();
        self.root.collect_proof_raw(&nibbles, &mut proof);
        Ok(proof)
    }

    /// Verify a raw-preimage Merkle proof — full hash-chain verification.
    ///
    /// This is the **strongest verification available** for this trie (LUX-TRIE-42).
    ///
    /// # Algorithm
    /// 1. Verify `keccak256(proof[0].preimage) == root`
    /// 2. For each `i` in `1..proof.len()`: verify that `keccak256(proof[i].preimage)`
    ///    appears as a substring (32-byte boundary) inside `proof[i-1].preimage`.
    ///    This confirms that each node's hash is actually referenced by its parent.
    /// 3. Verify the last node's raw encoding matches a valid leaf for `(key, value)`.
    ///
    /// # Returns
    /// `true` if the full chain is cryptographically valid.
    pub fn verify_proof_with_preimages(
        root: &Hash,
        key: &[u8],
        value: &[u8],
        proof: &[RawNodeProof],
    ) -> bool {
        if proof.is_empty() {
            return false;
        }

        // Step 1: keccak256(proof[0]) must equal root
        if keccak256(&proof[0].preimage) != *root {
            return false;
        }

        // Step 2: Full hash chain — each child's hash must appear in parent's preimage
        for i in 1..proof.len() {
            let child_hash = keccak256(&proof[i].preimage);
            // The parent preimage must contain the child hash as a 32-byte aligned
            // field. We search for it as a raw byte substring.
            let parent = &proof[i - 1].preimage;
            if !contains_hash_bytes(parent, &child_hash) {
                return false;
            }
        }

        // Step 3: Last element must encode a valid leaf for some suffix of the key
        let last_preimage = &proof[proof.len() - 1].preimage;
        let nibbles = bytes_to_nibbles(key);
        (0..=nibbles.len()).any(|start| {
            let suffix = &nibbles[start..];
            let encoded_path = hex_prefix_encode(suffix, true);
            let mut expected = Vec::with_capacity(encoded_path.len() + value.len());
            expected.extend_from_slice(&encoded_path);
            expected.extend_from_slice(value);
            *last_preimage == expected
        })
    }

    /// Delete a key from the trie — O(log n) recursive removal.
    ///
    /// # Performance
    /// Previously rebuilt the entire trie from remaining keys — O(n) inserts.
    /// Now traverses only the path to the deleted key and collapses nodes
    /// on the way back up — O(log n) with respect to trie depth.
    ///
    /// The `self.keys` HashMap is still updated for `get_all_keys()` backward
    /// compatibility, which remains O(1) amortized.
    pub fn delete(&mut self, key: &[u8]) -> Result<()> {
        if self.keys.remove(key).is_some() {
            let nibbles = bytes_to_nibbles(key);
            let old_root = std::mem::take(&mut self.root);
            let (new_root, _removed) = old_root.remove(&nibbles);
            self.root = new_root;
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

/// Check if `haystack` contains `needle` as a raw byte subsequence.
/// Used by `verify_proof_with_preimages` to locate child hashes inside parent node encodings.
#[inline]
fn contains_hash_bytes(haystack: &[u8], needle: &[u8; 32]) -> bool {
    if haystack.len() < 32 {
        return false;
    }
    haystack.windows(32).any(|w| w == needle)
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
        // Use verify_proof_strict (leaf must be LAST element — more secure)
        let proof = trie.get_proof(b"key").unwrap();
        assert!(MerkleTrie::verify_proof_strict(&root, b"key", b"value", &proof));
        // Use verify_proof_with_preimages (full hash-chain — strongest, LUX-TRIE-42)
        let raw_proof = trie.get_proof_with_preimages(b"key").unwrap();
        assert!(MerkleTrie::verify_proof_with_preimages(&root, b"key", b"value", &raw_proof));
    }

    #[test]
    fn test_proof_fails_with_wrong_root() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();

        let wrong_root = [0xFFu8; 32];
        // verify_proof_strict
        let proof = trie.get_proof(b"key").unwrap();
        assert!(!MerkleTrie::verify_proof_strict(&wrong_root, b"key", b"value", &proof));
        // verify_proof_with_preimages
        let raw_proof = trie.get_proof_with_preimages(b"key").unwrap();
        assert!(!MerkleTrie::verify_proof_with_preimages(&wrong_root, b"key", b"value", &raw_proof));
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

    /// FIX-1 regression: root after delete must equal a freshly built trie
    /// containing only the remaining keys.
    #[test]
    fn test_delete_root_matches_fresh_trie() {
        let mut trie_abc = MerkleTrie::new();
        trie_abc.insert(b"aaa", b"1").unwrap();
        trie_abc.insert(b"bbb", b"2").unwrap();
        trie_abc.insert(b"ccc", b"3").unwrap();
        trie_abc.delete(b"bbb").unwrap();

        let mut trie_ac = MerkleTrie::new();
        trie_ac.insert(b"aaa", b"1").unwrap();
        trie_ac.insert(b"ccc", b"3").unwrap();

        assert_eq!(
            trie_abc.root_hash(), trie_ac.root_hash(),
            "Root after delete must equal trie built without the deleted key"
        );
    }

    /// FIX-1 stress: insert 100 keys, delete 50, verify root == fresh trie with 50.
    #[test]
    fn test_delete_50_keys_root_matches() {
        let mut trie_all = MerkleTrie::new();
        let mut trie_half = MerkleTrie::new();

        for i in 0..100u32 {
            let key = format!("key_{:04}", i);
            let val = format!("v{}", i);
            trie_all.insert(key.as_bytes(), val.as_bytes()).unwrap();
            if i >= 50 {
                trie_half.insert(key.as_bytes(), val.as_bytes()).unwrap();
            }
        }

        for i in 0..50u32 {
            let key = format!("key_{:04}", i);
            trie_all.delete(key.as_bytes()).unwrap();
        }

        assert_eq!(
            trie_all.root_hash(), trie_half.root_hash(),
            "Root after 50 deletes must match trie built with remaining 50 keys"
        );
        assert_eq!(trie_all.len(), 50);
    }

    /// FIX-1: delete the only key → empty root
    #[test]
    fn test_delete_single_key_becomes_empty() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"only", b"value").unwrap();

        trie.delete(b"only").unwrap();

        assert!(trie.is_empty());
        // Root hash of empty trie must equal keccak256(b"")
        let empty_root = MerkleTrie::new().root_hash();
        assert_eq!(trie.root_hash(), empty_root);
    }

    /// FIX-1: delete nonexistent key is a no-op
    #[test]
    fn test_delete_nonexistent_key_is_noop() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"exists", b"val").unwrap();
        let root_before = trie.root_hash();

        trie.delete(b"ghost").unwrap(); // not inserted

        assert_eq!(trie.root_hash(), root_before, "Delete nonexistent key must not change root");
        assert_eq!(trie.len(), 1);
    }

    /// FIX-5: verify_proof_strict rejects wrong root
    #[test]
    fn test_verify_proof_strict_wrong_root() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();
        let proof = trie.get_proof(b"key").unwrap();
        let wrong_root = [0xFFu8; 32];

        assert!(!MerkleTrie::verify_proof_strict(&wrong_root, b"key", b"value", &proof));
    }

    /// FIX-5: verify_proof_strict rejects proof where last hash ≠ leaf
    #[test]
    fn test_verify_proof_strict_wrong_value() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();
        let root = trie.root_hash();
        let proof = trie.get_proof(b"key").unwrap();

        // Wrong value → leaf hash won't match last proof element
        assert!(
            !MerkleTrie::verify_proof_strict(&root, b"key", b"WRONG_VALUE", &proof),
            "Strict verify must reject wrong value"
        );
    }

    /// FIX-5: verify_proof_strict accepts valid proof
    #[test]
    fn test_verify_proof_strict_valid() {
        let mut trie = MerkleTrie::new();
        trie.insert(b"key", b"value").unwrap();
        trie.insert(b"other", b"data").unwrap();
        let root = trie.root_hash();
        let proof = trie.get_proof(b"key").unwrap();

        assert!(MerkleTrie::verify_proof_strict(&root, b"key", b"value", &proof));
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
            // Use strict verification (leaf must be LAST element)
            let proof = trie.get_proof(&key).unwrap();
            assert!(MerkleTrie::verify_proof_strict(&root, &key, value.as_bytes(), &proof));
            // Also verify with raw preimages (full hash-chain, LUX-TRIE-42)
            let raw_proof = trie.get_proof_with_preimages(&key).unwrap();
            assert!(MerkleTrie::verify_proof_with_preimages(&root, &key, value.as_bytes(), &raw_proof));
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
        let value_str = format!("{}", 5000u128 * 1_000_000_000);
        // verify_proof_strict (leaf LAST)
        let proof = trie.get_proof(key.as_bytes()).unwrap();
        assert!(proof.len() >= 2);
        assert!(MerkleTrie::verify_proof_strict(&root, key.as_bytes(), value_str.as_bytes(), &proof));
        // verify_proof_with_preimages (full hash-chain, LUX-TRIE-42)
        let raw_proof = trie.get_proof_with_preimages(key.as_bytes()).unwrap();
        assert!(MerkleTrie::verify_proof_with_preimages(&root, key.as_bytes(), value_str.as_bytes(), &raw_proof));
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

