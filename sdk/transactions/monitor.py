"""
Transaction Monitor

Monitors transaction status and provides real-time updates.
"""

import logging
import time
import asyncio
from typing import Optional, Dict, Any, Callable, List
from enum import Enum
from dataclasses import dataclass, field
from datetime import datetime

logger = logging.getLogger(__name__)


class TransactionStatus(str, Enum):
    """Transaction status states."""
    
    PENDING = "pending"
    SUBMITTED = "submitted"
    CONFIRMED = "confirmed"
    FAILED = "failed"
    TIMEOUT = "timeout"


@dataclass
class TransactionRecord:
    """Record of a monitored transaction."""
    
    tx_hash: str
    status: TransactionStatus = TransactionStatus.PENDING
    submitted_at: Optional[datetime] = None
    confirmed_at: Optional[datetime] = None
    block_number: Optional[int] = None
    error: Optional[str] = None
    confirmations: int = 0
    metadata: Dict[str, Any] = field(default_factory=dict)
    
    def duration_seconds(self) -> Optional[float]:
        """Get duration from submission to confirmation."""
        if self.submitted_at and self.confirmed_at:
            return (self.confirmed_at - self.submitted_at).total_seconds()
        return None


class TransactionMonitor:
    """
    Monitor transaction status and lifecycle.
    
    Tracks:
    - Transaction submission
    - Confirmation status
    - Block inclusion
    - Failures and retries
    
    Example:
        ```python
        monitor = TransactionMonitor()
        
        # Submit transaction
        tx_hash = await client.submit_transaction(tx)
        monitor.track(tx_hash, metadata={"type": "transfer"})
        
        # Wait for confirmation
        status = await monitor.wait_for_confirmation(tx_hash, timeout=60)
        ```
    """
    
    def __init__(self, required_confirmations: int = 1):
        """
        Initialize monitor.
        
        Args:
            required_confirmations: Number of confirmations required
        """
        self.required_confirmations = required_confirmations
        self._transactions: Dict[str, TransactionRecord] = {}
        
    def track(
        self,
        tx_hash: str,
        metadata: Optional[Dict[str, Any]] = None
    ) -> TransactionRecord:
        """
        Start tracking a transaction.
        
        Args:
            tx_hash: Transaction hash
            metadata: Optional metadata to store
            
        Returns:
            Transaction record
        """
        record = TransactionRecord(
            tx_hash=tx_hash,
            submitted_at=datetime.now(),
            metadata=metadata or {}
        )
        self._transactions[tx_hash] = record
        
        logger.info(f"Started tracking transaction: {tx_hash}")
        return record
    
    def update_status(
        self,
        tx_hash: str,
        status: TransactionStatus,
        block_number: Optional[int] = None,
        error: Optional[str] = None
    ):
        """
        Update transaction status.
        
        Args:
            tx_hash: Transaction hash
            status: New status
            block_number: Block number if confirmed
            error: Error message if failed
        """
        if tx_hash not in self._transactions:
            logger.warning(f"Transaction not tracked: {tx_hash}")
            return
        
        record = self._transactions[tx_hash]
        record.status = status
        
        if block_number is not None:
            record.block_number = block_number
        
        if error is not None:
            record.error = error
        
        if status == TransactionStatus.CONFIRMED:
            record.confirmed_at = datetime.now()
        
        logger.debug(f"Updated transaction {tx_hash}: {status}")
    
    def update_confirmations(self, tx_hash: str, confirmations: int):
        """
        Update confirmation count.
        
        Args:
            tx_hash: Transaction hash
            confirmations: Number of confirmations
        """
        if tx_hash not in self._transactions:
            return
        
        record = self._transactions[tx_hash]
        record.confirmations = confirmations
        
        if confirmations >= self.required_confirmations:
            record.status = TransactionStatus.CONFIRMED
            if not record.confirmed_at:
                record.confirmed_at = datetime.now()
    
    def get_status(self, tx_hash: str) -> Optional[TransactionStatus]:
        """
        Get transaction status.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction status or None if not tracked
        """
        if tx_hash not in self._transactions:
            return None
        
        return self._transactions[tx_hash].status
    
    def get_record(self, tx_hash: str) -> Optional[TransactionRecord]:
        """
        Get full transaction record.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction record or None if not tracked
        """
        return self._transactions.get(tx_hash)
    
    async def wait_for_confirmation(
        self,
        tx_hash: str,
        timeout: float = 60.0,
        poll_interval: float = 2.0,
        check_fn: Optional[Callable[[str], Any]] = None
    ) -> TransactionStatus:
        """
        Wait for transaction confirmation.
        
        Args:
            tx_hash: Transaction hash
            timeout: Maximum wait time in seconds
            poll_interval: Polling interval in seconds
            check_fn: Optional async function to check transaction status
            
        Returns:
            Final transaction status
        """
        if tx_hash not in self._transactions:
            logger.error(f"Transaction not tracked: {tx_hash}")
            return TransactionStatus.FAILED
        
        start_time = time.time()
        
        logger.info(f"Waiting for confirmation: {tx_hash}")
        
        while time.time() - start_time < timeout:
            record = self._transactions[tx_hash]
            
            # Check if already confirmed or failed
            if record.status == TransactionStatus.CONFIRMED:
                logger.info(f"Transaction confirmed: {tx_hash}")
                return TransactionStatus.CONFIRMED
            
            if record.status == TransactionStatus.FAILED:
                logger.error(f"Transaction failed: {tx_hash}")
                return TransactionStatus.FAILED
            
            # Poll status if check function provided
            if check_fn:
                try:
                    result = await check_fn(tx_hash)
                    if result:
                        # Update based on result
                        # Implementation depends on what check_fn returns
                        pass
                except Exception as e:
                    logger.warning(f"Error checking transaction status: {e}")
            
            # Wait before next poll
            await asyncio.sleep(poll_interval)
        
        # Timeout
        logger.warning(f"Transaction confirmation timeout: {tx_hash}")
        self.update_status(tx_hash, TransactionStatus.TIMEOUT)
        return TransactionStatus.TIMEOUT
    
    def get_statistics(self) -> Dict[str, Any]:
        """
        Get monitoring statistics.
        
        Returns:
            Statistics dictionary
        """
        total = len(self._transactions)
        if total == 0:
            return {
                "total": 0,
                "by_status": {},
                "average_confirmation_time": None
            }
        
        # Count by status
        by_status = {}
        for record in self._transactions.values():
            status = record.status.value
            by_status[status] = by_status.get(status, 0) + 1
        
        # Calculate average confirmation time
        confirmed = [
            r for r in self._transactions.values()
            if r.status == TransactionStatus.CONFIRMED and r.duration_seconds()
        ]
        
        avg_time = None
        if confirmed:
            avg_time = sum(r.duration_seconds() for r in confirmed) / len(confirmed)
        
        return {
            "total": total,
            "by_status": by_status,
            "average_confirmation_time": avg_time,
            "confirmed_count": len(confirmed)
        }
    
    def get_pending_transactions(self) -> List[TransactionRecord]:
        """Get all pending transactions."""
        return [
            r for r in self._transactions.values()
            if r.status in (TransactionStatus.PENDING, TransactionStatus.SUBMITTED)
        ]
    
    def clear_completed(self):
        """Remove completed transactions from monitoring."""
        to_remove = [
            tx_hash for tx_hash, record in self._transactions.items()
            if record.status in (TransactionStatus.CONFIRMED, TransactionStatus.FAILED)
        ]
        
        for tx_hash in to_remove:
            del self._transactions[tx_hash]
        
        logger.info(f"Cleared {len(to_remove)} completed transactions")
    
    def clear_all(self):
        """Clear all transactions."""
        count = len(self._transactions)
        self._transactions.clear()
        logger.info(f"Cleared all {count} transactions")
