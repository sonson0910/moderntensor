"""
Weight Utilities

Helper functions for weight normalization and validation.
"""

import hashlib
import json
from typing import List, Tuple, Optional
import numpy as np


def normalize_weights(weights: List[float]) -> List[float]:
    """
    Normalize weights to sum to 1.0.
    
    Args:
        weights: List of raw weights
        
    Returns:
        Normalized weights that sum to 1.0
        
    Example:
        ```python
        from sdk.utils import normalize_weights
        
        weights = [10, 20, 30]
        normalized = normalize_weights(weights)
        print(normalized)  # [0.166..., 0.333..., 0.5]
        print(sum(normalized))  # 1.0
        ```
    """
    if not weights:
        return []
    
    weights_array = np.array(weights, dtype=np.float64)
    
    # Handle negative weights by setting to 0
    weights_array = np.maximum(weights_array, 0)
    
    total = np.sum(weights_array)
    
    if total == 0:
        # If all weights are 0, distribute equally
        return [1.0 / len(weights)] * len(weights)
    
    normalized = weights_array / total
    return normalized.tolist()


def validate_weights(
    uids: List[int],
    weights: List[float],
    min_weight: float = 0.0,
    max_weight: float = 1.0,
) -> Tuple[bool, Optional[str]]:
    """
    Validate weights for correctness.
    
    Args:
        uids: List of UIDs
        weights: List of weights
        min_weight: Minimum allowed weight
        max_weight: Maximum allowed weight
        
    Returns:
        Tuple of (is_valid, error_message)
        
    Example:
        ```python
        from sdk.utils import validate_weights
        
        uids = [0, 1, 2]
        weights = [0.5, 0.3, 0.2]
        is_valid, error = validate_weights(uids, weights)
        print(is_valid)  # True
        ```
    """
    # Check lengths match
    if len(uids) != len(weights):
        return False, "UIDs and weights must have the same length"
    
    # Check for empty
    if not uids or not weights:
        return False, "UIDs and weights cannot be empty"
    
    # Check UIDs are unique
    if len(set(uids)) != len(uids):
        return False, "UIDs must be unique"
    
    # Check UIDs are non-negative
    if any(uid < 0 for uid in uids):
        return False, "UIDs must be non-negative"
    
    # Check weights are in valid range
    for weight in weights:
        if weight < min_weight or weight > max_weight:
            return False, f"Weights must be between {min_weight} and {max_weight}"
    
    # Check weights sum approximately to 1.0 (allow small floating point error)
    total = sum(weights)
    if abs(total - 1.0) > 0.01:
        return False, f"Weights should sum to 1.0, got {total}"
    
    return True, None


def compute_weight_hash(
    uids: List[int],
    weights: List[float],
    salt: str = ""
) -> str:
    """
    Compute hash of weights for commit-reveal scheme.
    
    Args:
        uids: List of UIDs
        weights: List of weights
        salt: Random salt for hashing
        
    Returns:
        Hex string hash
        
    Example:
        ```python
        from sdk.utils import compute_weight_hash
        
        uids = [0, 1, 2]
        weights = [0.5, 0.3, 0.2]
        salt = "random_salt_123"
        
        commit_hash = compute_weight_hash(uids, weights, salt)
        print(commit_hash)  # "a1b2c3d4..."
        ```
    """
    # Create deterministic representation
    data = {
        "uids": uids,
        "weights": weights,
        "salt": salt,
    }
    
    # Serialize to JSON with sorted keys for determinism
    json_str = json.dumps(data, sort_keys=True)
    
    # Compute SHA256 hash
    hash_obj = hashlib.sha256(json_str.encode())
    
    return hash_obj.hexdigest()


__all__ = ["normalize_weights", "validate_weights", "compute_weight_hash"]
