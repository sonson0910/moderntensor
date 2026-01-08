"""
Extended tests for TransactionBuilder to improve coverage.

These tests target the missing coverage in builder.py:
- unstake() method
- register() method  
- propose() method
- vote() method
- delegate() method
- serve_axon() method
- swap_hotkey() method
- build() error handling
"""

import pytest
from sdk.transactions.builder import TransactionBuilder
from sdk.transactions.types import (
    UnstakeTransaction,
    RegisterTransaction,
    ProposalTransaction,
    VoteTransaction,
    DelegateTransaction,
    ServeAxonTransaction,
    SwapHotkeyTransaction,
)


class TestBuilderExtendedMethods:
    """Test extended builder methods for full coverage."""
    
    def test_unstake_transaction(self):
        """Test building unstake transaction."""
        tx = TransactionBuilder() \
            .unstake(
                from_address="addr1234567890",
                hotkey="hotkey123456",
                amount=50.0,
                subnet_id=1
            ) \
            .with_fee(0.01) \
            .build()
        
        assert isinstance(tx, UnstakeTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.hotkey == "hotkey123456"
        assert tx.amount == 50.0
        assert tx.subnet_id == 1
        assert tx.fee == 0.01
    
    def test_unstake_without_subnet(self):
        """Test unstake without subnet_id."""
        tx = TransactionBuilder() \
            .unstake(
                from_address="addr1234567890",
                hotkey="hotkey123456",
                amount=25.0
            ) \
            .build()
        
        assert isinstance(tx, UnstakeTransaction)
        assert tx.subnet_id is None
    
    def test_register_transaction(self):
        """Test building register transaction."""
        metadata = {"name": "test_miner", "version": "1.0"}
        
        tx = TransactionBuilder() \
            .register(
                from_address="addr1234567890",
                subnet_id=1,
                hotkey="hotkey123456",
                metadata=metadata
            ) \
            .with_nonce(10) \
            .build()
        
        assert isinstance(tx, RegisterTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.subnet_id == 1
        assert tx.hotkey == "hotkey123456"
        assert tx.metadata == metadata
        assert tx.nonce == 10
    
    def test_register_without_metadata(self):
        """Test register without metadata."""
        tx = TransactionBuilder() \
            .register(
                from_address="addr1234567890",
                subnet_id=2,
                hotkey="hotkey789"
            ) \
            .build()
        
        assert isinstance(tx, RegisterTransaction)
        assert tx.metadata is None
    
    def test_propose_transaction(self):
        """Test building proposal transaction."""
        tx = TransactionBuilder() \
            .propose(
                from_address="addr1234567890",
                title="Network Parameter Update",
                description="Update emission rate to 5%",
                proposal_type="parameter_change",
                options=["Yes", "No", "Abstain"],
                duration_blocks=10000
            ) \
            .with_memo("Important proposal") \
            .build()
        
        assert isinstance(tx, ProposalTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.title == "Network Parameter Update"
        assert tx.description == "Update emission rate to 5%"
        assert tx.proposal_type == "parameter_change"
        assert len(tx.options) == 3
        assert tx.duration_blocks == 10000
        assert tx.memo == "Important proposal"
    
    def test_vote_transaction(self):
        """Test building vote transaction."""
        tx = TransactionBuilder() \
            .vote(
                from_address="addr1234567890",
                proposal_id=42,
                option="Yes",
                voting_power=100.0
            ) \
            .build()
        
        assert isinstance(tx, VoteTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.proposal_id == 42
        assert tx.option == "Yes"
        assert tx.voting_power == 100.0
    
    def test_vote_without_voting_power(self):
        """Test vote without explicit voting power."""
        tx = TransactionBuilder() \
            .vote(
                from_address="addr1234567890",
                proposal_id=100,
                option="No"
            ) \
            .build()
        
        assert isinstance(tx, VoteTransaction)
        assert tx.voting_power is None
    
    def test_delegate_transaction(self):
        """Test building delegate transaction."""
        tx = TransactionBuilder() \
            .delegate(
                from_address="addr1234567890",
                validator_hotkey="validator_hotkey123",
                amount=500.0
            ) \
            .with_fee(0.05) \
            .with_nonce(20) \
            .build()
        
        assert isinstance(tx, DelegateTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.validator_hotkey == "validator_hotkey123"
        assert tx.amount == 500.0
        assert tx.fee == 0.05
        assert tx.nonce == 20
    
    def test_serve_axon_transaction(self):
        """Test building serve axon transaction."""
        tx = TransactionBuilder() \
            .serve_axon(
                from_address="addr1234567890",
                subnet_id=1,
                ip="192.168.1.100",
                port=8080,
                protocol="https",
                version=2
            ) \
            .build()
        
        assert isinstance(tx, ServeAxonTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.subnet_id == 1
        assert tx.ip == "192.168.1.100"
        assert tx.port == 8080
        assert tx.protocol == "https"
        assert tx.version == 2
    
    def test_serve_axon_default_protocol(self):
        """Test serve axon with default protocol."""
        tx = TransactionBuilder() \
            .serve_axon(
                from_address="addr1234567890",
                subnet_id=2,
                ip="10.0.0.1",
                port=9090
            ) \
            .build()
        
        assert isinstance(tx, ServeAxonTransaction)
        assert tx.protocol == "http"
        assert tx.version == 1
    
    def test_swap_hotkey_transaction(self):
        """Test building swap hotkey transaction."""
        tx = TransactionBuilder() \
            .swap_hotkey(
                from_address="addr1234567890",
                subnet_id=1,
                old_hotkey="old_hotkey123",
                new_hotkey="new_hotkey456"
            ) \
            .with_memo("Security update") \
            .build()
        
        assert isinstance(tx, SwapHotkeyTransaction)
        assert tx.from_address == "addr1234567890"
        assert tx.subnet_id == 1
        assert tx.old_hotkey == "old_hotkey123"
        assert tx.new_hotkey == "new_hotkey456"
        assert tx.memo == "Security update"


class TestBuilderErrorHandling:
    """Test builder error handling and edge cases."""
    
    def test_build_without_transaction_type(self):
        """Test building without setting transaction type."""
        builder = TransactionBuilder()
        
        with pytest.raises(ValueError) as exc_info:
            builder.build()
        
        assert "transaction type not set" in str(exc_info.value).lower()
    
    def test_build_with_invalid_data(self):
        """Test building with invalid transaction data."""
        builder = TransactionBuilder()
        
        # Create invalid transfer (negative amount will fail Pydantic validation)
        builder.transfer("addr1", "addr2", -100.0)
        
        with pytest.raises(Exception):
            builder.build()
    
    def test_reset_clears_all_data(self):
        """Test that reset clears transaction type and data."""
        builder = TransactionBuilder()
        
        # Build a transaction
        builder.transfer("addr1", "addr2", 100.0).with_fee(0.01)
        
        # Reset
        builder.reset()
        
        # Should not be able to build without setting type again
        with pytest.raises(ValueError):
            builder.build()


class TestBuilderChaining:
    """Test builder method chaining with different transaction types."""
    
    def test_unstake_with_all_options(self):
        """Test unstake with all chained options."""
        tx = TransactionBuilder() \
            .unstake("addr1234567890", "hotkey123", 100.0, subnet_id=1) \
            .with_nonce(5) \
            .with_fee(0.02) \
            .with_memo("Unstaking tokens") \
            .build()
        
        assert tx.nonce == 5
        assert tx.fee == 0.02
        assert tx.memo == "Unstaking tokens"
    
    def test_register_with_all_options(self):
        """Test register with all chained options."""
        tx = TransactionBuilder() \
            .register("addr1234567890", 1, "hotkey123", {"type": "miner"}) \
            .with_nonce(1) \
            .with_fee(0.1) \
            .with_memo("New registration") \
            .build()
        
        assert tx.nonce == 1
        assert tx.fee == 0.1
        assert tx.memo == "New registration"
    
    def test_propose_with_all_options(self):
        """Test propose with all chained options."""
        tx = TransactionBuilder() \
            .propose(
                "addr1234567890",
                "Test Proposal",
                "Description",
                "parameter_change",
                ["Yes", "No"],
                1000
            ) \
            .with_nonce(10) \
            .with_fee(1.0) \
            .build()
        
        assert tx.nonce == 10
        assert tx.fee == 1.0
    
    def test_multiple_transactions_with_reset(self):
        """Test building multiple different transactions with reset."""
        builder = TransactionBuilder()
        
        # Build delegate transaction
        tx1 = builder.delegate("addr1", "validator1", 100.0).build()
        assert isinstance(tx1, DelegateTransaction)
        
        # Reset and build serve_axon transaction
        builder.reset()
        tx2 = builder.serve_axon("addr1", 1, "127.0.0.1", 8080).build()
        assert isinstance(tx2, ServeAxonTransaction)
        
        # Reset and build swap_hotkey transaction  
        builder.reset()
        tx3 = builder.swap_hotkey("addr1", 1, "old", "new").build()
        assert isinstance(tx3, SwapHotkeyTransaction)
    
    def test_overwrite_options(self):
        """Test that later chained options overwrite earlier ones."""
        tx = TransactionBuilder() \
            .transfer("addr1", "addr2", 100.0) \
            .with_fee(0.01) \
            .with_fee(0.02) \
            .with_nonce(5) \
            .with_nonce(10) \
            .build()
        
        # Should use the last set values
        assert tx.fee == 0.02
        assert tx.nonce == 10
