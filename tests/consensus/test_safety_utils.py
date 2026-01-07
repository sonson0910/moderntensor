# tests/consensus/test_safety_utils.py
"""
Tests for consensus safety utilities.
"""
import pytest
import math
from sdk.consensus.safety_utils import (
    safe_divide,
    safe_mean,
    clamp,
    validate_score,
    validate_uid,
    validate_dict_structure,
    safe_get_nested,
    ValidationError,
    EPSILON
)


class TestSafeDivide:
    """Tests for safe_divide function."""
    
    def test_normal_division(self):
        """Test normal division operation."""
        assert safe_divide(10.0, 2.0) == 5.0
        assert safe_divide(100.0, 4.0) == 25.0
        assert safe_divide(-10.0, 2.0) == -5.0
    
    def test_division_by_zero(self):
        """Test division by exactly zero."""
        assert safe_divide(10.0, 0.0) == 0.0
        assert safe_divide(10.0, 0.0, default=-1.0) == -1.0
    
    def test_division_by_near_zero(self):
        """Test division by very small numbers."""
        assert safe_divide(10.0, 1e-10) == 0.0  # Below EPSILON
        assert safe_divide(10.0, 1e-10, default=999.0) == 999.0
    
    def test_division_by_negative_zero(self):
        """Test division by negative near-zero."""
        assert safe_divide(10.0, -1e-10) == 0.0
    
    def test_division_with_floats(self):
        """Test division with float results."""
        result = safe_divide(10.0, 3.0)
        assert abs(result - 3.333333) < 0.00001
    
    def test_division_overflow_protection(self):
        """Test protection against overflow."""
        # This should be handled gracefully
        result = safe_divide(1e308, 1e-308, default=0.0)
        # Python handles this, but our function should still work
        assert isinstance(result, (int, float))


class TestSafeMean:
    """Tests for safe_mean function."""
    
    def test_normal_mean(self):
        """Test normal mean calculation."""
        assert safe_mean([1.0, 2.0, 3.0, 4.0, 5.0]) == 3.0
        assert safe_mean([10.0, 20.0, 30.0]) == 20.0
    
    def test_empty_list(self):
        """Test mean of empty list."""
        assert safe_mean([]) == 0.0
        assert safe_mean([], default=999.0) == 999.0
    
    def test_single_value(self):
        """Test mean of single value."""
        assert safe_mean([42.0]) == 42.0
    
    def test_negative_values(self):
        """Test mean with negative values."""
        assert safe_mean([-1.0, -2.0, -3.0]) == -2.0
    
    def test_mixed_values(self):
        """Test mean with mixed positive/negative."""
        assert safe_mean([10.0, -10.0, 5.0, -5.0]) == 0.0
    
    def test_none_in_list(self):
        """Test handling of None in list."""
        # Should return default due to TypeError
        assert safe_mean([1.0, None, 3.0], default=-1.0) == -1.0


class TestClamp:
    """Tests for clamp function."""
    
    def test_within_bounds(self):
        """Test value within bounds."""
        assert clamp(0.5, 0.0, 1.0) == 0.5
        assert clamp(50.0, 0.0, 100.0) == 50.0
    
    def test_below_min(self):
        """Test value below minimum."""
        assert clamp(-1.0, 0.0, 1.0) == 0.0
        assert clamp(5.0, 10.0, 20.0) == 10.0
    
    def test_above_max(self):
        """Test value above maximum."""
        assert clamp(2.0, 0.0, 1.0) == 1.0
        assert clamp(100.0, 0.0, 50.0) == 50.0
    
    def test_at_boundaries(self):
        """Test values at exact boundaries."""
        assert clamp(0.0, 0.0, 1.0) == 0.0
        assert clamp(1.0, 0.0, 1.0) == 1.0
    
    def test_invalid_bounds(self):
        """Test with invalid min > max."""
        with pytest.raises(ValueError):
            clamp(0.5, 1.0, 0.0)  # min > max
    
    def test_negative_bounds(self):
        """Test with negative bounds."""
        assert clamp(-5.0, -10.0, 0.0) == -5.0
        assert clamp(-15.0, -10.0, 0.0) == -10.0
        assert clamp(5.0, -10.0, 0.0) == 0.0


class TestValidateScore:
    """Tests for validate_score function."""
    
    def test_valid_scores(self):
        """Test valid scores."""
        assert validate_score(0.0) == 0.0
        assert validate_score(0.5) == 0.5
        assert validate_score(1.0) == 1.0
    
    def test_clamping(self):
        """Test score clamping."""
        assert validate_score(-0.1) == 0.0
        assert validate_score(1.1) == 1.0
        assert validate_score(2.0) == 1.0
        assert validate_score(-5.0) == 0.0
    
    def test_nan_detection(self):
        """Test NaN detection."""
        with pytest.raises(ValueError, match="NaN"):
            validate_score(float('nan'))
    
    def test_infinity_detection(self):
        """Test infinity detection."""
        with pytest.raises(ValueError, match="infinite"):
            validate_score(float('inf'))
        
        with pytest.raises(ValueError, match="infinite"):
            validate_score(float('-inf'))
    
    def test_custom_name(self):
        """Test custom name in error messages."""
        with pytest.raises(ValueError, match="trust_score"):
            validate_score(float('nan'), name="trust_score")


