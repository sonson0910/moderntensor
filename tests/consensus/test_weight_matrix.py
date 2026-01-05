# tests/consensus/test_weight_matrix.py
"""
Tests for WeightMatrixManager with hybrid storage.
"""

import pytest
import numpy as np
from sdk.consensus.weight_matrix import (
    WeightMatrixManager,
    WeightMatrixMetadata
)


class TestWeightMatrixManager:
    """Test WeightMatrixManager functionality."""
    
    @pytest.mark.asyncio
    async def test_store_and_retrieve_dense_matrix(self):
        """Test storing and retrieving a dense weight matrix."""
        manager = WeightMatrixManager()
        
        # Create a dense weight matrix (5 validators x 10 miners)
        weights = np.random.rand(5, 10)
        
        # Store the matrix
        merkle_root, ipfs_hash = await manager.store_weight_matrix(
            subnet_uid=1,
            epoch=10,
            weights=weights,
            upload_to_ipfs=False
        )
        
        assert len(merkle_root) == 32  # SHA256 hash
        assert ipfs_hash == "local_only"
        
        # Retrieve the matrix
        retrieved_weights = await manager.get_weight_matrix(1, 10)
        
        assert retrieved_weights is not None
        assert np.allclose(weights, retrieved_weights)
    
    @pytest.mark.asyncio
    async def test_store_and_retrieve_sparse_matrix(self):
        """Test storing and retrieving a sparse weight matrix."""
        manager = WeightMatrixManager()
        
        # Create a sparse weight matrix (many zeros)
        weights = np.zeros((20, 50))
        # Add some non-zero values
        weights[0, 0] = 1.0
        weights[5, 10] = 0.8
        weights[10, 20] = 0.6
        weights[15, 40] = 0.9
        
        # Store the matrix
        merkle_root, ipfs_hash = await manager.store_weight_matrix(
            subnet_uid=2,
            epoch=5,
            weights=weights,
            upload_to_ipfs=False
        )
        
        assert len(merkle_root) == 32
        
        # Retrieve the matrix
        retrieved_weights = await manager.get_weight_matrix(2, 5)
        
        assert retrieved_weights is not None
        assert np.allclose(weights, retrieved_weights)
    
    @pytest.mark.asyncio
    async def test_verify_weight_matrix(self):
        """Test verifying a weight matrix against Merkle root."""
        manager = WeightMatrixManager()
        
        weights = np.random.rand(3, 5)
        
        # Store and get Merkle root
        merkle_root, _ = await manager.store_weight_matrix(
            subnet_uid=1,
            epoch=1,
            weights=weights,
            upload_to_ipfs=False
        )
        
        # Verify with correct weights
        is_valid = await manager.verify_weight_matrix(
            subnet_uid=1,
            epoch=1,
            weights=weights,
            merkle_root=merkle_root
        )
        
        assert is_valid
        
        # Verify with incorrect weights
        wrong_weights = np.random.rand(3, 5)
        is_valid = await manager.verify_weight_matrix(
            subnet_uid=1,
            epoch=1,
            weights=wrong_weights,
            merkle_root=merkle_root
        )
        
        assert not is_valid
    
    @pytest.mark.asyncio
    async def test_cache_functionality(self):
        """Test that cache works correctly."""
        manager = WeightMatrixManager()
        
        weights = np.random.rand(4, 8)
        
        # Store matrix
        await manager.store_weight_matrix(
            subnet_uid=3,
            epoch=7,
            weights=weights,
            upload_to_ipfs=False
        )
        
        # First retrieval (from DB)
        retrieved1 = await manager.get_weight_matrix(3, 7)
        
        # Second retrieval (should be from cache)
        retrieved2 = await manager.get_weight_matrix(3, 7)
        
        assert np.allclose(retrieved1, retrieved2)
        assert np.allclose(weights, retrieved2)
        
        # Check cache
        assert (3, 7) in manager.cache
    
    @pytest.mark.asyncio
    async def test_get_metadata(self):
        """Test getting metadata for a weight matrix."""
        manager = WeightMatrixManager()
        
        weights = np.random.rand(6, 12)
        
        await manager.store_weight_matrix(
            subnet_uid=4,
            epoch=3,
            weights=weights,
            upload_to_ipfs=False
        )
        
        metadata = await manager.get_metadata(4, 3)
        
        assert metadata is not None
        assert metadata.subnet_uid == 4
        assert metadata.epoch == 3
        assert metadata.num_validators == 6
        assert metadata.num_miners == 12
        assert len(metadata.merkle_root) == 32
        assert metadata.ipfs_hash == "local_only"
    
    @pytest.mark.asyncio
    async def test_storage_stats(self):
        """Test getting storage statistics."""
        manager = WeightMatrixManager()
        
        # Store multiple matrices
        for epoch in range(5):
            weights = np.random.rand(5, 10)
            await manager.store_weight_matrix(
                subnet_uid=1,
                epoch=epoch,
                weights=weights,
                upload_to_ipfs=False
            )
        
        stats = manager.get_storage_stats()
        
        assert stats['total_matrices'] == 5
        assert stats['total_size_bytes'] > 0
        assert stats['cache_size'] == 5
        assert 0.0 <= stats['avg_compression_ratio'] <= 1.0
    
    @pytest.mark.asyncio
    async def test_clear_cache(self):
        """Test clearing the cache."""
        manager = WeightMatrixManager()
        
        weights = np.random.rand(3, 6)
        await manager.store_weight_matrix(
            subnet_uid=5,
            epoch=2,
            weights=weights,
            upload_to_ipfs=False
        )
        
        # Cache should have data
        assert len(manager.cache) > 0
        assert len(manager.metadata_cache) > 0
        
        # Clear cache
        manager.clear_cache()
        
        assert len(manager.cache) == 0
        assert len(manager.metadata_cache) == 0
        
        # Data should still be retrievable from DB
        retrieved = await manager.get_weight_matrix(5, 2)
        assert retrieved is not None
        assert np.allclose(weights, retrieved)
    
    @pytest.mark.asyncio
    async def test_prune_old_matrices(self):
        """Test pruning old weight matrices."""
        manager = WeightMatrixManager()
        
        # Store 10 matrices
        for epoch in range(10):
            weights = np.random.rand(3, 5)
            await manager.store_weight_matrix(
                subnet_uid=6,
                epoch=epoch,
                weights=weights,
                upload_to_ipfs=False
            )
        
        # Prune, keeping only 5 recent
        pruned_count = await manager.prune_old_matrices(
            subnet_uid=6,
            keep_recent=5
        )
        
        assert pruned_count == 5
        
        # Recent matrices should still be available
        for epoch in range(5, 10):
            weights = await manager.get_weight_matrix(6, epoch)
            assert weights is not None
        
        # Old matrices should be gone
        for epoch in range(5):
            weights = await manager.get_weight_matrix(6, epoch)
            assert weights is None
    
    @pytest.mark.asyncio
    async def test_invalid_matrix_dimensions(self):
        """Test handling of invalid matrix dimensions."""
        manager = WeightMatrixManager()
        
        # Try to store a 1D array
        invalid_weights = np.random.rand(10)
        
        with pytest.raises(ValueError):
            await manager.store_weight_matrix(
                subnet_uid=7,
                epoch=1,
                weights=invalid_weights,
                upload_to_ipfs=False
            )
    
    @pytest.mark.asyncio
    async def test_get_nonexistent_matrix(self):
        """Test retrieving a non-existent matrix."""
        manager = WeightMatrixManager()
        
        # Try to get a matrix that was never stored
        weights = await manager.get_weight_matrix(999, 999)
        
        assert weights is None
    
    @pytest.mark.asyncio
    async def test_merkle_root_consistency(self):
        """Test that Merkle root is consistent for the same matrix."""
        manager = WeightMatrixManager()
        
        weights = np.random.rand(4, 6)
        
        # Store the same matrix twice
        root1, _ = await manager.store_weight_matrix(
            subnet_uid=8,
            epoch=1,
            weights=weights,
            upload_to_ipfs=False
        )
        
        root2, _ = await manager.store_weight_matrix(
            subnet_uid=9,
            epoch=1,
            weights=weights,
            upload_to_ipfs=False
        )
        
        # Merkle roots should be identical for the same matrix
        assert root1 == root2
    
    @pytest.mark.asyncio
    async def test_sparse_compression(self):
        """Test that sparse matrices are compressed efficiently."""
        manager = WeightMatrixManager()
        
        # Create a very sparse matrix (99% zeros)
        weights = np.zeros((100, 200))
        # Add just a few non-zero values
        weights[10, 20] = 1.0
        weights[50, 100] = 0.5
        
        await manager.store_weight_matrix(
            subnet_uid=10,
            epoch=1,
            weights=weights,
            upload_to_ipfs=False
        )
        
        metadata = await manager.get_metadata(10, 1)
        
        assert metadata.is_sparse
        # Compression ratio should be < 1 (compressed size < original size)
        assert metadata.compression_ratio < 1.0


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
