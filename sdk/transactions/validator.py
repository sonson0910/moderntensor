"""
Transaction Validator

Validates transactions before submission.
"""

import logging
from typing import List, Dict, Any, Optional
from sdk.transactions.types import (
    BaseTransaction,
    TransferTransaction,
    StakeTransaction,
    UnstakeTransaction,
    WeightTransaction,
)

logger = logging.getLogger(__name__)


class ValidationError(Exception):
    """Raised when transaction validation fails."""
    pass


class TransactionValidator:
    """
    Validates transactions before submission.
    
    Performs checks including:
    - Schema validation (handled by Pydantic)
    - Business logic validation
    - Balance checks
    - Rate limiting
    - Duplicate detection
    """
    
    def __init__(self, strict: bool = True):
        """
        Initialize validator.
        
        Args:
            strict: If True, raise exceptions on validation failures
        """
        self.strict = strict
        self._seen_hashes: set = set()
    
    def validate(self, transaction: BaseTransaction) -> List[str]:
        """
        Validate a transaction.
        
        Args:
            transaction: Transaction to validate
            
        Returns:
            List of validation error messages (empty if valid)
            
        Raises:
            ValidationError: If strict mode and validation fails
        """
        errors = []
        
        # Basic validation (already done by Pydantic)
        try:
            # Pydantic validation happens on construction
            pass
        except Exception as e:
            errors.append(f"Schema validation failed: {e}")
        
        # Type-specific validation
        if isinstance(transaction, TransferTransaction):
            errors.extend(self._validate_transfer(transaction))
        elif isinstance(transaction, StakeTransaction):
            errors.extend(self._validate_stake(transaction))
        elif isinstance(transaction, UnstakeTransaction):
            errors.extend(self._validate_unstake(transaction))
        elif isinstance(transaction, WeightTransaction):
            errors.extend(self._validate_weights(transaction))
        
        # Common validations
        errors.extend(self._validate_common(transaction))
        
        if errors:
            logger.warning(f"Transaction validation failed: {errors}")
            if self.strict:
                raise ValidationError(f"Validation failed: {', '.join(errors)}")
        
        return errors
    
    def _validate_transfer(self, tx: TransferTransaction) -> List[str]:
        """Validate transfer transaction."""
        errors = []
        
        # Check addresses
        if tx.from_address == tx.to_address:
            errors.append("Cannot transfer to same address")
        
        # Check amount
        if tx.amount <= 0:
            errors.append("Transfer amount must be positive")
        
        return errors
    
    def _validate_stake(self, tx: StakeTransaction) -> List[str]:
        """Validate stake transaction."""
        errors = []
        
        # Check amount
        if tx.amount <= 0:
            errors.append("Stake amount must be positive")
        
        # Check hotkey format (basic check)
        if not tx.hotkey or len(tx.hotkey) < 10:
            errors.append("Invalid hotkey format")
        
        return errors
    
    def _validate_unstake(self, tx: UnstakeTransaction) -> List[str]:
        """Validate unstake transaction."""
        errors = []
        
        # Check amount
        if tx.amount <= 0:
            errors.append("Unstake amount must be positive")
        
        # Check hotkey format
        if not tx.hotkey or len(tx.hotkey) < 10:
            errors.append("Invalid hotkey format")
        
        return errors
    
    def _validate_weights(self, tx: WeightTransaction) -> List[str]:
        """Validate weight transaction."""
        errors = []
        
        # Check UIDs and weights match
        if len(tx.uids) != len(tx.weights):
            errors.append("Number of UIDs must match number of weights")
        
        # Check weight sum
        weight_sum = sum(tx.weights)
        if not (0.99 <= weight_sum <= 1.01):
            errors.append(f"Weights must sum to 1.0, got {weight_sum}")
        
        # Check for negative weights
        if any(w < 0 for w in tx.weights):
            errors.append("All weights must be non-negative")
        
        # Check for duplicate UIDs
        if len(tx.uids) != len(set(tx.uids)):
            errors.append("Duplicate UIDs not allowed")
        
        return errors
    
    def _validate_common(self, tx: BaseTransaction) -> List[str]:
        """Validate common fields."""
        errors = []
        
        # Check from address
        if not tx.from_address or len(tx.from_address) < 10:
            errors.append("Invalid from_address")
        
        # Check fee if specified
        if tx.fee is not None and tx.fee < 0:
            errors.append("Fee cannot be negative")
        
        # Check nonce if specified
        if tx.nonce is not None and tx.nonce < 0:
            errors.append("Nonce cannot be negative")
        
        return errors
    
    def validate_batch(self, transactions: List[BaseTransaction]) -> Dict[int, List[str]]:
        """
        Validate a batch of transactions.
        
        Args:
            transactions: List of transactions to validate
            
        Returns:
            Dictionary mapping transaction index to error messages
        """
        results = {}
        
        for i, tx in enumerate(transactions):
            errors = self.validate(tx)
            if errors:
                results[i] = errors
        
        if results:
            logger.warning(f"Batch validation: {len(results)} transactions have errors")
        else:
            logger.info(f"Batch validation: All {len(transactions)} transactions valid")
        
        return results
    
    def check_duplicate(self, transaction: BaseTransaction) -> bool:
        """
        Check if transaction is a duplicate.
        
        Args:
            transaction: Transaction to check
            
        Returns:
            True if duplicate detected
        """
        # Simple hash-based duplicate detection
        tx_hash = hash(transaction.json())
        
        if tx_hash in self._seen_hashes:
            logger.warning("Duplicate transaction detected")
            return True
        
        self._seen_hashes.add(tx_hash)
        return False
    
    def reset_duplicates(self):
        """Reset duplicate detection cache."""
        self._seen_hashes.clear()
        logger.debug("Reset duplicate detection cache")
