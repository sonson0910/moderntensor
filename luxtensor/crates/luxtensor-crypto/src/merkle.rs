use crate::{Hash, keccak256};

/// Proof element with positional encoding for correct verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofElement {
    /// The sibling hash at this level
    pub hash: Hash,
    /// True if this sibling is on the left side
    pub is_left: bool,
}

/// Merkle tree implementation
pub struct MerkleTree {
    _leaves: Vec<Hash>,  // Keep for future use
    nodes: Vec<Hash>,
}

impl MerkleTree {
    /// Create a new Merkle tree from leaves
    pub fn new(leaves: Vec<Hash>) -> Self {
        let nodes = Self::build_tree(&leaves);
        Self { _leaves: leaves, nodes }
    }

    /// Hash a leaf with 0x00 domain separator.
    ///
    /// SECURITY: Leaf hashes must be prefixed with 0x00 to distinguish them
    /// from internal node hashes (prefixed 0x01). Without this, an attacker
    /// can construct a 65-byte "leaf" that, when hashed, collides with an
    /// internal node hash `0x01 || left || right` — enabling second-preimage
    /// attacks that forge Merkle proofs.
    pub fn hash_leaf(data: &[u8]) -> Hash {
        let mut combined = Vec::with_capacity(1 + data.len());
        combined.push(0x00); // Leaf domain separator
        combined.extend_from_slice(data);
        keccak256(&combined)
    }

    /// Get the root hash
    pub fn root(&self) -> Hash {
        if self.nodes.is_empty() {
            [0u8; 32]
        } else {
            self.nodes[0]
        }
    }

    /// Build Merkle tree from leaves
    fn build_tree(leaves: &[Hash]) -> Vec<Hash> {
        if leaves.is_empty() {
            return vec![];
        }

        if leaves.len() == 1 {
            return vec![leaves[0]];
        }

        let mut current_level: Vec<Hash> = leaves.to_vec();
        let mut all_nodes = Vec::new();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    Self::hash_pair(&chunk[0], &chunk[1])
                } else {
                    Self::hash_pair(&chunk[0], &chunk[0])
                };
                next_level.push(hash);
                all_nodes.push(hash);
            }

            current_level = next_level;
        }

        // Root is the last node
        all_nodes.reverse();
        all_nodes
    }

    /// Hash two nodes together
    /// Hash two child hashes into a parent hash with domain separation.
    ///
    /// SECURITY: Prepends 0x01 byte to distinguish internal node hashes from
    /// leaf hashes (which should be prefixed with 0x00). This prevents
    /// second-preimage attacks where an attacker could create a 64-byte leaf
    /// that collides with an internal node hash.
    fn hash_pair(left: &Hash, right: &Hash) -> Hash {
        let mut combined = Vec::with_capacity(65);
        combined.push(0x01); // Internal node domain separator
        combined.extend_from_slice(left);
        combined.extend_from_slice(right);
        keccak256(&combined)
    }

    /// Generate Merkle proof for a leaf at given index
    pub fn get_proof(&self, index: usize) -> Vec<Hash> {
        if self._leaves.is_empty() || index >= self._leaves.len() {
            return vec![];
        }

        let mut proof = Vec::new();
        let mut current_index = index;
        let mut current_level = self._leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            // Get sibling node
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Add sibling to proof if it exists
            if sibling_index < current_level.len() {
                proof.push(current_level[sibling_index]);
            } else {
                // If no sibling, use the current node itself (for odd number of nodes)
                proof.push(current_level[current_index]);
            }

            // Build next level
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    Self::hash_pair(&chunk[0], &chunk[1])
                } else {
                    Self::hash_pair(&chunk[0], &chunk[0])
                };
                next_level.push(hash);
            }

            current_level = next_level;
            current_index /= 2;
        }

        proof
    }

    /// Generate Merkle proof with explicit position information (recommended)
    pub fn get_proof_with_positions(&self, index: usize) -> Vec<ProofElement> {
        if self._leaves.is_empty() || index >= self._leaves.len() {
            return vec![];
        }

        let mut proof = Vec::new();
        let mut current_index = index;
        let mut current_level = self._leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            // Get sibling node
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            // Add sibling to proof with position info
            if sibling_index < current_level.len() {
                proof.push(ProofElement {
                    hash: current_level[sibling_index],
                    is_left: current_index % 2 == 1, // sibling is left if current is at odd position
                });
            } else {
                // If no sibling (odd count), duplicate the current node
                proof.push(ProofElement {
                    hash: current_level[current_index],
                    is_left: false,
                });
            }

            // Build next level
            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    Self::hash_pair(&chunk[0], &chunk[1])
                } else {
                    Self::hash_pair(&chunk[0], &chunk[0])
                };
                next_level.push(hash);
            }

            current_level = next_level;
            current_index /= 2;
        }

        proof
    }

    /// Verify Merkle proof with explicit positional encoding (recommended)
    ///
    /// This is the correct implementation that uses explicit position
    /// information instead of relying on lexicographic ordering.
    pub fn verify_proof_with_positions(leaf: &Hash, proof: &[ProofElement], root: &Hash) -> bool {
        if proof.is_empty() {
            return leaf == root;
        }

        let mut current_hash = *leaf;

        for element in proof {
            current_hash = if element.is_left {
                // Sibling is on the left
                Self::hash_pair(&element.hash, &current_hash)
            } else {
                // Sibling is on the right
                Self::hash_pair(&current_hash, &element.hash)
            };
        }

        &current_hash == root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_single_leaf() {
        let leaves = vec![[1u8; 32]];
        let tree = MerkleTree::new(leaves);
        assert_eq!(tree.root(), [1u8; 32]);
    }

    #[test]
    fn test_merkle_tree_multiple_leaves() {
        let leaves = vec![
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            [4u8; 32],
        ];
        let tree = MerkleTree::new(leaves);
        let root = tree.root();
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn test_merkle_tree_empty() {
        let leaves = vec![];
        let tree = MerkleTree::new(leaves);
        assert_eq!(tree.root(), [0u8; 32]);
    }
}
