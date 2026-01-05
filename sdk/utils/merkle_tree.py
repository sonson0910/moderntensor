# sdk/utils/merkle_tree.py
"""
Production-ready Merkle tree implementation for ModernTensor Layer 1.

This module provides a complete binary Merkle tree with:
- Proof generation and verification
- Efficient tree construction
- Support for variable leaf counts
- SHA256 hashing
"""

import hashlib
from typing import List, Optional, Tuple
from dataclasses import dataclass


@dataclass
class MerkleProof:
    """Merkle proof for a leaf."""
    leaf_index: int
    leaf_hash: bytes
    proof: List[Tuple[bytes, bool]]  # (hash, is_right_sibling)
    root: bytes


class MerkleTree:
    """
    Production-ready binary Merkle tree implementation.
    
    Constructs a binary tree from leaf hashes and supports:
    - Root calculation
    - Proof generation for any leaf
    - Proof verification
    """
    
    def __init__(self, leaves: List[bytes]):
        """
        Initialize Merkle tree from leaf hashes.
        
        Args:
            leaves: List of leaf hashes (pre-hashed data)
        """
        if not leaves:
            raise ValueError("Cannot create Merkle tree from empty leaves")
        
        self.leaves = leaves.copy()
        self.tree = self._build_tree()
        # Root is the single element in the top level
        self.root = self.tree[0][0] if self.tree and self.tree[0] else b'\x00' * 32
    
    def _hash_pair(self, left: bytes, right: bytes) -> bytes:
        """Hash a pair of nodes."""
        return hashlib.sha256(left + right).digest()
    
    def _build_tree(self) -> List[bytes]:
        """
        Build the complete binary tree.
        
        Returns:
            List representing tree levels (root at index 0)
        """
        if not self.leaves:
            return []
        
        # Start with leaves as the bottom level
        current_level = self.leaves.copy()
        tree = [current_level]
        
        # Build tree bottom-up
        while len(current_level) > 1:
            next_level = []
            
            # Process pairs
            for i in range(0, len(current_level), 2):
                left = current_level[i]
                
                # Handle odd number of nodes - duplicate last node
                if i + 1 < len(current_level):
                    right = current_level[i + 1]
                else:
                    right = left
                
                parent = self._hash_pair(left, right)
                next_level.append(parent)
            
            tree.insert(0, next_level)  # Insert at beginning (root will be at index 0)
            current_level = next_level
        
        return tree
    
    def get_root(self) -> bytes:
        """Get the Merkle root."""
        return self.root
    
    def get_proof(self, leaf_index: int) -> MerkleProof:
        """
        Generate Merkle proof for a leaf.
        
        Args:
            leaf_index: Index of the leaf (0-based)
            
        Returns:
            MerkleProof object containing the proof path
            
        Raises:
            IndexError: If leaf_index is out of range
        """
        if leaf_index < 0 or leaf_index >= len(self.leaves):
            raise IndexError(f"Leaf index {leaf_index} out of range [0, {len(self.leaves)})")
        
        proof = []
        current_index = leaf_index
        
        # Traverse from leaf to root
        for level_idx in range(len(self.tree) - 1, 0, -1):
            level = self.tree[level_idx]
            
            # Determine sibling
            if current_index % 2 == 0:
                # Current node is left child
                sibling_index = current_index + 1
                is_right_sibling = True
            else:
                # Current node is right child
                sibling_index = current_index - 1
                is_right_sibling = False
            
            # Get sibling hash
            if sibling_index < len(level):
                sibling_hash = level[sibling_index]
            else:
                # No sibling (odd number of nodes), use current node
                sibling_hash = level[current_index]
            
            proof.append((sibling_hash, is_right_sibling))
            
            # Move to parent level
            current_index = current_index // 2
        
        return MerkleProof(
            leaf_index=leaf_index,
            leaf_hash=self.leaves[leaf_index],
            proof=proof,
            root=self.root
        )
    
    @staticmethod
    def verify_proof(proof: MerkleProof) -> bool:
        """
        Verify a Merkle proof.
        
        Args:
            proof: MerkleProof to verify
            
        Returns:
            True if proof is valid
        """
        current_hash = proof.leaf_hash
        
        # Traverse proof path
        for sibling_hash, is_right_sibling in proof.proof:
            if is_right_sibling:
                # Sibling is on the right
                current_hash = hashlib.sha256(current_hash + sibling_hash).digest()
            else:
                # Sibling is on the left
                current_hash = hashlib.sha256(sibling_hash + current_hash).digest()
        
        return current_hash == proof.root
    
    @staticmethod
    def create_from_data(data_list: List[bytes]) -> 'MerkleTree':
        """
        Create Merkle tree from raw data (will hash each item).
        
        Args:
            data_list: List of raw data bytes
            
        Returns:
            MerkleTree instance
        """
        leaves = [hashlib.sha256(data).digest() for data in data_list]
        return MerkleTree(leaves)
    
    def get_leaf(self, index: int) -> bytes:
        """Get leaf hash by index."""
        return self.leaves[index]
    
    def get_num_leaves(self) -> int:
        """Get number of leaves."""
        return len(self.leaves)
    
    def __repr__(self) -> str:
        return f"MerkleTree(leaves={len(self.leaves)}, root={self.root.hex()[:16]}...)"


class MerkleTreeBuilder:
    """Helper class to build Merkle trees incrementally."""
    
    def __init__(self):
        """Initialize builder."""
        self.leaves: List[bytes] = []
    
    def add_leaf(self, data: bytes) -> None:
        """Add a leaf (raw data, will be hashed)."""
        leaf_hash = hashlib.sha256(data).digest()
        self.leaves.append(leaf_hash)
    
    def add_leaf_hash(self, leaf_hash: bytes) -> None:
        """Add a pre-hashed leaf."""
        self.leaves.append(leaf_hash)
    
    def build(self) -> MerkleTree:
        """Build the Merkle tree."""
        if not self.leaves:
            raise ValueError("Cannot build tree with no leaves")
        return MerkleTree(self.leaves)
    
    def reset(self) -> None:
        """Reset the builder."""
        self.leaves.clear()
