# tests/utils/test_merkle_tree.py
"""
Tests for production Merkle tree implementation.
"""

import pytest
import hashlib
from sdk.utils.merkle_tree import MerkleTree, MerkleProof, MerkleTreeBuilder


class TestMerkleTree:
    """Test MerkleTree functionality."""
    
    def test_create_tree_single_leaf(self):
        """Test creating tree with single leaf."""
        leaf = hashlib.sha256(b"test").digest()
        tree = MerkleTree([leaf])
        
        assert tree.get_num_leaves() == 1
        assert tree.get_root() == leaf
    
    def test_create_tree_multiple_leaves(self):
        """Test creating tree with multiple leaves."""
        leaves = [
            hashlib.sha256(b"leaf1").digest(),
            hashlib.sha256(b"leaf2").digest(),
            hashlib.sha256(b"leaf3").digest(),
            hashlib.sha256(b"leaf4").digest(),
        ]
        
        tree = MerkleTree(leaves)
        
        assert tree.get_num_leaves() == 4
        assert len(tree.get_root()) == 32
    
    def test_generate_proof(self):
        """Test generating Merkle proof."""
        leaves = [
            hashlib.sha256(f"leaf{i}".encode()).digest()
            for i in range(8)
        ]
        
        tree = MerkleTree(leaves)
        
        # Generate proof for leaf 3
        proof = tree.get_proof(3)
        
        assert proof.leaf_index == 3
        assert proof.leaf_hash == leaves[3]
        assert proof.root == tree.get_root()
        assert len(proof.proof) > 0
    
    def test_verify_proof(self):
        """Test verifying Merkle proof."""
        leaves = [
            hashlib.sha256(f"leaf{i}".encode()).digest()
            for i in range(8)
        ]
        
        tree = MerkleTree(leaves)
        
        # Generate and verify proof
        proof = tree.get_proof(3)
        is_valid = MerkleTree.verify_proof(proof)
        
        assert is_valid
    
    def test_invalid_proof(self):
        """Test that invalid proof fails verification."""
        leaves = [
            hashlib.sha256(f"leaf{i}".encode()).digest()
            for i in range(8)
        ]
        
        tree = MerkleTree(leaves)
        proof = tree.get_proof(3)
        
        # Tamper with proof
        tampered_proof = MerkleProof(
            leaf_index=proof.leaf_index,
            leaf_hash=hashlib.sha256(b"wrong").digest(),
            proof=proof.proof,
            root=proof.root
        )
        
        is_valid = MerkleTree.verify_proof(tampered_proof)
        assert not is_valid
    
    def test_odd_number_of_leaves(self):
        """Test tree with odd number of leaves."""
        leaves = [
            hashlib.sha256(f"leaf{i}".encode()).digest()
            for i in range(7)
        ]
        
        tree = MerkleTree(leaves)
        
        assert tree.get_num_leaves() == 7
        assert len(tree.get_root()) == 32
        
        # Verify proofs for all leaves
        for i in range(7):
            proof = tree.get_proof(i)
            assert MerkleTree.verify_proof(proof)
    
    def test_create_from_data(self):
        """Test creating tree from raw data."""
        data = [b"data1", b"data2", b"data3", b"data4"]
        tree = MerkleTree.create_from_data(data)
        
        assert tree.get_num_leaves() == 4
        assert len(tree.get_root()) == 32
    
    def test_empty_leaves_raises_error(self):
        """Test that empty leaves raises error."""
        with pytest.raises(ValueError):
            MerkleTree([])
    
    def test_proof_out_of_range(self):
        """Test that out of range proof raises error."""
        leaves = [hashlib.sha256(b"test").digest()]
        tree = MerkleTree(leaves)
        
        with pytest.raises(IndexError):
            tree.get_proof(5)


class TestMerkleTreeBuilder:
    """Test MerkleTreeBuilder functionality."""
    
    def test_build_tree(self):
        """Test building tree incrementally."""
        builder = MerkleTreeBuilder()
        
        for i in range(5):
            builder.add_leaf(f"data{i}".encode())
        
        tree = builder.build()
        
        assert tree.get_num_leaves() == 5
        assert len(tree.get_root()) == 32
    
    def test_add_leaf_hash(self):
        """Test adding pre-hashed leaves."""
        builder = MerkleTreeBuilder()
        
        for i in range(3):
            leaf_hash = hashlib.sha256(f"data{i}".encode()).digest()
            builder.add_leaf_hash(leaf_hash)
        
        tree = builder.build()
        assert tree.get_num_leaves() == 3
    
    def test_reset(self):
        """Test resetting builder."""
        builder = MerkleTreeBuilder()
        builder.add_leaf(b"test")
        builder.reset()
        
        with pytest.raises(ValueError):
            builder.build()
    
    def test_build_without_leaves(self):
        """Test building without leaves raises error."""
        builder = MerkleTreeBuilder()
        
        with pytest.raises(ValueError):
            builder.build()


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
