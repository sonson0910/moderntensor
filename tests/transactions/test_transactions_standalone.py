"""
Standalone tests for transaction system (no SDK dependencies)
"""

import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../..'))

import pytest


def test_transaction_types_import():
    """Test importing transaction types."""
    from sdk.transactions.types import (
        TransactionType,
        TransferTransaction,
        StakeTransaction,
        WeightTransaction,
    )
    
    assert TransactionType.TRANSFER == "transfer"
    assert TransactionType.STAKE == "stake"


def test_transfer_transaction():
    """Test transfer transaction creation."""
    from sdk.transactions.types import TransferTransaction, TransactionType
    
    tx = TransferTransaction(
        from_address="addr1",
        to_address="addr2",
        amount=100.0
    )
    
    assert tx.transaction_type == TransactionType.TRANSFER
    assert tx.from_address == "addr1"
    assert tx.to_address == "addr2"
    assert tx.amount == 100.0


def test_transfer_invalid_amount():
    """Test transfer with invalid amount."""
    from sdk.transactions.types import TransferTransaction
    from pydantic import ValidationError
    
    with pytest.raises(ValidationError):
        TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=-10.0
        )


def test_stake_transaction():
    """Test stake transaction creation."""
    from sdk.transactions.types import StakeTransaction, TransactionType
    
    tx = StakeTransaction(
        from_address="addr1",
        hotkey="hotkey123",
        amount=50.0,
        subnet_id=1
    )
    
    assert tx.transaction_type == TransactionType.STAKE
    assert tx.hotkey == "hotkey123"
    assert tx.amount == 50.0
    assert tx.subnet_id == 1


def test_weight_transaction():
    """Test weight transaction creation."""
    from sdk.transactions.types import WeightTransaction, TransactionType
    
    tx = WeightTransaction(
        from_address="addr1",
        subnet_id=1,
        uids=[1, 2, 3],
        weights=[0.5, 0.3, 0.2],
        version_key=1
    )
    
    assert tx.transaction_type == TransactionType.SET_WEIGHTS
    assert len(tx.uids) == 3
    assert len(tx.weights) == 3
    assert abs(sum(tx.weights) - 1.0) < 0.01


def test_weight_invalid_sum():
    """Test weight transaction with invalid sum."""
    from sdk.transactions.types import WeightTransaction
    from pydantic import ValidationError
    
    with pytest.raises(ValidationError):
        WeightTransaction(
            from_address="addr1",
            subnet_id=1,
            uids=[1, 2, 3],
            weights=[0.5, 0.3, 0.3],  # Sum = 1.1
            version_key=1
        )


def test_transaction_builder():
    """Test transaction builder."""
    from sdk.transactions.builder import TransactionBuilder
    from sdk.transactions.types import TransferTransaction, StakeTransaction
    
    # Test transfer
    tx = TransactionBuilder() \
        .transfer("addr1", "addr2", 100.0) \
        .with_fee(0.01) \
        .with_memo("Test transfer") \
        .build()
    
    assert isinstance(tx, TransferTransaction)
    assert tx.amount == 100.0
    assert tx.fee == 0.01
    assert tx.memo == "Test transfer"


def test_builder_stake():
    """Test building stake transaction."""
    from sdk.transactions.builder import TransactionBuilder
    from sdk.transactions.types import StakeTransaction
    
    tx = TransactionBuilder() \
        .stake("addr1", "hotkey123", 50.0) \
        .with_nonce(5) \
        .build()
    
    assert isinstance(tx, StakeTransaction)
    assert tx.amount == 50.0
    assert tx.nonce == 5


def test_builder_reset():
    """Test builder reset."""
    from sdk.transactions.builder import TransactionBuilder
    from sdk.transactions.types import TransferTransaction, StakeTransaction
    
    builder = TransactionBuilder()
    
    tx1 = builder.transfer("addr1", "addr2", 100.0).build()
    assert isinstance(tx1, TransferTransaction)
    
    builder.reset()
    tx2 = builder.stake("addr1", "hotkey", 50.0).build()
    assert isinstance(tx2, StakeTransaction)


def test_builder_without_type():
    """Test building without setting type."""
    from sdk.transactions.builder import TransactionBuilder
    
    builder = TransactionBuilder()
    
    with pytest.raises(ValueError):
        builder.build()


def test_transaction_validator():
    """Test transaction validator."""
    from sdk.transactions.validator import TransactionValidator
    from sdk.transactions.types import TransferTransaction
    
    validator = TransactionValidator(strict=False)
    
    tx = TransferTransaction(
        from_address="addr1234567890",
        to_address="addr0987654321",
        amount=100.0
    )
    
    errors = validator.validate(tx)
    assert len(errors) == 0


def test_batch_builder():
    """Test batch transaction builder."""
    from sdk.transactions.batch import BatchTransactionBuilder
    from sdk.transactions.types import TransferTransaction, StakeTransaction, TransactionType
    
    batch = BatchTransactionBuilder()
    
    batch.add_transaction(TransferTransaction(
        from_address="addr1",
        to_address="addr2",
        amount=100.0
    ))
    batch.add_transaction(StakeTransaction(
        from_address="addr1",
        hotkey="hotkey123",
        amount=50.0
    ))
    
    assert batch.count() == 2
    
    transfers = batch.get_transactions_by_type(TransactionType.TRANSFER)
    assert len(transfers) == 1


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
