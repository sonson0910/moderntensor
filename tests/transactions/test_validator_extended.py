"""
Extended tests for TransactionValidator to improve coverage.

These tests target the missing coverage in validator.py:
- _validate_stake() method
- _validate_unstake() method
- _validate_common() method with various edge cases
- reset_duplicates() method
- Error handling paths
"""

import pytest
from sdk.transactions.validator import TransactionValidator, ValidationError
from sdk.transactions.types import (
    TransferTransaction,
    StakeTransaction,
    UnstakeTransaction,
    WeightTransaction,
    RegisterTransaction,
    ProposalTransaction,
)


class TestValidateStake:
    """Test stake transaction validation."""
    
    def test_validate_valid_stake(self):
        """Test validating valid stake transaction."""
        validator = TransactionValidator(strict=False)
        
        tx = StakeTransaction(
            from_address="addr1234567890",
            hotkey="hotkey1234567890",
            amount=100.0,
            subnet_id=1
        )
        
        errors = validator.validate(tx)
        assert len(errors) == 0
    
    def test_validate_stake_negative_amount(self):
        """Test stake with negative amount (caught by Pydantic)."""
        with pytest.raises(ValueError):
            StakeTransaction(
                from_address="addr1234567890",
                hotkey="hotkey123",
                amount=-50.0
            )
    
    def test_validate_stake_zero_amount(self):
        """Test stake with zero amount."""
        # Zero should be caught by Pydantic validation
        with pytest.raises(ValueError):
            StakeTransaction(
                from_address="addr1234567890",
                hotkey="hotkey123",
                amount=0.0
            )
    
    def test_validate_stake_invalid_hotkey(self):
        """Test stake with invalid hotkey format."""
        validator = TransactionValidator(strict=False)
        
        tx = StakeTransaction(
            from_address="addr1234567890",
            hotkey="short",  # Too short
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("hotkey" in e.lower() for e in errors)
    
    def test_validate_stake_empty_hotkey(self):
        """Test stake with empty hotkey."""
        validator = TransactionValidator(strict=False)
        
        tx = StakeTransaction(
            from_address="addr1234567890",
            hotkey="",  # Empty
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("hotkey" in e.lower() for e in errors)
    
    def test_validate_stake_strict_mode(self):
        """Test stake validation in strict mode."""
        validator = TransactionValidator(strict=True)
        
        tx = StakeTransaction(
            from_address="addr1234567890",
            hotkey="bad",  # Invalid
            amount=100.0
        )
        
        with pytest.raises(ValidationError):
            validator.validate(tx)


class TestValidateUnstake:
    """Test unstake transaction validation."""
    
    def test_validate_valid_unstake(self):
        """Test validating valid unstake transaction."""
        validator = TransactionValidator(strict=False)
        
        tx = UnstakeTransaction(
            from_address="addr1234567890",
            hotkey="hotkey1234567890",
            amount=50.0,
            subnet_id=1
        )
        
        errors = validator.validate(tx)
        assert len(errors) == 0
    
    def test_validate_unstake_negative_amount(self):
        """Test unstake with negative amount."""
        with pytest.raises(ValueError):
            UnstakeTransaction(
                from_address="addr1234567890",
                hotkey="hotkey123",
                amount=-25.0
            )
    
    def test_validate_unstake_zero_amount(self):
        """Test unstake with zero amount."""
        with pytest.raises(ValueError):
            UnstakeTransaction(
                from_address="addr1234567890",
                hotkey="hotkey123",
                amount=0.0
            )
    
    def test_validate_unstake_invalid_hotkey(self):
        """Test unstake with invalid hotkey."""
        validator = TransactionValidator(strict=False)
        
        tx = UnstakeTransaction(
            from_address="addr1234567890",
            hotkey="xyz",  # Too short
            amount=50.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("hotkey" in e.lower() for e in errors)
    
    def test_validate_unstake_empty_hotkey(self):
        """Test unstake with empty hotkey."""
        validator = TransactionValidator(strict=False)
        
        tx = UnstakeTransaction(
            from_address="addr1234567890",
            hotkey="",
            amount=50.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
    
    def test_validate_unstake_strict_mode(self):
        """Test unstake validation in strict mode."""
        validator = TransactionValidator(strict=True)
        
        tx = UnstakeTransaction(
            from_address="addr1234567890",
            hotkey="bad",
            amount=50.0
        )
        
        with pytest.raises(ValidationError):
            validator.validate(tx)


class TestValidateCommon:
    """Test common field validation."""
    
    def test_validate_short_from_address(self):
        """Test validation with short from_address."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="short",  # Too short
            to_address="addr1234567890",
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("from_address" in e.lower() for e in errors)
    
    def test_validate_empty_from_address(self):
        """Test validation with empty from_address."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="",  # Empty
            to_address="addr1234567890",
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("from_address" in e.lower() for e in errors)
    
    def test_validate_negative_fee(self):
        """Test validation with negative fee."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0,
            fee=-0.01  # Negative fee
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("fee" in e.lower() for e in errors)
    
    def test_validate_negative_nonce(self):
        """Test validation with negative nonce."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0,
            nonce=-5  # Negative nonce
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("nonce" in e.lower() for e in errors)
    
    def test_validate_zero_fee(self):
        """Test validation with zero fee (should be valid)."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0,
            fee=0.0  # Zero fee is valid
        )
        
        errors = validator.validate(tx)
        # Should not have fee-related errors
        assert not any("fee" in e.lower() for e in errors)
    
    def test_validate_zero_nonce(self):
        """Test validation with zero nonce (should be valid)."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0,
            nonce=0  # Zero nonce is valid
        )
        
        errors = validator.validate(tx)
        # Should not have nonce-related errors
        assert not any("nonce" in e.lower() for e in errors)
    
    def test_validate_multiple_common_errors(self):
        """Test validation with multiple common field errors."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="bad",  # Too short
            to_address="bad",  # Too short (and same)
            amount=100.0,
            fee=-0.01,  # Negative
            nonce=-1  # Negative
        )
        
        errors = validator.validate(tx)
        # Should have multiple errors
        assert len(errors) >= 3
        assert any("from_address" in e.lower() for e in errors)
        assert any("fee" in e.lower() for e in errors)
        assert any("nonce" in e.lower() for e in errors)


class TestValidateWeights:
    """Test weight transaction validation edge cases."""
    
    def test_validate_weights_mismatched_lengths(self):
        """Test weights with mismatched UID and weight counts."""
        validator = TransactionValidator(strict=False)
        
        # This should fail at Pydantic validation during construction
        with pytest.raises(ValueError):
            WeightTransaction(
                from_address="addr1234567890",
                subnet_id=1,
                uids=[1, 2, 3],
                weights=[0.5, 0.5],  # Mismatch
                version_key=1
            )
    
    def test_validate_weights_negative_weight(self):
        """Test weights with negative values."""
        validator = TransactionValidator(strict=False)
        
        # This should fail at Pydantic validation
        with pytest.raises(ValueError):
            WeightTransaction(
                from_address="addr1234567890",
                subnet_id=1,
                uids=[1, 2],
                weights=[0.6, -0.6],  # Negative weight
                version_key=1
            )
    
    def test_validate_weights_duplicate_uids(self):
        """Test weights with duplicate UIDs."""
        validator = TransactionValidator(strict=False)
        
        # Duplicate UIDs may be caught by validator, not Pydantic
        tx = WeightTransaction(
            from_address="addr1234567890",
            subnet_id=1,
            uids=[1, 1, 2],  # Duplicate UID
            weights=[0.4, 0.4, 0.2],
            version_key=1
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("duplicate" in e.lower() for e in errors)
    
    def test_validate_weights_invalid_sum_low(self):
        """Test weights with sum too low."""
        validator = TransactionValidator(strict=False)
        
        with pytest.raises(ValueError):
            WeightTransaction(
                from_address="addr1234567890",
                subnet_id=1,
                uids=[1, 2],
                weights=[0.3, 0.3],  # Sum = 0.6, too low
                version_key=1
            )
    
    def test_validate_weights_invalid_sum_high(self):
        """Test weights with sum too high."""
        validator = TransactionValidator(strict=False)
        
        with pytest.raises(ValueError):
            WeightTransaction(
                from_address="addr1234567890",
                subnet_id=1,
                uids=[1, 2],
                weights=[0.7, 0.7],  # Sum = 1.4, too high
                version_key=1
            )


class TestBatchValidation:
    """Test batch validation functionality."""
    
    def test_validate_batch_all_valid(self):
        """Test batch validation with all valid transactions."""
        validator = TransactionValidator(strict=False)
        
        transactions = [
            TransferTransaction(
                from_address="addr1234567890",
                to_address="addr0987654321",
                amount=100.0
            ),
            StakeTransaction(
                from_address="addr1234567890",
                hotkey="hotkey1234567890",
                amount=50.0
            ),
            UnstakeTransaction(
                from_address="addr1234567890",
                hotkey="hotkey1234567890",
                amount=25.0
            ),
        ]
        
        results = validator.validate_batch(transactions)
        assert len(results) == 0  # No errors
    
    def test_validate_batch_some_invalid(self):
        """Test batch validation with some invalid transactions."""
        validator = TransactionValidator(strict=False)
        
        transactions = [
            TransferTransaction(
                from_address="addr1234567890",
                to_address="addr0987654321",
                amount=100.0
            ),
            TransferTransaction(
                from_address="bad",  # Invalid
                to_address="addr2",
                amount=50.0
            ),
            StakeTransaction(
                from_address="addr1234567890",
                hotkey="bad",  # Invalid
                amount=30.0
            ),
        ]
        
        results = validator.validate_batch(transactions)
        assert len(results) == 2  # Transactions at index 1 and 2 have errors
        assert 1 in results
        assert 2 in results
    
    def test_validate_batch_empty(self):
        """Test batch validation with empty list."""
        validator = TransactionValidator(strict=False)
        
        results = validator.validate_batch([])
        assert len(results) == 0


class TestDuplicateDetection:
    """Test duplicate transaction detection."""
    
    def test_check_duplicate_same_transaction(self):
        """Test duplicate detection with same transaction."""
        validator = TransactionValidator()
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0
        )
        
        # First check should not be duplicate
        assert not validator.check_duplicate(tx)
        
        # Second check should be duplicate
        assert validator.check_duplicate(tx)
        
        # Third check should still be duplicate
        assert validator.check_duplicate(tx)
    
    def test_check_duplicate_different_transactions(self):
        """Test that different transactions are not duplicates."""
        validator = TransactionValidator()
        
        tx1 = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0
        )
        
        tx2 = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=200.0  # Different amount
        )
        
        assert not validator.check_duplicate(tx1)
        assert not validator.check_duplicate(tx2)  # Different transaction
    
    def test_reset_duplicates(self):
        """Test resetting duplicate detection cache."""
        validator = TransactionValidator()
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0
        )
        
        # Mark as seen
        assert not validator.check_duplicate(tx)
        assert validator.check_duplicate(tx)  # Duplicate
        
        # Reset cache
        validator.reset_duplicates()
        
        # Should not be duplicate anymore
        assert not validator.check_duplicate(tx)
    
    def test_duplicate_with_different_optional_fields(self):
        """Test that transactions with different optional fields are different."""
        validator = TransactionValidator()
        
        tx1 = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0,
            fee=0.01
        )
        
        tx2 = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0,
            fee=0.02  # Different fee
        )
        
        assert not validator.check_duplicate(tx1)
        assert not validator.check_duplicate(tx2)  # Different due to fee


class TestStrictMode:
    """Test strict validation mode."""
    
    def test_strict_mode_raises_exception(self):
        """Test that strict mode raises exceptions."""
        validator = TransactionValidator(strict=True)
        
        tx = TransferTransaction(
            from_address="bad",  # Invalid
            to_address="addr1234567890",
            amount=100.0
        )
        
        with pytest.raises(ValidationError) as exc_info:
            validator.validate(tx)
        
        assert "validation failed" in str(exc_info.value).lower()
    
    def test_non_strict_mode_returns_errors(self):
        """Test that non-strict mode returns errors without raising."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="bad",  # Invalid
            to_address="addr1234567890",
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
    
    def test_strict_mode_with_valid_transaction(self):
        """Test that strict mode doesn't raise for valid transactions."""
        validator = TransactionValidator(strict=True)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0
        )
        
        # Should not raise
        errors = validator.validate(tx)
        assert len(errors) == 0


class TestUnsupportedTransactionTypes:
    """Test validation of transaction types without specific validators."""
    
    def test_validate_register_transaction(self):
        """Test validating register transaction (uses only common validation)."""
        validator = TransactionValidator(strict=False)
        
        tx = RegisterTransaction(
            from_address="addr1234567890",
            subnet_id=1,
            hotkey="hotkey1234567890"
        )
        
        errors = validator.validate(tx)
        # Should only check common fields
        assert len(errors) == 0
    
    def test_validate_proposal_transaction(self):
        """Test validating proposal transaction (uses only common validation)."""
        validator = TransactionValidator(strict=False)
        
        tx = ProposalTransaction(
            from_address="addr1234567890",
            title="Test Proposal",
            description="Test Description",
            proposal_type="parameter_change",
            options=["Yes", "No"],
            duration_blocks=1000
        )
        
        errors = validator.validate(tx)
        # Should only check common fields
        assert len(errors) == 0
    
    def test_validate_unsupported_with_invalid_common_fields(self):
        """Test unsupported type with invalid common fields."""
        validator = TransactionValidator(strict=False)
        
        tx = RegisterTransaction(
            from_address="bad",  # Invalid
            subnet_id=1,
            hotkey="hotkey123",
            fee=-0.01  # Invalid
        )
        
        errors = validator.validate(tx)
        # Should catch common field errors
        assert len(errors) > 0
