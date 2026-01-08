"""
Weight Utilities Module

Provides utilities for weight matrix operations, normalization, and validation.
Used for managing neuron weights in the network topology.
"""

import numpy as np
from typing import List, Tuple, Optional, Union
import logging


logger = logging.getLogger(__name__)


class WeightError(Exception):
    """Exception raised for weight-related errors."""
    pass


def normalize_weights(
    weights: np.ndarray,
    method: str = "sum",
    axis: Optional[int] = None
) -> np.ndarray:
    """
    Normalize weight matrix or vector.
    
    Args:
        weights: Weight matrix or vector to normalize
        method: Normalization method:
            - "sum": Normalize so sum equals 1 (default)
            - "max": Normalize by maximum value (0-1 range)
            - "zscore": Z-score normalization (mean=0, std=1)
            - "minmax": Min-max normalization (0-1 range)
        axis: Axis along which to normalize (None for entire array)
        
    Returns:
        Normalized weights
        
    Examples:
        >>> weights = np.array([1.0, 2.0, 3.0, 4.0])
        >>> normalize_weights(weights)
        array([0.1, 0.2, 0.3, 0.4])
        
        >>> matrix = np.array([[1, 2], [3, 4]])
        >>> normalize_weights(matrix, axis=1)
        array([[0.33333333, 0.66666667],
               [0.42857143, 0.57142857]])
    """
    weights = np.asarray(weights, dtype=np.float64)
    
    if method == "sum":
        # Normalize so sum equals 1
        weight_sum = np.sum(weights, axis=axis, keepdims=True)
        if np.any(weight_sum == 0):
            logger.warning("Sum normalization with zero sum detected, returning zeros")
            return np.zeros_like(weights)
        return weights / weight_sum
    
    elif method == "max":
        # Normalize by maximum value
        weight_max = np.max(weights, axis=axis, keepdims=True)
        if np.any(weight_max == 0):
            logger.warning("Max normalization with zero max detected, returning zeros")
            return np.zeros_like(weights)
        return weights / weight_max
    
    elif method == "zscore":
        # Z-score normalization
        mean = np.mean(weights, axis=axis, keepdims=True)
        std = np.std(weights, axis=axis, keepdims=True)
        if np.any(std == 0):
            logger.warning("Z-score normalization with zero std detected, returning zeros")
            return np.zeros_like(weights)
        return (weights - mean) / std
    
    elif method == "minmax":
        # Min-max normalization to [0, 1]
        weight_min = np.min(weights, axis=axis, keepdims=True)
        weight_max = np.max(weights, axis=axis, keepdims=True)
        weight_range = weight_max - weight_min
        if np.any(weight_range == 0):
            logger.warning("Min-max normalization with zero range detected, returning zeros")
            return np.zeros_like(weights)
        return (weights - weight_min) / weight_range
    
    else:
        raise WeightError(f"Unknown normalization method: {method}")


def validate_weight_matrix(
    weights: np.ndarray,
    num_neurons: Optional[int] = None,
    allow_negative: bool = False,
    check_normalized: bool = True,
    tolerance: float = 1e-6
) -> Tuple[bool, Optional[str]]:
    """
    Validate weight matrix for correctness.
    
    Args:
        weights: Weight matrix to validate
        num_neurons: Expected number of neurons (checks shape)
        allow_negative: Whether negative weights are allowed
        check_normalized: Whether to check if rows sum to 1
        tolerance: Tolerance for normalization check
        
    Returns:
        Tuple of (is_valid, error_message)
        
    Examples:
        >>> weights = np.array([[0.5, 0.5], [0.3, 0.7]])
        >>> is_valid, error = validate_weight_matrix(weights)
        >>> print(is_valid)
        True
        
        >>> weights = np.array([[0.5, 0.3], [0.3, 0.7]])
        >>> is_valid, error = validate_weight_matrix(weights)
        >>> print(is_valid)
        False
        >>> print(error)
        Row 0 sum is 0.8, expected ~1.0
    """
    weights = np.asarray(weights)
    
    # Check if array is empty
    if weights.size == 0:
        return False, "Weight matrix is empty"
    
    # Check for NaN or Inf
    if np.any(np.isnan(weights)):
        return False, "Weight matrix contains NaN values"
    if np.any(np.isinf(weights)):
        return False, "Weight matrix contains infinite values"
    
    # Check shape
    if num_neurons is not None:
        if weights.ndim == 1:
            if len(weights) != num_neurons:
                return False, f"Expected {num_neurons} weights, got {len(weights)}"
        elif weights.ndim == 2:
            if weights.shape[0] != num_neurons or weights.shape[1] != num_neurons:
                return False, f"Expected {num_neurons}x{num_neurons} matrix, got {weights.shape}"
        else:
            return False, f"Expected 1D or 2D array, got {weights.ndim}D"
    
    # Check for negative weights
    if not allow_negative and np.any(weights < 0):
        return False, "Weight matrix contains negative values"
    
    # Check normalization (for 2D matrices, check rows)
    if check_normalized:
        if weights.ndim == 1:
            weight_sum = np.sum(weights)
            if abs(weight_sum - 1.0) > tolerance:
                return False, f"Weights sum to {weight_sum:.6f}, expected ~1.0"
        elif weights.ndim == 2:
            row_sums = np.sum(weights, axis=1)
            for i, row_sum in enumerate(row_sums):
                if abs(row_sum - 1.0) > tolerance:
                    return False, f"Row {i} sum is {row_sum:.6f}, expected ~1.0"
    
    return True, None


