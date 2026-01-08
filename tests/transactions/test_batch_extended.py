"""
Extended tests for batch transaction processing.

These tests improve coverage of the batch.py module.
"""

import pytest
import asyncio
from sdk.transactions.types import TransferTransaction, StakeTransaction
from sdk.transactions.batch import BatchTransactionBuilder


class TestBatchBuilderExtended:
    """Extended tests for batch builder."""
    
    def test_add_transactions_method(self):
        """Test add_transactions method with multiple transactions."""
        batch = BatchTransactionBuilder()
        
        tx1 = TransferTransaction(from_address="addr1", to_address="addr2", amount=100.0)
        tx2 = TransferTransaction(from_address="addr3", to_address="addr4", amount=200.0)
        tx3 = TransferTransaction(from_address="addr5", to_address="addr6", amount=300.0)
        
        batch.add_transactions([tx1, tx2, tx3])
        
        assert batch.count() == 3
        assert batch.transactions[0] == tx1
        assert batch.transactions[1] == tx2
        assert batch.transactions[2] == tx3
    
    def test_count_method(self):
        """Test count method."""
        batch = BatchTransactionBuilder()
        
        assert batch.count() == 0
        
        batch.add_transaction(TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        ))
        
        assert batch.count() == 1
        
        batch.add_transaction(TransferTransaction(
            from_address="addr3",
            to_address="addr4",
            amount=200.0
        ))
        
        assert batch.count() == 2
    
    @pytest.mark.asyncio
    async def test_submit_all_async_empty(self):
        """Test async submission with empty batch."""
        batch = BatchTransactionBuilder()
        
        async def mock_submit(tx):
            return "success"
        
        results = await batch.submit_all_async(mock_submit)
        
        assert results == []
    
    @pytest.mark.asyncio
    async def test_submit_all_async_with_progress(self):
        """Test async submission with progress callback."""
        batch = BatchTransactionBuilder(max_concurrent=2)
        
        # Add 5 transactions
        for i in range(5):
            batch.add_transaction(TransferTransaction(
                from_address=f"addr{i}",
                to_address=f"addr{i+1}",
                amount=float(i + 1) * 100
            ))
        
        # Track progress
        progress_calls = []
        
        def on_progress(completed, total):
            progress_calls.append((completed, total))
        
        # Mock submit function
        async def mock_submit(tx):
            await asyncio.sleep(0.01)  # Simulate network delay
            return f"tx_hash_{tx.from_address}"
        
        results = await batch.submit_all_async(mock_submit, on_progress=on_progress)
        
        # Verify results
        assert len(results) == 5
        assert all("tx_hash_" in r for r in results)
        
        # Verify progress was tracked
        assert len(progress_calls) > 0
        assert progress_calls[-1] == (5, 5)  # Final call should show all complete
    
    @pytest.mark.asyncio
    async def test_submit_all_async_with_errors(self):
        """Test async submission with some transactions failing."""
        batch = BatchTransactionBuilder()
        
        # Add 3 transactions
        for i in range(3):
            batch.add_transaction(TransferTransaction(
                from_address=f"addr{i}",
                to_address=f"addr{i+1}",
                amount=float(i + 1) * 100
            ))
        
        # Mock submit function that fails on second transaction
        async def mock_submit(tx):
            if tx.from_address == "addr1":
                raise ValueError("Simulated error")
            return f"success_{tx.from_address}"
        
        results = await batch.submit_all_async(mock_submit)
        
        # Verify results
        assert len(results) == 3
        assert results[0] == "success_addr0"
        assert isinstance(results[1], Exception)
        assert results[2] == "success_addr2"
    
    def test_submit_all_sync_empty(self):
        """Test sync submission with empty batch."""
        batch = BatchTransactionBuilder()
        
        def mock_submit(tx):
            return "success"
        
        results = batch.submit_all_sync(mock_submit)
        
        assert results == []
    
    def test_submit_all_sync_with_progress(self):
        """Test sync submission with progress callback."""
        batch = BatchTransactionBuilder(max_concurrent=2)
        
        # Add 3 transactions
        for i in range(3):
            batch.add_transaction(TransferTransaction(
                from_address=f"addr{i}",
                to_address=f"addr{i+1}",
                amount=float(i + 1) * 100
            ))
        
        # Track progress
        progress_calls = []
        
        def on_progress(completed, total):
            progress_calls.append((completed, total))
        
        # Mock submit function
        def mock_submit(tx):
            return f"tx_hash_{tx.from_address}"
        
        results = batch.submit_all_sync(mock_submit, on_progress=on_progress)
        
        # Verify results
        assert len(results) == 3
        assert all("tx_hash_" in r for r in results)
        
        # Verify progress was tracked
        assert len(progress_calls) == 3
        assert (3, 3) in progress_calls  # Final call should show all complete
    
    def test_submit_all_sync_with_errors(self):
        """Test sync submission with some transactions failing."""
        batch = BatchTransactionBuilder()
        
        # Add 3 transactions
        for i in range(3):
            batch.add_transaction(TransferTransaction(
                from_address=f"addr{i}",
                to_address=f"addr{i+1}",
                amount=float(i + 1) * 100
            ))
        
        # Mock submit function that fails on second transaction
        def mock_submit(tx):
            if tx.from_address == "addr1":
                raise ValueError("Simulated error")
            return f"success_{tx.from_address}"
        
        results = batch.submit_all_sync(mock_submit)
        
        # Verify results
        assert len(results) == 3
        
        # Count successes and failures
        successes = sum(1 for r in results if not isinstance(r, Exception))
        failures = sum(1 for r in results if isinstance(r, Exception))
        
        assert successes == 2
        assert failures == 1
    
    def test_validate_all(self):
        """Test validate_all method."""
        batch = BatchTransactionBuilder()
        
        # Add valid transactions
        batch.add_transaction(TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        ))
        batch.add_transaction(StakeTransaction(
            from_address="addr3",
            hotkey="hotkey123",
            amount=50.0,
            subnet_id=1
        ))
        
        # Validate all - this should work as long as transactions are valid
        errors = batch.validate_all()
        
        # The method returns errors, check it's callable
        assert isinstance(errors, list)
    
    def test_get_transactions_by_type(self):
        """Test get_transactions_by_type method."""
        from sdk.transactions.types import TransactionType
        
        batch = BatchTransactionBuilder()
        
        # Add different transaction types
        batch.add_transaction(TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        ))
        batch.add_transaction(StakeTransaction(
            from_address="addr3",
            hotkey="hotkey123",
            amount=50.0,
            subnet_id=1
        ))
        batch.add_transaction(TransferTransaction(
            from_address="addr4",
            to_address="addr5",
            amount=200.0
        ))
        
        # Get transfers only
        transfers = batch.get_transactions_by_type(TransactionType.TRANSFER)
        
        assert len(transfers) == 2
        assert all(tx.transaction_type == TransactionType.TRANSFER for tx in transfers)
        
        # Get stakes only
        stakes = batch.get_transactions_by_type(TransactionType.STAKE)
        
        assert len(stakes) == 1
        assert stakes[0].transaction_type == TransactionType.STAKE
    
    def test_estimate_total_fees(self):
        """Test estimate_total_fees method."""
        batch = BatchTransactionBuilder()
        
        # Add transactions with fees
        tx1 = TransferTransaction(
            from_address="addr1",
            to_address="addr2",
            amount=100.0
        )
        tx1.fee = 0.01
        
        tx2 = TransferTransaction(
            from_address="addr3",
            to_address="addr4",
            amount=200.0
        )
        tx2.fee = 0.02
        
        tx3 = TransferTransaction(
            from_address="addr5",
            to_address="addr6",
            amount=300.0
        )
        tx3.fee = 0.03
        
        batch.add_transactions([tx1, tx2, tx3])
        
        total_fees = batch.estimate_total_fees()
        
        assert total_fees == pytest.approx(0.06, rel=1e-9)
    
    def test_chaining(self):
        """Test method chaining."""
        batch = BatchTransactionBuilder()
        
        tx1 = TransferTransaction(from_address="addr1", to_address="addr2", amount=100.0)
        tx2 = TransferTransaction(from_address="addr3", to_address="addr4", amount=200.0)
        
        # Chain methods
        result = batch.add_transaction(tx1).add_transaction(tx2).clear().add_transaction(tx1)
        
        # Verify chaining worked
        assert result is batch
        assert batch.count() == 1
        assert batch.transactions[0] == tx1
