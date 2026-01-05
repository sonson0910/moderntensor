# sdk/consensus/safety_utils.py
"""
Safety utilities for consensus operations.
Provides defensive programming helpers to prevent common errors.
"""
import logging
import math
from typing import Optional, Dict, Any, TypeVar, Callable
from functools import wraps

logger = logging.getLogger(__name__)

T = TypeVar('T')

# Safety constants
EPSILON = 1e-9


def safe_divide(numerator: float, denominator: float, default: float = 0.0) -> float:
    """
    Safely divide two numbers, returning default if denominator is zero or near-zero.
    
    Args:
        numerator: The numerator
        denominator: The denominator
        default: Value to return if division is unsafe (default: 0.0)
    
    Returns:
        The division result or default value
    
    Examples:
        >>> safe_divide(10.0, 2.0)
        5.0
        >>> safe_divide(10.0, 0.0)
        0.0
        >>> safe_divide(10.0, 1e-10, default=-1.0)
        -1.0
    """
    if abs(denominator) < EPSILON:
        logger.warning(
            f"Division by near-zero value: {denominator}. Returning default: {default}"
        )
        return default
    
    try:
        result = numerator / denominator
        # Check for invalid results (NaN, infinity)
        if math.isnan(result) or math.isinf(result):
            logger.error(f"Division produced invalid result: {result}. Using default.")
            return default
        return result
    except (ZeroDivisionError, OverflowError, ValueError) as e:
        logger.error(f"Division error: {e}. Returning default: {default}")
        return default


def safe_mean(values: list, default: float = 0.0) -> float:
    """
    Safely calculate mean of a list of values.
    
    Args:
        values: List of numeric values
        default: Value to return if list is empty or invalid
    
    Returns:
        The mean value or default
    """
    if not values or len(values) == 0:
        return default
    
    try:
        return sum(values) / len(values)
    except (TypeError, ValueError) as e:
        logger.error(f"Error calculating mean: {e}. Returning default: {default}")
        return default


def clamp(value: float, min_val: float, max_val: float) -> float:
    """
    Clamp a value between min and max bounds.
    
    Args:
        value: Value to clamp
        min_val: Minimum allowed value
        max_val: Maximum allowed value
    
    Returns:
        Clamped value
    """
    if min_val > max_val:
        raise ValueError(f"min_val ({min_val}) must be <= max_val ({max_val})")
    
    return max(min_val, min(max_val, value))


def validate_score(score: float, name: str = "score") -> float:
    """
    Validate and clamp a score to [0.0, 1.0] range.
    
    Args:
        score: Score value to validate
        name: Name of the score for logging
    
    Returns:
        Validated and clamped score
    
    Raises:
        ValueError: If score is NaN or infinite
    """
    if math.isnan(score):
        raise ValueError(f"{name} is NaN")
    
    if math.isinf(score):
        raise ValueError(f"{name} is infinite: {score}")
    
    clamped = clamp(score, 0.0, 1.0)
    
    if abs(clamped - score) > EPSILON:
        logger.warning(f"{name} {score} clamped to {clamped}")
    
    return clamped


def validate_uid(uid: Any, name: str = "UID") -> str:
    """
    Validate that a UID is a non-empty hex string.
    
    Args:
        uid: The UID to validate
        name: Name for error messages
    
    Returns:
        The validated UID as string
    
    Raises:
        ValueError: If UID is invalid
    """
    if not uid:
        raise ValueError(f"{name} is empty or None")
    
    uid_str = str(uid)
    
    if not uid_str:
        raise ValueError(f"{name} is empty string")
    
    # Check if it's a valid hex string
    try:
        int(uid_str, 16)
    except ValueError:
        raise ValueError(f"{name} '{uid_str}' is not a valid hex string")
    
    return uid_str