def compute_weight_consensus(
    weight_matrices: List[np.ndarray],
    method: str = "mean"
) -> np.ndarray:
    """
    Compute consensus weight matrix from multiple weight matrices.
    
    Args:
        weight_matrices: List of weight matrices from different sources
        method: Consensus method:
            - "mean": Average weights
            - "median": Median weights
            - "max": Maximum weight for each position
            - "min": Minimum weight for each position
        
    Returns:
        Consensus weight matrix
        
    Examples:
        >>> w1 = np.array([[0.6, 0.4], [0.3, 0.7]])
        >>> w2 = np.array([[0.5, 0.5], [0.4, 0.6]])
        >>> consensus = compute_weight_consensus([w1, w2], method="mean")
        >>> print(consensus)
        [[0.55 0.45]
         [0.35 0.65]]
    """
    if not weight_matrices:
        raise WeightError("Empty weight matrices list")
    
    # Stack all matrices
    stacked = np.stack(weight_matrices)
    
    if method == "mean":
        return np.mean(stacked, axis=0)
    elif method == "median":
        return np.median(stacked, axis=0)
    elif method == "max":
        return np.max(stacked, axis=0)
    elif method == "min":
        return np.min(stacked, axis=0)
    else:
        raise WeightError(f"Unknown consensus method: {method}")


def apply_weight_decay(
    weights: np.ndarray,
    decay_factor: float,
    min_weight: float = 0.0
) -> np.ndarray:
    """
    Apply exponential decay to weights.
    
    Useful for aging old weights over time.
    
    Args:
        weights: Weight matrix or vector
        decay_factor: Decay factor (0-1), where 1 means no decay
        min_weight: Minimum weight threshold
        
    Returns:
        Decayed weights
        
    Examples:
        >>> weights = np.array([1.0, 0.8, 0.6, 0.4])
        >>> decayed = apply_weight_decay(weights, decay_factor=0.9)
        >>> print(decayed)
        [0.9 0.72 0.54 0.36]
    """
    if not 0 <= decay_factor <= 1:
        raise WeightError(f"Decay factor must be in [0, 1], got {decay_factor}")
    
    decayed = weights * decay_factor
    decayed[decayed < min_weight] = min_weight
    return decayed


def sparse_to_dense_weights(
    sparse_weights: dict,
    num_neurons: int,
    default_weight: float = 0.0
) -> np.ndarray:
    """
    Convert sparse weight representation to dense matrix.
    
    Args:
        sparse_weights: Dictionary mapping (from_idx, to_idx) to weight
        num_neurons: Total number of neurons
        default_weight: Default weight for missing entries
        
    Returns:
        Dense weight matrix
        
    Examples:
        >>> sparse = {(0, 1): 0.5, (0, 2): 0.3, (1, 2): 0.8}
        >>> dense = sparse_to_dense_weights(sparse, num_neurons=3)
        >>> print(dense)
        [[0.  0.5 0.3]
         [0.  0.  0.8]
         [0.  0.  0. ]]
    """
    weights = np.full((num_neurons, num_neurons), default_weight, dtype=np.float64)
    
    for (from_idx, to_idx), weight in sparse_weights.items():
        if not (0 <= from_idx < num_neurons and 0 <= to_idx < num_neurons):
            raise WeightError(
                f"Invalid indices ({from_idx}, {to_idx}) for {num_neurons} neurons"
            )
        weights[from_idx, to_idx] = weight
    
    return weights


def dense_to_sparse_weights(
    weights: np.ndarray,
    threshold: float = 0.0
) -> dict:
    """
    Convert dense weight matrix to sparse representation.
    
    Only includes weights above threshold to save space.
    
    Args:
        weights: Dense weight matrix
        threshold: Minimum weight to include (default: 0.0)
        
    Returns:
        Dictionary mapping (from_idx, to_idx) to weight
        
    Examples:
        >>> dense = np.array([[0.0, 0.5, 0.3], [0.0, 0.0, 0.8], [0.0, 0.0, 0.0]])
        >>> sparse = dense_to_sparse_weights(dense)
        >>> print(sparse)
        {(0, 1): 0.5, (0, 2): 0.3, (1, 2): 0.8}
    """
    sparse = {}
    rows, cols = np.where(weights > threshold)
    
    for row, col in zip(rows, cols):
        sparse[(int(row), int(col))] = float(weights[row, col])
    
    return sparse


