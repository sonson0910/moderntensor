"""
Tests for Balance Utilities

Comprehensive test suite for balance operations, formatting, and conversions.
"""

import pytest
from decimal import Decimal
from sdk.utils.balance import (
    Balance,
    BalanceError,
    format_balance,
    parse_balance,
    tao_to_rao,
    rao_to_tao,
    validate_balance,
    calculate_percentage,
    sum_balances,
    RAO_PER_TAO,
)


class TestBalance:
    """Test Balance class operations."""
    
    def test_balance_from_rao(self):
        """Test creating balance from RAO."""
        balance = Balance.from_rao(1000000000)
        assert balance.rao == 1000000000
        assert balance.tao == Decimal('1')
    
    def test_balance_from_tao(self):
        """Test creating balance from TAO."""
        balance = Balance.from_tao(1.5)
        assert balance.rao == 1500000000
        assert balance.tao == Decimal('1.5')
    
    def test_balance_from_string_tao(self):
        """Test creating balance from string TAO."""
        balance = Balance.from_tao("0.000000001")
        assert balance.rao == 1
        assert balance.tao == Decimal('0.000000001')
    
    def test_balance_negative_raises(self):
        """Test that negative balance raises error."""
        with pytest.raises(BalanceError):
            Balance.from_rao(-100)
    
    def test_balance_exceeds_max_raises(self):
        """Test that balance exceeding max supply raises error."""
        max_supply = 21_000_000 * RAO_PER_TAO
        with pytest.raises(BalanceError):
            Balance.from_rao(max_supply + 1)
    
    def test_balance_addition(self):
        """Test adding two balances."""
        b1 = Balance.from_tao(10)
        b2 = Balance.from_tao(20)
        result = b1 + b2
        assert result.tao == Decimal('30')
    
    def test_balance_subtraction(self):
        """Test subtracting two balances."""
        b1 = Balance.from_tao(30)
        b2 = Balance.from_tao(10)
        result = b1 - b2
        assert result.tao == Decimal('20')
    
    def test_balance_subtraction_negative_raises(self):
        """Test that subtraction resulting in negative raises error."""
        b1 = Balance.from_tao(10)
        b2 = Balance.from_tao(20)
        with pytest.raises(BalanceError):
            _ = b1 - b2
    
    def test_balance_multiplication(self):
        """Test multiplying balance by scalar."""
        balance = Balance.from_tao(10)
        result = balance * 2.5
        assert result.tao == Decimal('25')
    
    def test_balance_division(self):
        """Test dividing balance by scalar."""
        balance = Balance.from_tao(100)
        result = balance / 4
        assert result.tao == Decimal('25')
    
    def test_balance_division_by_zero_raises(self):
        """Test that division by zero raises error."""
        balance = Balance.from_tao(100)
        with pytest.raises(BalanceError):
            _ = balance / 0
    
    def test_balance_equality(self):
        """Test balance equality comparison."""
        b1 = Balance.from_tao(10)
        b2 = Balance.from_tao(10)
        b3 = Balance.from_tao(20)
        assert b1 == b2
        assert not (b1 == b3)
    
    def test_balance_comparison(self):
        """Test balance comparison operators."""
        b1 = Balance.from_tao(10)
        b2 = Balance.from_tao(20)
        b3 = Balance.from_tao(10)
        
        assert b1 < b2
        assert b2 > b1
        assert b1 <= b3
        assert b1 >= b3
    
    def test_balance_string_representation(self):
        """Test balance string representation."""
        balance = Balance.from_tao(1.5)
        assert "1.5" in str(balance)
        assert "TAO" in str(balance)
    
    def test_balance_repr(self):
        """Test balance repr."""
        balance = Balance.from_rao(1500000000)
        assert "Balance(1500000000 RAO)" == repr(balance)
    
    def test_balance_hash(self):
        """Test balance hashing for use in sets/dicts."""
        b1 = Balance.from_tao(10)
        b2 = Balance.from_tao(10)
        b3 = Balance.from_tao(20)
        
        # Equal balances should have same hash
        assert hash(b1) == hash(b2)
        
        # Can use in set
        balance_set = {b1, b2, b3}
        assert len(balance_set) == 2  # b1 and b2 are equal


class TestFormatBalance:
    """Test balance formatting functions."""
    
    def test_format_balance_tao(self):
        """Test formatting balance as TAO."""
        result = format_balance(1500000000, unit="TAO", decimals=2)
        assert result == "1.50 TAO"
    
    def test_format_balance_rao(self):
        """Test formatting balance as RAO."""
        result = format_balance(1500000000, unit="RAO")
        assert result == "1,500,000,000 RAO"
    
    def test_format_balance_no_unit(self):
        """Test formatting without unit suffix."""
        result = format_balance(1500000000, unit="TAO", include_unit=False)
        assert "TAO" not in result
        assert "1.5" in result
    
    def test_format_balance_no_separator(self):
        """Test formatting without thousands separator."""
        result = format_balance(1500000000, unit="RAO", thousands_separator=False)
        assert result == "1500000000 RAO"
    
    def test_format_balance_from_float(self):
        """Test formatting from float input."""
        result = format_balance(1.5, unit="TAO", decimals=1)
        assert result == "1.5 TAO"
    
    def test_format_balance_from_balance_object(self):
        """Test formatting from Balance object."""
        balance = Balance.from_tao(1.5)
        result = format_balance(balance, decimals=2)
        assert result == "1.50 TAO"