def validate_dict_structure(
    data: Dict[str, Any],
    required_keys: list,
    optional_keys: list = None,
    name: str = "data"
) -> None:
    """
    Validate that a dictionary has required structure.
    
    Args:
        data: Dictionary to validate
        required_keys: List of keys that must be present
        optional_keys: List of keys that may be present (others are invalid)
        name: Name for error messages
    
    Raises:
        ValueError: If validation fails
    """
    if not isinstance(data, dict):
        raise ValueError(f"{name} must be a dictionary, got {type(data)}")
    
    # Check required keys
    missing_keys = set(required_keys) - set(data.keys())
    if missing_keys:
        raise ValueError(f"{name} missing required keys: {missing_keys}")
    
    # Check for unexpected keys if optional_keys specified
    if optional_keys is not None:
        allowed_keys = set(required_keys) | set(optional_keys)
        unexpected_keys = set(data.keys()) - allowed_keys
        if unexpected_keys:
            logger.warning(f"{name} has unexpected keys: {unexpected_keys}")


def safe_get_nested(
    data: Dict,
    keys: list,
    default: Any = None,
    required: bool = False
) -> Any:
    """
    Safely get a nested value from a dictionary.
    
    Args:
        data: The dictionary to search
        keys: List of keys for nested access (e.g., ['a', 'b', 'c'] for data['a']['b']['c'])
        default: Default value if key not found
        required: If True, raise ValueError if key not found
    
    Returns:
        The value or default
    
    Raises:
        ValueError: If required=True and key not found
    """
    current = data
    for i, key in enumerate(keys):
        if not isinstance(current, dict):
            if required:
                raise ValueError(f"Path {keys[:i]} does not lead to a dict")
            return default
        
        if key not in current:
            if required:
                raise ValueError(f"Required key '{key}' not found at path {keys[:i+1]}")
            return default
        
        current = current[key]
    
    return current


def retry_on_exception(
    max_retries: int = 3,
    exceptions: tuple = (Exception,),
    delay: float = 1.0,
    backoff: float = 2.0
):
    """
    Decorator to retry a function on exception.
    
    Args:
        max_retries: Maximum number of retry attempts
        exceptions: Tuple of exception types to catch
        delay: Initial delay between retries in seconds
        backoff: Multiplier for delay after each retry
    
    Returns:
        Decorated function
    """
    def decorator(func: Callable) -> Callable:
        @wraps(func)
        async def async_wrapper(*args, **kwargs):
            import asyncio
            current_delay = delay
            last_exception = None
            
            for attempt in range(max_retries + 1):
                try:
                    return await func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    if attempt < max_retries:
                        logger.warning(
                            f"{func.__name__} failed (attempt {attempt + 1}/{max_retries}): {e}. "
                            f"Retrying in {current_delay}s..."
                        )
                        await asyncio.sleep(current_delay)
                        current_delay *= backoff
                    else:
                        logger.error(
                            f"{func.__name__} failed after {max_retries} retries: {e}"
                        )
            
            raise last_exception
        
        @wraps(func)
        def sync_wrapper(*args, **kwargs):
            import time
            current_delay = delay
            last_exception = None
            
            for attempt in range(max_retries + 1):
                try:
                    return func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    if attempt < max_retries:
                        logger.warning(
                            f"{func.__name__} failed (attempt {attempt + 1}/{max_retries}): {e}. "
                            f"Retrying in {current_delay}s..."
                        )
                        time.sleep(current_delay)
                        current_delay *= backoff
                    else:
                        logger.error(
                            f"{func.__name__} failed after {max_retries} retries: {e}"
                        )
            
            raise last_exception
        
        # Return appropriate wrapper based on whether function is async
        import asyncio
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        else:
            return sync_wrapper
    
    return decorator


class ValidationError(ValueError):
    """Custom exception for validation errors."""
    pass


class ConsensusError(Exception):
    """Base exception for consensus-related errors."""
    pass


class StateError(ConsensusError):
    """Exception for state-related errors."""
    pass


class ScoreError(ConsensusError):
    """Exception for scoring-related errors."""
    pass
