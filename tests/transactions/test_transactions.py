"""
Tests for transaction system
"""

import pytest
from sdk.transactions.types import (
    TransactionType,
    TransferTransaction,
    StakeTransaction,
    UnstakeTransaction,
    RegisterTransaction,
    WeightTransaction,
    ProposalTransaction,
)
from sdk.transactions.builder import TransactionBuilder
from sdk.transactions.validator import TransactionValidator, ValidationError
from sdk.transactions.batch import BatchTransactionBuilder


class TestTransactionTypes:
    """Test transaction type models."""
    
    def test_transfer_transaction(self):
        """Test transfer transaction creation."""
        tx = TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        )
        
        assert tx.transaction_type == TransactionType.TRANSFER
        assert tx.from_address == "addr1"
        assert tx.to_address == "addr2"
        assert tx.amount == 100.0
    
    def test_transfer_invalid_amount(self):
        """Test transfer with invalid amount."""
        with pytest.raises(ValueError):
            TransferTransaction(
                from_address="addr1",
                to_address="addr2",
                amount=-10.0
            )
    
    def test_stake_transaction(self):
        """Test stake transaction creation."""
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
    
    def test_weight_transaction(self):
        """Test weight transaction creation."""
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
    
    def test_weight_invalid_sum(self):
        """Test weight transaction with invalid sum."""
        with pytest.raises(ValueError):
            WeightTransaction(
                from_address="addr1",
                subnet_id=1,
                uids=[1, 2, 3],
                weights=[0.5, 0.3, 0.3],  # Sum = 1.1
                version_key=1
            )
    
    def test_proposal_transaction(self):
        """Test proposal transaction creation."""
        tx = ProposalTransaction(
            from_address="addr1",
            title="Test Proposal",
            description="A test proposal for governance",
            proposal_type="parameter_change",
            options=["Yes", "No"],
            duration_blocks=1000
        )
        
        assert tx.transaction_type == TransactionType.PROPOSE
        assert tx.title == "Test Proposal"
        assert len(tx.options) == 2


class TestTransactionBuilder:
    """Test transaction builder."""
    
    def test_builder_transfer(self):
        """Test building transfer transaction."""
        tx = TransactionBuilder() \
            .transfer("addr1", "addr2", 100.0) \
            .with_fee(0.01) \
            .with_memo("Test transfer") \
            .build()
        
        assert isinstance(tx, TransferTransaction)
        assert tx.amount == 100.0
        assert tx.fee == 0.01
        assert tx.memo == "Test transfer"
    
    def test_builder_stake(self):
        """Test building stake transaction."""
        tx = TransactionBuilder() \
            .stake("addr1", "hotkey123", 50.0) \
            .with_nonce(5) \
            .build()
        
        assert isinstance(tx, StakeTransaction)
        assert tx.amount == 50.0
        assert tx.nonce == 5
    
    def test_builder_weights(self):
        """Test building weight transaction."""
        tx = TransactionBuilder() \
            .set_weights(
                "addr1",
                subnet_id=1,
                uids=[1, 2, 3],
                weights=[0.5, 0.3, 0.2],
                version_key=1
            ) \
            .build()
        
        assert isinstance(tx, WeightTransaction)
        assert len(tx.uids) == 3
    
    def test_builder_reset(self):
        """Test builder reset."""
        builder = TransactionBuilder()
        
        tx1 = builder.transfer("addr1", "addr2", 100.0).build()
        assert isinstance(tx1, TransferTransaction)
        
        builder.reset()
        tx2 = builder.stake("addr1", "hotkey", 50.0).build()
        assert isinstance(tx2, StakeTransaction)
    
    def test_builder_without_type(self):
        """Test building without setting type."""
        builder = TransactionBuilder()
        
        with pytest.raises(ValueError):
            builder.build()