class TestParseBalance:
    """Test balance parsing functions."""
    
    def test_parse_balance_tao(self):
        """Test parsing TAO balance."""
        balance = parse_balance("1.5 TAO")
        assert balance.tao == Decimal('1.5')
    
    def test_parse_balance_rao(self):
        """Test parsing RAO balance."""
        balance = parse_balance("1,500,000,000 RAO")
        assert balance.rao == 1500000000
    
    def test_parse_balance_no_unit_decimal(self):
        """Test parsing without unit (with decimal, assumes TAO)."""
        balance = parse_balance("1.5")
        assert balance.tao == Decimal('1.5')
    
    def test_parse_balance_no_unit_integer(self):
        """Test parsing without unit (no decimal, assumes RAO)."""
        balance = parse_balance("1500000000")
        assert balance.rao == 1500000000
    
    def test_parse_balance_with_commas(self):
        """Test parsing with comma separators."""
        balance = parse_balance("1,500,000,000")
        assert balance.rao == 1500000000


class TestConversionFunctions:
    """Test conversion utility functions."""
    
    def test_tao_to_rao(self):
        """Test TAO to RAO conversion."""
        rao = tao_to_rao(1.5)
        assert rao == 1500000000
    
    def test_tao_to_rao_string(self):
        """Test TAO to RAO conversion from string."""
        rao = tao_to_rao("0.000000001")
        assert rao == 1
    
    def test_rao_to_tao(self):
        """Test RAO to TAO conversion."""
        tao = rao_to_tao(1500000000)
        assert tao == Decimal('1.5')
    
    def test_rao_to_tao_small(self):
        """Test RAO to TAO conversion for small amounts."""
        tao = rao_to_tao(1)
        assert tao == Decimal('0.000000001')


class TestValidateBalance:
    """Test balance validation functions."""
    
    def test_validate_balance_no_limits(self):
        """Test validation without limits."""
        assert validate_balance(Balance.from_tao(100))
    
    def test_validate_balance_with_min(self):
        """Test validation with minimum."""
        assert validate_balance(
            Balance.from_tao(100),
            min_balance=Balance.from_tao(10)
        )
        assert not validate_balance(
            Balance.from_tao(5),
            min_balance=Balance.from_tao(10)
        )
    
    def test_validate_balance_with_max(self):
        """Test validation with maximum."""
        assert validate_balance(
            Balance.from_tao(50),
            max_balance=Balance.from_tao(100)
        )
        assert not validate_balance(
            Balance.from_tao(150),
            max_balance=Balance.from_tao(100)
        )
    
    def test_validate_balance_with_both_limits(self):
        """Test validation with both min and max."""
        assert validate_balance(
            Balance.from_tao(50),
            min_balance=Balance.from_tao(10),
            max_balance=Balance.from_tao(100)
        )


class TestCalculations:
    """Test calculation utility functions."""
    
    def test_calculate_percentage(self):
        """Test percentage calculation."""
        percentage = calculate_percentage(
            Balance.from_tao(25),
            Balance.from_tao(100)
        )
        assert percentage == Decimal('25.00')
    
    def test_calculate_percentage_zero_total(self):
        """Test percentage with zero total."""
        percentage = calculate_percentage(
            Balance.from_tao(25),
            Balance.from_tao(0)
        )
        assert percentage == Decimal('0')
    
    def test_calculate_percentage_custom_decimals(self):
        """Test percentage with custom decimal places."""
        percentage = calculate_percentage(
            Balance.from_tao(33.333),
            Balance.from_tao(100),
            decimals=3
        )
        assert percentage == Decimal('33.333')
    
    def test_sum_balances(self):
        """Test summing multiple balances."""
        balances = [
            Balance.from_tao(10),
            Balance.from_tao(20),
            Balance.from_tao(30)
        ]
        total = sum_balances(balances)
        assert total.tao == Decimal('60')
    
    def test_sum_balances_empty(self):
        """Test summing empty list."""
        total = sum_balances([])
        assert total.tao == Decimal('0')
    
    def test_sum_balances_single(self):
        """Test summing single balance."""
        balances = [Balance.from_tao(42)]
        total = sum_balances(balances)
        assert total.tao == Decimal('42')


class TestEdgeCases:
    """Test edge cases and error conditions."""
    
    def test_very_small_amount(self):
        """Test handling very small amounts (1 RAO)."""
        balance = Balance.from_rao(1)
        assert balance.rao == 1
        assert balance.tao == Decimal('0.000000001')
    
    def test_very_large_amount(self):
        """Test handling very large amounts (max supply)."""
        max_supply_rao = 21_000_000 * RAO_PER_TAO
        balance = Balance.from_rao(max_supply_rao)
        assert balance.rao == max_supply_rao
        assert balance.tao == Decimal('21000000')
    
    def test_invalid_tao_string(self):
        """Test invalid TAO string raises error."""
        with pytest.raises(BalanceError):
            Balance.from_tao("invalid")
    
    def test_balance_type_error_in_operations(self):
        """Test type errors in balance operations."""
        balance = Balance.from_tao(10)
        
        with pytest.raises(TypeError):
            _ = balance + 10  # Should be Balance object
        
        with pytest.raises(TypeError):
            _ = balance < 10  # Should be Balance object


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
