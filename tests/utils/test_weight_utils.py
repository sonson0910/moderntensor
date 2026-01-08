"""
Tests for Weight Utilities

Comprehensive test suite for weight matrix operations and utilities.
"""

import pytest
import numpy as np
from sdk.utils.weight_utils import (
    WeightError,
    normalize_weights,
    validate_weight_matrix,
    compute_weight_consensus,
    apply_weight_decay,
    sparse_to_dense_weights,
    dense_to_sparse_weights,
    clip_weights,
    compute_weight_entropy,
    smooth_weights,
    top_k_weights,
)


class TestNormalizeWeights:
    """Test weight normalization functions."""
    
    def test_normalize_sum_vector(self):
        """Test sum normalization on vector."""
        weights = np.array([1.0, 2.0, 3.0, 4.0])
        normalized = normalize_weights(weights, method="sum")
        assert np.allclose(normalized, [0.1, 0.2, 0.3, 0.4])
        assert np.isclose(np.sum(normalized), 1.0)
    
    def test_normalize_sum_matrix(self):
        """Test sum normalization on matrix by row."""
        weights = np.array([[1, 2], [3, 4]])
        normalized = normalize_weights(weights, method="sum", axis=1)
        # Check each row sums to 1
        assert np.allclose(np.sum(normalized, axis=1), [1.0, 1.0])
    
    def test_normalize_max(self):
        """Test max normalization."""
        weights = np.array([1.0, 2.0, 3.0, 4.0])
        normalized = normalize_weights(weights, method="max")
        assert np.allclose(normalized, [0.25, 0.5, 0.75, 1.0])
    
    def test_normalize_minmax(self):
        """Test min-max normalization."""
        weights = np.array([1.0, 2.0, 3.0, 4.0])
        normalized = normalize_weights(weights, method="minmax")
        assert np.isclose(normalized[0], 0.0)  # Min maps to 0
        assert np.isclose(normalized[-1], 1.0)  # Max maps to 1
    
    def test_normalize_zscore(self):
        """Test z-score normalization."""
        weights = np.array([1.0, 2.0, 3.0, 4.0, 5.0])
        normalized = normalize_weights(weights, method="zscore")
        assert np.isclose(np.mean(normalized), 0.0, atol=1e-10)
        assert np.isclose(np.std(normalized), 1.0)
    
    def test_normalize_zero_sum(self):
        """Test normalization with zero sum."""
        weights = np.array([0.0, 0.0, 0.0])
        normalized = normalize_weights(weights, method="sum")
        assert np.all(normalized == 0.0)
    
    def test_normalize_invalid_method(self):
        """Test invalid normalization method raises error."""
        weights = np.array([1.0, 2.0, 3.0])
        with pytest.raises(WeightError):
            normalize_weights(weights, method="invalid")


class TestValidateWeightMatrix:
    """Test weight matrix validation."""
    
    def test_validate_correct_matrix(self):
        """Test validation of correct normalized matrix."""
        weights = np.array([[0.5, 0.5], [0.3, 0.7]])
        is_valid, error = validate_weight_matrix(weights)
        assert is_valid
        assert error is None
    
    def test_validate_unnormalized_matrix(self):
        """Test validation fails for unnormalized matrix."""
        weights = np.array([[0.5, 0.3], [0.3, 0.7]])
        is_valid, error = validate_weight_matrix(weights)
        assert not is_valid
        assert "sum" in error.lower()
    
    def test_validate_with_nan(self):
        """Test validation fails with NaN."""
        weights = np.array([[0.5, np.nan], [0.3, 0.7]])
        is_valid, error = validate_weight_matrix(weights)
        assert not is_valid
        assert "NaN" in error
    
    def test_validate_with_inf(self):
        """Test validation fails with infinite values."""
        weights = np.array([[0.5, np.inf], [0.3, 0.7]])
        is_valid, error = validate_weight_matrix(weights)
        assert not is_valid
        assert "infinite" in error.lower()
    
    def test_validate_negative_weights(self):
        """Test validation of negative weights."""
        weights = np.array([[0.6, -0.1], [0.3, 0.7]])
        is_valid, error = validate_weight_matrix(weights, allow_negative=False)
        assert not is_valid
        assert "negative" in error.lower()
        
        # Should pass if we allow negative
        is_valid, error = validate_weight_matrix(weights, allow_negative=True, check_normalized=False)
        assert is_valid
    
    def test_validate_shape(self):
        """Test validation of matrix shape."""
        weights = np.array([[0.5, 0.5], [0.3, 0.7]])
        is_valid, error = validate_weight_matrix(weights, num_neurons=2)
        assert is_valid
        
        is_valid, error = validate_weight_matrix(weights, num_neurons=3)
        assert not is_valid
    
    def test_validate_empty_matrix(self):
        """Test validation of empty matrix."""
        weights = np.array([])
        is_valid, error = validate_weight_matrix(weights)
        assert not is_valid
        assert "empty" in error.lower()