class TestTransactionValidator:
    """Test transaction validator."""
    
    def test_validate_valid_transfer(self):
        """Test validating valid transfer."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) == 0
    
    def test_validate_same_address_transfer(self):
        """Test transfer to same address."""
        validator = TransactionValidator(strict=False)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr1234567890",
            amount=100.0
        )
        
        errors = validator.validate(tx)
        assert len(errors) > 0
        assert any("same address" in e.lower() for e in errors)
    
    def test_validate_invalid_weight_sum(self):
        """Test weight validation with invalid sum."""
        # This should fail at Pydantic level during creation
        with pytest.raises(ValueError) as exc_info:
            tx = WeightTransaction(
                from_address="addr1234567890",
                subnet_id=1,
                uids=[1, 2],
                weights=[0.6, 0.6],  # Invalid sum
                version_key=1
            )
        
        # Verify the error message mentions weight sum
        assert "sum" in str(exc_info.value).lower() or "weight" in str(exc_info.value).lower()
    
    def test_validate_strict_mode(self):
        """Test strict validation mode."""
        validator = TransactionValidator(strict=True)
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr1234567890",  # Same as from
            amount=100.0
        )
        
        with pytest.raises(ValidationError):
            validator.validate(tx)
    
    def test_validate_batch(self):
        """Test batch validation."""
        validator = TransactionValidator(strict=False)
        
        transactions = [
            TransferTransaction(
                from_address="addr1234567890",
                to_address="addr0987654321",
                amount=100.0
            ),
            TransferTransaction(
                from_address="addr1",  # Short address
                to_address="addr2",
                amount=50.0
            ),
        ]
        
        results = validator.validate_batch(transactions)
        assert 1 in results  # Second transaction should have errors
    
    def test_duplicate_detection(self):
        """Test duplicate transaction detection."""
        validator = TransactionValidator()
        
        tx = TransferTransaction(
            from_address="addr1234567890",
            to_address="addr0987654321",
            amount=100.0
        )
        
        # First time should not be duplicate
        assert not validator.check_duplicate(tx)
        
        # Second time should be duplicate
        assert validator.check_duplicate(tx)


class TestBatchTransactionBuilder:
    """Test batch transaction builder."""
    
    def test_add_transaction(self):
        """Test adding single transaction."""
        batch = BatchTransactionBuilder()
        
        tx = TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        )
        
        batch.add_transaction(tx)
        assert batch.count() == 1
    
    def test_add_multiple_transactions(self):
        """Test adding multiple transactions."""
        batch = BatchTransactionBuilder()
        
        transactions = [
            TransferTransaction(
                from_address="addr1",
                to_address="addr2",
                amount=100.0
            ),
            StakeTransaction(
                from_address="addr1",
                hotkey="hotkey123",
                amount=50.0
            ),
        ]
        
        batch.add_transactions(transactions)
        assert batch.count() == 2
    
    def test_clear_batch(self):
        """Test clearing batch."""
        batch = BatchTransactionBuilder()
        
        tx = TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        )
        
        batch.add_transaction(tx)
        assert batch.count() == 1
        
        batch.clear()
        assert batch.count() == 0
    
    def test_filter_by_type(self):
        """Test filtering transactions by type."""
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
        batch.add_transaction(TransferTransaction(
            from_address="addr3",
            to_address="addr4",
            amount=200.0
        ))
        
        transfers = batch.get_transactions_by_type(TransactionType.TRANSFER)
        assert len(transfers) == 2
        
        stakes = batch.get_transactions_by_type(TransactionType.STAKE)
        assert len(stakes) == 1
    
    def test_estimate_fees(self):
        """Test fee estimation."""
        batch = BatchTransactionBuilder()
        
        tx1 = TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0,
            fee=0.01
        )
        tx2 = TransferTransaction(
            from_address="addr3",
            to_address="addr4",
            amount=200.0,
            fee=0.02
        )
        
        batch.add_transaction(tx1)
        batch.add_transaction(tx2)
        
        total_fees = batch.estimate_total_fees()
        assert abs(total_fees - 0.03) < 0.001