class TestValidateUid:
    """Tests for validate_uid function."""
    
    def test_valid_uid(self):
        """Test valid hex UID."""
        assert validate_uid("1a2b3c4d") == "1a2b3c4d"
        assert validate_uid("ABCDEF123456") == "ABCDEF123456"
        assert validate_uid("0123456789abcdef") == "0123456789abcdef"
    
    def test_empty_uid(self):
        """Test empty UID."""
        with pytest.raises(ValueError, match="empty"):
            validate_uid("")
        
        with pytest.raises(ValueError, match="empty or None"):
            validate_uid(None)
    
    def test_invalid_hex(self):
        """Test invalid hex string."""
        with pytest.raises(ValueError, match="not a valid hex"):
            validate_uid("xyz")
        
        with pytest.raises(ValueError, match="not a valid hex"):
            validate_uid("12g4")
    
    def test_custom_name(self):
        """Test custom name in error messages."""
        with pytest.raises(ValueError, match="miner_uid"):
            validate_uid("", name="miner_uid")
    
    def test_numeric_uid(self):
        """Test numeric UID (should convert to string and validate as decimal, not hex)."""
        # Note: '123' is valid hex (equals 291 decimal), but this tests basic conversion
        result = validate_uid(123)
        assert result == "123"
        # Verify it's treated as valid hex
        assert int(result, 16) == 0x123  # 291 in decimal


class TestValidateDictStructure:
    """Tests for validate_dict_structure function."""
    
    def test_valid_structure(self):
        """Test valid dictionary structure."""
        data = {"key1": "value1", "key2": "value2"}
        validate_dict_structure(data, required_keys=["key1", "key2"])
        # Should not raise
    
    def test_missing_required_key(self):
        """Test missing required key."""
        data = {"key1": "value1"}
        with pytest.raises(ValueError, match="missing required keys"):
            validate_dict_structure(data, required_keys=["key1", "key2"])
    
    def test_optional_keys(self):
        """Test with optional keys."""
        data = {"key1": "value1", "key2": "value2", "key3": "value3"}
        validate_dict_structure(
            data,
            required_keys=["key1"],
            optional_keys=["key2", "key3"]
        )
        # Should not raise
    
    def test_unexpected_keys(self):
        """Test with unexpected keys (should warn, not fail)."""
        data = {"key1": "value1", "unexpected": "value"}
        # Should complete without raising, but log warning
        validate_dict_structure(
            data,
            required_keys=["key1"],
            optional_keys=[]
        )
    
    def test_not_dict(self):
        """Test with non-dict input."""
        with pytest.raises(ValueError, match="must be a dictionary"):
            validate_dict_structure("not a dict", required_keys=["key1"])
    
    def test_custom_name(self):
        """Test custom name in error messages."""
        with pytest.raises(ValueError, match="my_data"):
            validate_dict_structure({}, required_keys=["key1"], name="my_data")


class TestSafeGetNested:
    """Tests for safe_get_nested function."""
    
    def test_simple_get(self):
        """Test simple nested access."""
        data = {"a": {"b": {"c": "value"}}}
        assert safe_get_nested(data, ["a", "b", "c"]) == "value"
    
    def test_missing_key_default(self):
        """Test missing key with default."""
        data = {"a": {"b": "value"}}
        assert safe_get_nested(data, ["a", "x"], default="default") == "default"
    
    def test_missing_key_required(self):
        """Test missing key with required=True."""
        data = {"a": {"b": "value"}}
        with pytest.raises(ValueError, match="not found"):
            safe_get_nested(data, ["a", "x"], required=True)
    
    def test_not_dict_in_path(self):
        """Test non-dict in path."""
        data = {"a": "string_value"}
        assert safe_get_nested(data, ["a", "b"], default="default") == "default"
    
    def test_not_dict_required(self):
        """Test non-dict in path with required=True."""
        data = {"a": "string_value"}
        with pytest.raises(ValueError, match="does not lead to a dict"):
            safe_get_nested(data, ["a", "b"], required=True)
    
    def test_empty_keys(self):
        """Test empty key list."""
        data = {"a": "value"}
        assert safe_get_nested(data, []) == data
    
    def test_single_key(self):
        """Test single key access."""
        data = {"key": "value"}
        assert safe_get_nested(data, ["key"]) == "value"


class TestConstants:
    """Tests for module constants."""
    
    def test_epsilon_value(self):
        """Test EPSILON constant."""
        assert EPSILON == 1e-9
        assert EPSILON > 0
        assert EPSILON < 0.001


class TestExceptions:
    """Tests for custom exceptions."""
    
    def test_validation_error(self):
        """Test ValidationError exception."""
        with pytest.raises(ValidationError):
            raise ValidationError("test error")
    
    def test_validation_error_is_value_error(self):
        """Test that ValidationError inherits from ValueError."""
        assert issubclass(ValidationError, ValueError)


@pytest.mark.asyncio
class TestRetryDecorator:
    """Tests for retry decorator (placeholder for future implementation)."""
    
    async def test_placeholder(self):
        """Placeholder test for retry decorator."""
        # This would test the retry_on_exception decorator
        # when it's fully implemented
        pass


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