class TestWeightConsensus:
    """Test weight consensus computation."""
    
    def test_consensus_mean(self):
        """Test mean consensus."""
        w1 = np.array([[0.6, 0.4], [0.3, 0.7]])
        w2 = np.array([[0.5, 0.5], [0.4, 0.6]])
        consensus = compute_weight_consensus([w1, w2], method="mean")
        expected = np.array([[0.55, 0.45], [0.35, 0.65]])
        assert np.allclose(consensus, expected)
    
    def test_consensus_median(self):
        """Test median consensus."""
        w1 = np.array([[0.6, 0.4]])
        w2 = np.array([[0.5, 0.5]])
        w3 = np.array([[0.7, 0.3]])
        consensus = compute_weight_consensus([w1, w2, w3], method="median")
        expected = np.array([[0.6, 0.4]])
        assert np.allclose(consensus, expected)
    
    def test_consensus_max(self):
        """Test max consensus."""
        w1 = np.array([[0.6, 0.4]])
        w2 = np.array([[0.5, 0.5]])
        consensus = compute_weight_consensus([w1, w2], method="max")
        expected = np.array([[0.6, 0.5]])
        assert np.allclose(consensus, expected)
    
    def test_consensus_empty_list(self):
        """Test consensus with empty list raises error."""
        with pytest.raises(WeightError):
            compute_weight_consensus([])


class TestWeightDecay:
    """Test weight decay functions."""
    
    def test_apply_decay(self):
        """Test applying decay to weights."""
        weights = np.array([1.0, 0.8, 0.6, 0.4])
        decayed = apply_weight_decay(weights, decay_factor=0.9)
        expected = np.array([0.9, 0.72, 0.54, 0.36])
        assert np.allclose(decayed, expected)
    
    def test_apply_decay_with_minimum(self):
        """Test decay with minimum threshold."""
        weights = np.array([1.0, 0.5, 0.1, 0.05])
        decayed = apply_weight_decay(weights, decay_factor=0.5, min_weight=0.1)
        assert np.all(decayed >= 0.1)
    
    def test_apply_decay_invalid_factor(self):
        """Test invalid decay factor raises error."""
        weights = np.array([1.0, 0.5])
        with pytest.raises(WeightError):
            apply_weight_decay(weights, decay_factor=1.5)


class TestSparseConversion:
    """Test sparse/dense weight conversion."""
    
    def test_sparse_to_dense(self):
        """Test converting sparse to dense weights."""
        sparse = {(0, 1): 0.5, (0, 2): 0.3, (1, 2): 0.8}
        dense = sparse_to_dense_weights(sparse, num_neurons=3)
        assert dense[0, 1] == 0.5
        assert dense[0, 2] == 0.3
        assert dense[1, 2] == 0.8
        assert dense[0, 0] == 0.0  # Default value
    
    def test_dense_to_sparse(self):
        """Test converting dense to sparse weights."""
        dense = np.array([[0.0, 0.5, 0.3], [0.0, 0.0, 0.8], [0.0, 0.0, 0.0]])
        sparse = dense_to_sparse_weights(dense)
        assert sparse[(0, 1)] == 0.5
        assert sparse[(0, 2)] == 0.3
        assert sparse[(1, 2)] == 0.8
        assert len(sparse) == 3
    
    def test_sparse_to_dense_invalid_indices(self):
        """Test sparse to dense with invalid indices raises error."""
        sparse = {(0, 5): 0.5}  # Index 5 out of range for 3 neurons
        with pytest.raises(WeightError):
            sparse_to_dense_weights(sparse, num_neurons=3)
    
    def test_dense_to_sparse_with_threshold(self):
        """Test sparse conversion with threshold."""
        dense = np.array([[0.01, 0.5], [0.02, 0.8]])
        sparse = dense_to_sparse_weights(dense, threshold=0.05)
        # Should only include weights > 0.05
        assert (0, 0) not in sparse
        assert (1, 0) not in sparse
        assert (0, 1) in sparse
        assert (1, 1) in sparse


class TestClipWeights:
    """Test weight clipping."""
    
    def test_clip_weights(self):
        """Test clipping weights to range."""
        weights = np.array([0.5, 1.5, -0.5, 0.8])
        clipped = clip_weights(weights, min_weight=0.0, max_weight=1.0)
        assert np.allclose(clipped, [0.5, 1.0, 0.0, 0.8])
    
    def test_clip_weights_custom_range(self):
        """Test clipping to custom range."""
        weights = np.array([0.1, 0.5, 0.9])
        clipped = clip_weights(weights, min_weight=0.2, max_weight=0.8)
        assert np.allclose(clipped, [0.2, 0.5, 0.8])