def clip_weights(
    weights: np.ndarray,
    min_weight: float = 0.0,
    max_weight: float = 1.0
) -> np.ndarray:
    """
    Clip weights to specified range.
    
    Args:
        weights: Weight matrix or vector
        min_weight: Minimum weight value
        max_weight: Maximum weight value
        
    Returns:
        Clipped weights
        
    Examples:
        >>> weights = np.array([0.5, 1.5, -0.5, 0.8])
        >>> clipped = clip_weights(weights, min_weight=0.0, max_weight=1.0)
        >>> print(clipped)
        [0.5 1.  0.  0.8]
    """
    return np.clip(weights, min_weight, max_weight)


def compute_weight_entropy(weights: np.ndarray) -> float:
    """
    Compute Shannon entropy of weight distribution.
    
    Higher entropy means more uniform distribution.
    
    Args:
        weights: Weight vector or matrix (if matrix, computes mean row entropy)
        
    Returns:
        Entropy value
        
    Examples:
        >>> weights = np.array([0.25, 0.25, 0.25, 0.25])  # Uniform
        >>> entropy = compute_weight_entropy(weights)
        >>> print(f"{entropy:.2f}")
        1.39  # High entropy
        
        >>> weights = np.array([0.9, 0.05, 0.03, 0.02])  # Concentrated
        >>> entropy = compute_weight_entropy(weights)
        >>> print(f"{entropy:.2f}")
        0.64  # Low entropy
    """
    weights = np.asarray(weights)
    
    if weights.ndim == 2:
        # For matrix, compute mean entropy across rows
        entropies = []
        for row in weights:
            # Filter positive weights
            positive_weights = row[row > 0]
            if len(positive_weights) > 0:
                # Normalize
                normalized = positive_weights / np.sum(positive_weights)
                # Compute entropy
                entropy = -np.sum(normalized * np.log(normalized))
                entropies.append(entropy)
        return np.mean(entropies) if entropies else 0.0
    else:
        # For vector
        positive_weights = weights[weights > 0]
        if len(positive_weights) == 0:
            return 0.0
        # Normalize
        normalized = positive_weights / np.sum(positive_weights)
        # Compute entropy
        return -np.sum(normalized * np.log(normalized))


def smooth_weights(
    weights: np.ndarray,
    window_size: int = 3,
    method: str = "moving_average"
) -> np.ndarray:
    """
    Smooth weights using moving average or other methods.
    
    Args:
        weights: Weight vector to smooth
        window_size: Size of smoothing window
        method: Smoothing method ("moving_average" or "exponential")
        
    Returns:
        Smoothed weights
        
    Examples:
        >>> weights = np.array([0.1, 0.5, 0.2, 0.8, 0.3])
        >>> smoothed = smooth_weights(weights, window_size=3)
        >>> print(smoothed)
        [0.1  0.26666667 0.5  0.43333333 0.3]
    """
    if weights.ndim != 1:
        raise WeightError("Smoothing only supported for 1D weight vectors")
    
    if window_size < 1:
        raise WeightError(f"Window size must be >= 1, got {window_size}")
    
    if method == "moving_average":
        # Simple moving average
        smoothed = np.copy(weights)
        for i in range(len(weights)):
            start_idx = max(0, i - window_size // 2)
            end_idx = min(len(weights), i + window_size // 2 + 1)
            smoothed[i] = np.mean(weights[start_idx:end_idx])
        return smoothed
    
    elif method == "exponential":
        # Exponential moving average
        alpha = 2.0 / (window_size + 1)
        smoothed = np.zeros_like(weights)
        smoothed[0] = weights[0]
        for i in range(1, len(weights)):
            smoothed[i] = alpha * weights[i] + (1 - alpha) * smoothed[i - 1]
        return smoothed
    
    else:
        raise WeightError(f"Unknown smoothing method: {method}")


def top_k_weights(
    weights: np.ndarray,
    k: int,
    keep_others: bool = False,
    other_value: float = 0.0
) -> np.ndarray:
    """
    Keep only top-k weights, set others to zero or specified value.
    
    Args:
        weights: Weight vector or matrix
        k: Number of top weights to keep
        keep_others: If True, keep other weights; if False, set to other_value
        other_value: Value for non-top-k weights (if keep_others=False)
        
    Returns:
        Filtered weights
        
    Examples:
        >>> weights = np.array([0.1, 0.5, 0.2, 0.8, 0.3])
        >>> top_k = top_k_weights(weights, k=2)
        >>> print(top_k)
        [0.  0.5 0.  0.8 0. ]
    """
    result = np.copy(weights)
    
    if weights.ndim == 1:
        # For vector
        if k >= len(weights):
            return result
        
        # Get indices of top-k values
        top_indices = np.argsort(weights)[-k:]
        
        if not keep_others:
            # Set non-top-k to other_value
            mask = np.ones(len(weights), dtype=bool)
            mask[top_indices] = False
            result[mask] = other_value
    
    elif weights.ndim == 2:
        # For matrix, apply to each row
        for i in range(weights.shape[0]):
            if k >= weights.shape[1]:
                continue
            
            top_indices = np.argsort(weights[i])[-k:]
            
            if not keep_others:
                mask = np.ones(weights.shape[1], dtype=bool)
                mask[top_indices] = False
                result[i, mask] = other_value
    
    else:
        raise WeightError(f"Unsupported weight dimensions: {weights.ndim}")
    
    return result
