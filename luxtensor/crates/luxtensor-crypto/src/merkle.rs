use crate::{Hash, keccak256};

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
    fn hash_pair(left: &Hash, right: &Hash) -> Hash {
        let mut combined = Vec::with_capacity(64);
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
    
    /// Verify Merkle proof
    pub fn verify_proof(leaf: &Hash, proof: &[Hash], root: &Hash) -> bool {
        if proof.is_empty() {
            return leaf == root;
        }
        
        let mut current_hash = *leaf;
        
        // For each sibling in the proof, try both left and right positions
        for sibling in proof {
            // Try hashing with sibling on the left
            let hash_left = Self::hash_pair(sibling, &current_hash);
            // Try hashing with sibling on the right  
            let hash_right = Self::hash_pair(&current_hash, sibling);
            
            // For simplicity, we'll use the lexicographically smaller hash
            // In a production implementation, position should be encoded in the proof
            current_hash = if sibling < &current_hash {
                hash_left
            } else {
                hash_right
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