class TestWeightEntropy:
    """Test weight entropy calculation."""
    
    def test_entropy_uniform(self):
        """Test entropy of uniform distribution (high entropy)."""
        weights = np.array([0.25, 0.25, 0.25, 0.25])
        entropy = compute_weight_entropy(weights)
        # Uniform distribution should have high entropy
        assert entropy > 1.0
    
    def test_entropy_concentrated(self):
        """Test entropy of concentrated distribution (low entropy)."""
        weights = np.array([0.9, 0.05, 0.03, 0.02])
        entropy = compute_weight_entropy(weights)
        # Concentrated distribution should have low entropy
        assert entropy < 1.0
    
    def test_entropy_matrix(self):
        """Test entropy calculation for matrix."""
        weights = np.array([[0.25, 0.25, 0.25, 0.25], [0.9, 0.05, 0.03, 0.02]])
        entropy = compute_weight_entropy(weights)
        # Should return mean entropy across rows
        assert 0.5 < entropy < 1.5


class TestSmoothWeights:
    """Test weight smoothing."""
    
    def test_smooth_moving_average(self):
        """Test moving average smoothing."""
        weights = np.array([0.1, 0.5, 0.2, 0.8, 0.3])
        smoothed = smooth_weights(weights, window_size=3, method="moving_average")
        # Smoothed values should be between min and max of window
        assert np.all(smoothed >= 0.1)
        assert np.all(smoothed <= 0.8)
    
    def test_smooth_exponential(self):
        """Test exponential smoothing."""
        weights = np.array([1.0, 0.0, 1.0, 0.0, 1.0])
        smoothed = smooth_weights(weights, window_size=3, method="exponential")
        # Exponential smoothing should create gradual transitions
        assert smoothed[0] == 1.0  # First value unchanged
        assert 0 < smoothed[1] < 1  # Smoothed between 1 and 0
    
    def test_smooth_invalid_method(self):
        """Test invalid smoothing method raises error."""
        weights = np.array([1.0, 0.5])
        with pytest.raises(WeightError):
            smooth_weights(weights, method="invalid")
    
    def test_smooth_multidim_raises(self):
        """Test smoothing multi-dimensional array raises error."""
        weights = np.array([[1.0, 0.5], [0.3, 0.7]])
        with pytest.raises(WeightError):
            smooth_weights(weights)


class TestTopKWeights:
    """Test top-k weight selection."""
    
    def test_top_k_vector(self):
        """Test top-k on vector."""
        weights = np.array([0.1, 0.5, 0.2, 0.8, 0.3])
        top_k = top_k_weights(weights, k=2)
        # Should keep only 0.5 and 0.8
        assert top_k[1] == 0.5
        assert top_k[3] == 0.8
        assert top_k[0] == 0.0
        assert top_k[2] == 0.0
        assert top_k[4] == 0.0
    
    def test_top_k_matrix(self):
        """Test top-k on matrix (per row)."""
        weights = np.array([[0.1, 0.5, 0.2], [0.8, 0.1, 0.3]])
        top_k = top_k_weights(weights, k=2)
        # Row 0: keep 0.5 and 0.2
        assert top_k[0, 1] == 0.5
        assert top_k[0, 2] == 0.2
        assert top_k[0, 0] == 0.0
        # Row 1: keep 0.8 and 0.3
        assert top_k[1, 0] == 0.8
        assert top_k[1, 2] == 0.3
        assert top_k[1, 1] == 0.0
    
    def test_top_k_all(self):
        """Test top-k when k >= length."""
        weights = np.array([0.1, 0.5, 0.2])
        top_k = top_k_weights(weights, k=5)
        # Should keep all weights
        assert np.allclose(top_k, weights)


class TestEdgeCases:
    """Test edge cases and error conditions."""
    
    def test_empty_weights(self):
        """Test handling of empty weight arrays."""
        weights = np.array([])
        is_valid, error = validate_weight_matrix(weights)
        assert not is_valid
    
    def test_single_weight(self):
        """Test handling of single weight."""
        weights = np.array([1.0])
        normalized = normalize_weights(weights, method="sum")
        assert np.isclose(normalized[0], 1.0)
    
    def test_very_small_weights(self):
        """Test handling of very small weights."""
        weights = np.array([1e-10, 1e-10, 1e-10])
        normalized = normalize_weights(weights, method="sum")
        assert np.isclose(np.sum(normalized), 1.0)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
