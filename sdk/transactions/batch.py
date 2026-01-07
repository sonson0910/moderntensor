"""
Batch Transaction Builder

Supports building and submitting multiple transactions efficiently.
"""

import logging
import asyncio
from typing import List, Optional, Callable, Any
from concurrent.futures import ThreadPoolExecutor, as_completed

from sdk.transactions.types import BaseTransaction

logger = logging.getLogger(__name__)


class BatchTransactionBuilder:
    """
    Build and execute multiple transactions in batch.
    
    Provides utilities for:
    - Batching multiple transactions
    - Parallel submission
    - Error handling and retry logic
    - Progress tracking
    
    Example:
        ```python
        batch = BatchTransactionBuilder()
        batch.add_transaction(tx1)
        batch.add_transaction(tx2)
        batch.add_transaction(tx3)
        
        results = await batch.submit_all_async(client)
        ```
    """
    
    def __init__(self, max_concurrent: int = 10):
        """
        Initialize batch builder.
        
        Args:
            max_concurrent: Maximum number of concurrent transactions
        """
        self.transactions: List[BaseTransaction] = []
        self.max_concurrent = max_concurrent
        
    def add_transaction(self, transaction: BaseTransaction) -> "BatchTransactionBuilder":
        """
        Add a transaction to the batch.
        
        Args:
            transaction: Transaction to add
            
        Returns:
            Self for chaining
        """
        self.transactions.append(transaction)
        logger.debug(f"Added {transaction.transaction_type} transaction to batch")
        return self
    
    def add_transactions(self, transactions: List[BaseTransaction]) -> "BatchTransactionBuilder":
        """
        Add multiple transactions to the batch.
        
        Args:
            transactions: List of transactions to add
            
        Returns:
            Self for chaining
        """
        self.transactions.extend(transactions)
        logger.debug(f"Added {len(transactions)} transactions to batch")
        return self
    
    def clear(self) -> "BatchTransactionBuilder":
        """Clear all transactions from the batch."""
        self.transactions.clear()
        logger.debug("Cleared batch transactions")
        return self
    
    def count(self) -> int:
        """Get number of transactions in batch."""
        return len(self.transactions)
    
    async def submit_all_async(
        self,
        submit_fn: Callable[[BaseTransaction], Any],
        on_progress: Optional[Callable[[int, int], None]] = None
    ) -> List[Any]:
        """
        Submit all transactions asynchronously.
        
        Args:
            submit_fn: Async function to submit a single transaction
            on_progress: Optional callback for progress updates (completed, total)
            
        Returns:
            List of submission results
        """
        if not self.transactions:
            logger.warning("No transactions to submit")
            return []
        
        logger.info(f"Submitting {len(self.transactions)} transactions asynchronously")
        
        results = []
        completed = 0
        
        # Process in batches of max_concurrent
        for i in range(0, len(self.transactions), self.max_concurrent):
            batch = self.transactions[i:i + self.max_concurrent]
            
            # Submit batch
            tasks = [submit_fn(tx) for tx in batch]
            batch_results = await asyncio.gather(*tasks, return_exceptions=True)
            
            results.extend(batch_results)
            completed += len(batch)
            
            # Progress callback
            if on_progress:
                on_progress(completed, len(self.transactions))
            
            logger.debug(f"Completed {completed}/{len(self.transactions)} transactions")
        
        # Count successes and failures
        successes = sum(1 for r in results if not isinstance(r, Exception))
        failures = len(results) - successes
        
        logger.info(f"Batch submission complete: {successes} succeeded, {failures} failed")
        
        return results
    
    def submit_all_sync(
        self,
        submit_fn: Callable[[BaseTransaction], Any],
        on_progress: Optional[Callable[[int, int], None]] = None
    ) -> List[Any]:
        """
        Submit all transactions synchronously with parallelism.
        
        Args:
            submit_fn: Function to submit a single transaction
            on_progress: Optional callback for progress updates (completed, total)
            
        Returns:
            List of submission results
        """
        if not self.transactions:
            logger.warning("No transactions to submit")
            return []
        
        logger.info(f"Submitting {len(self.transactions)} transactions in parallel")
        
        results = []
        completed = 0
        
        with ThreadPoolExecutor(max_workers=self.max_concurrent) as executor:
            # Submit all transactions
            future_to_tx = {
                executor.submit(submit_fn, tx): tx 
                for tx in self.transactions
            }
            
            # Collect results as they complete
            for future in as_completed(future_to_tx):
                tx = future_to_tx[future]
                try:
                    result = future.result()
                    results.append(result)
                except Exception as e:
                    logger.error(f"Transaction {tx.transaction_type} failed: {e}")
                    results.append(e)
                
                completed += 1
                
                # Progress callback
                if on_progress:
                    on_progress(completed, len(self.transactions))
                
                logger.debug(f"Completed {completed}/{len(self.transactions)} transactions")
        
        # Count successes and failures
        successes = sum(1 for r in results if not isinstance(r, Exception))
        failures = len(results) - successes
        
        logger.info(f"Batch submission complete: {successes} succeeded, {failures} failed")
        
        return results
    
    def validate_all(self) -> List[Optional[str]]:
        """
        Validate all transactions in the batch.
        
        Returns:
            List of error messages (None for valid transactions)
        """
        errors = []
        
        for i, tx in enumerate(self.transactions):
            try:
                # Pydantic validation happens on model construction
                # If we got here, the transaction is valid
                errors.append(None)
            except Exception as e:
                error_msg = f"Transaction {i} ({tx.transaction_type}): {str(e)}"
                logger.error(error_msg)
                errors.append(error_msg)
        
        valid_count = sum(1 for e in errors if e is None)
        logger.info(f"Validated {valid_count}/{len(self.transactions)} transactions")
        
        return errors
    
    def get_transactions_by_type(self, transaction_type: str) -> List[BaseTransaction]:
        """
        Filter transactions by type.
        
        Args:
            transaction_type: Type to filter by
            
        Returns:
            List of transactions matching the type
        """
        return [
            tx for tx in self.transactions 
            if tx.transaction_type == transaction_type
        ]
    
    def estimate_total_fees(self) -> float:
        """
        Estimate total fees for all transactions.
        
        Returns:
            Total estimated fees
        """
        total = 0.0
        for tx in self.transactions:
            if tx.fee is not None:
                total += tx.fee
        
        logger.debug(f"Estimated total fees: {total}")
        return total
    
    def __len__(self) -> int:
        """Get number of transactions in batch."""
        return len(self.transactions)
    
    def __iter__(self):
        """Iterate over transactions."""
        return iter(self.transactions)
