"""
Extended tests for transaction monitoring.

These tests improve coverage of the monitor.py module.
"""

import pytest
import asyncio
from datetime import datetime, timedelta
from sdk.transactions.monitor import (
    TransactionMonitor,
    TransactionStatus,
    TransactionRecord
)


class TestTransactionMonitor:
    """Tests for transaction monitor."""
    
    def test_track_transaction(self):
        """Test tracking a new transaction."""
        monitor = TransactionMonitor(required_confirmations=3)
        
        record = monitor.track("tx_hash_123", metadata={"type": "transfer"})
        
        assert record.tx_hash == "tx_hash_123"
        assert record.status == TransactionStatus.PENDING
        assert record.submitted_at is not None
        assert record.metadata["type"] == "transfer"
        assert record.confirmations == 0
    
    def test_get_status(self):
        """Test getting transaction status."""
        monitor = TransactionMonitor()
        
        # Track a transaction
        monitor.track("tx_hash_123")
        
        # Get status
        status = monitor.get_status("tx_hash_123")
        assert status == TransactionStatus.PENDING
        
        # Non-existent transaction
        status = monitor.get_status("non_existent")
        assert status is None
    
    def test_update_status(self):
        """Test updating transaction status."""
        monitor = TransactionMonitor()
        
        # Track and update
        monitor.track("tx_hash_123")
        monitor.update_status("tx_hash_123", TransactionStatus.SUBMITTED)
        
        assert monitor.get_status("tx_hash_123") == TransactionStatus.SUBMITTED
        
        # Update to confirmed
        monitor.update_status("tx_hash_123", TransactionStatus.CONFIRMED, block_number=12345)
        
        record = monitor.get_record("tx_hash_123")
        assert record.status == TransactionStatus.CONFIRMED
        assert record.block_number == 12345
        assert record.confirmed_at is not None
    
    def test_update_status_with_error(self):
        """Test updating status with error message."""
        monitor = TransactionMonitor()
        
        monitor.track("tx_hash_123")
        monitor.update_status("tx_hash_123", TransactionStatus.FAILED, error="Insufficient funds")
        
        record = monitor.get_record("tx_hash_123")
        assert record.status == TransactionStatus.FAILED
        assert record.error == "Insufficient funds"
    
    def test_update_confirmations(self):
        """Test updating confirmation count."""
        monitor = TransactionMonitor(required_confirmations=3)
        
        monitor.track("tx_hash_123")
        
        # Update confirmations
        monitor.update_confirmations("tx_hash_123", 1)
        record = monitor.get_record("tx_hash_123")
        assert record.confirmations == 1
        # Status remains PENDING until required confirmations reached
        
        # Reach required confirmations
        monitor.update_confirmations("tx_hash_123", 3)
        record = monitor.get_record("tx_hash_123")
        assert record.confirmations == 3
        assert record.status == TransactionStatus.CONFIRMED
    
    def test_update_confirmations_non_existent(self):
        """Test updating confirmations for non-existent transaction."""
        monitor = TransactionMonitor()
        
        # Should not raise error
        monitor.update_confirmations("non_existent", 1)
        
        # Transaction should not exist
        assert monitor.get_record("non_existent") is None
    
    def test_update_status_non_existent(self):
        """Test updating status for non-existent transaction."""
        monitor = TransactionMonitor()
        
        # Should not raise error
        monitor.update_status("non_existent", TransactionStatus.CONFIRMED)
        
        # Transaction should not exist
        assert monitor.get_record("non_existent") is None
    
    def test_get_record(self):
        """Test getting full transaction record."""
        monitor = TransactionMonitor()
        
        monitor.track("tx_hash_123", metadata={"amount": 100})
        record = monitor.get_record("tx_hash_123")
        
        assert record is not None
        assert record.tx_hash == "tx_hash_123"
        assert record.metadata["amount"] == 100
        
        # Non-existent transaction
        record = monitor.get_record("non_existent")
        assert record is None
    
    @pytest.mark.asyncio
    async def test_wait_for_confirmation_success(self):
        """Test waiting for transaction confirmation."""
        monitor = TransactionMonitor(required_confirmations=2)
        
        # Track transaction
        monitor.track("tx_hash_123")
        
        # Simulate confirmation in background
        async def confirm_later():
            await asyncio.sleep(0.1)
            monitor.update_confirmations("tx_hash_123", 1)
            await asyncio.sleep(0.1)
            monitor.update_confirmations("tx_hash_123", 2)
        
        # Start confirmation task
        asyncio.create_task(confirm_later())
        
        # Wait for confirmation
        status = await monitor.wait_for_confirmation(
            "tx_hash_123",
            timeout=5.0,
            poll_interval=0.05
        )
        
        assert status == TransactionStatus.CONFIRMED
    
    @pytest.mark.asyncio
    async def test_wait_for_confirmation_timeout(self):
        """Test timeout while waiting for confirmation."""
        monitor = TransactionMonitor(required_confirmations=5)
        
        # Track transaction but never confirm it
        monitor.track("tx_hash_123")
        
        # Wait with short timeout
        status = await monitor.wait_for_confirmation(
            "tx_hash_123",
            timeout=0.2,
            poll_interval=0.05
        )
        
        # Should timeout
        assert status == TransactionStatus.TIMEOUT
    
    @pytest.mark.asyncio
    async def test_wait_for_confirmation_failed(self):
        """Test waiting for a failed transaction."""
        monitor = TransactionMonitor()
        
        # Track transaction
        monitor.track("tx_hash_123")
        
        # Simulate failure in background
        async def fail_later():
            await asyncio.sleep(0.1)
            monitor.update_status("tx_hash_123", TransactionStatus.FAILED, error="Network error")
        
        asyncio.create_task(fail_later())
        
        # Wait for confirmation
        status = await monitor.wait_for_confirmation(
            "tx_hash_123",
            timeout=5.0,
            poll_interval=0.05
        )
        
        # Should detect failure
        assert status == TransactionStatus.FAILED
    
    @pytest.mark.asyncio
    async def test_wait_for_confirmation_not_tracked(self):
        """Test waiting for non-tracked transaction."""
        monitor = TransactionMonitor()
        
        # Don't track the transaction
        status = await monitor.wait_for_confirmation(
            "tx_hash_123",
            timeout=1.0,
            poll_interval=0.1
        )
        
        # Should return FAILED
        assert status == TransactionStatus.FAILED
    
    def test_statistics(self):
        """Test getting transaction statistics."""
        monitor = TransactionMonitor()
        
        # Track various transactions
        monitor.track("tx_1")
        monitor.track("tx_2")
        monitor.update_status("tx_2", TransactionStatus.CONFIRMED)
        monitor.track("tx_3")
        monitor.update_status("tx_3", TransactionStatus.FAILED, error="Error")
        monitor.track("tx_4")
        monitor.update_status("tx_4", TransactionStatus.CONFIRMED)
        
        stats = monitor.get_statistics()
        
        assert stats["total"] == 4
        assert "by_status" in stats
        assert "average_confirmation_time" in stats
        assert "confirmed_count" in stats
    
    def test_statistics_empty(self):
        """Test statistics with no transactions."""
        monitor = TransactionMonitor()
        
        stats = monitor.get_statistics()
        
        assert stats["total"] == 0
        assert stats["by_status"] == {}
        assert stats["average_confirmation_time"] is None
    
    def test_transaction_record_duration(self):
        """Test transaction duration calculation."""
        record = TransactionRecord(tx_hash="tx_123")
        
        # Set times
        record.submitted_at = datetime.now()
        record.confirmed_at = record.submitted_at + timedelta(seconds=5)
        
        duration = record.duration_seconds()
        
        assert duration is not None
        assert 4.9 <= duration <= 5.1  # Allow small variance
    
    def test_transaction_record_no_duration(self):
        """Test duration when not confirmed."""
        record = TransactionRecord(tx_hash="tx_123")
        record.submitted_at = datetime.now()
        # No confirmed_at set
        
        duration = record.duration_seconds()
        
        assert duration is None
    
    def test_transaction_record_defaults(self):
        """Test transaction record default values."""
        record = TransactionRecord(tx_hash="tx_123")
        
        assert record.status == TransactionStatus.PENDING
        assert record.submitted_at is None
        assert record.confirmed_at is None
        assert record.block_number is None
        assert record.error is None
        assert record.confirmations == 0
        assert record.metadata == {}
    
    def test_multiple_transactions(self):
        """Test monitoring multiple transactions."""
        monitor = TransactionMonitor()
        
        # Track multiple transactions
        for i in range(10):
            monitor.track(f"tx_{i}")
        
        # Update some
        monitor.update_confirmations("tx_0", 1)
        monitor.update_status("tx_1", TransactionStatus.CONFIRMED)
        monitor.update_status("tx_2", TransactionStatus.FAILED, error="Error")
        
        # Verify all tracked
        assert monitor.get_record("tx_0") is not None
        assert monitor.get_record("tx_9") is not None
        
        # Verify updates
        assert monitor.get_status("tx_1") == TransactionStatus.CONFIRMED
        assert monitor.get_status("tx_2") == TransactionStatus.FAILED


class TestTransactionStatus:
    """Tests for transaction status enum."""
    
    def test_status_values(self):
        """Test all status values are defined."""
        assert TransactionStatus.PENDING == "pending"
        assert TransactionStatus.SUBMITTED == "submitted"
        assert TransactionStatus.CONFIRMED == "confirmed"
        assert TransactionStatus.FAILED == "failed"
        assert TransactionStatus.TIMEOUT == "timeout"
    
    def test_status_comparison(self):
        """Test status comparison."""
        status1 = TransactionStatus.PENDING
        status2 = TransactionStatus.PENDING
        status3 = TransactionStatus.CONFIRMED
        
        assert status1 == status2
        assert status1 != status3
