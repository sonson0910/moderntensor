"""
Recycling Pool for token reuse.

This module manages token recycling from fees, slashing, and penalties,
reducing the need for new token minting.
"""

from typing import Dict


class RecyclingPool:
    """
    Manages token recycling from various sources.
    
    Tokens are collected from:
    - Registration fees
    - Slashing penalties
    - Task fees
    - Transaction fees
    
    Recycled tokens are prioritized over minting new tokens.
    """
    
    def __init__(self):
        """Initialize recycling pool."""
        self.pool_balance = 0
        self.sources = {
            'registration_fees': 0,
            'slashing_penalties': 0,
            'task_fees': 0,
            'transaction_fees': 0
        }
        self.total_recycled = 0
        self.total_allocated = 0
    
    def add_to_pool(
        self,
        amount: int,
        source: str
    ) -> None:
        """
        Add tokens to recycling pool.
        
        Args:
            amount: Amount to add
            source: Source of tokens (must be valid source key)
        """
        if amount < 0:
            raise ValueError(f"Amount must be non-negative, got {amount}")
        
        if source not in self.sources:
            raise ValueError(f"Invalid source: {source}. Must be one of {list(self.sources.keys())}")
        
        self.pool_balance += amount
        self.sources[source] += amount
        self.total_recycled += amount
    
    def allocate_rewards(
        self,
        required_amount: int
    ) -> tuple[int, int]:
        """
        Allocate rewards from pool or indicate amount to mint.
        
        Args:
            required_amount: Total rewards needed
            
        Returns:
            Tuple of (from_pool, from_mint)
        """
        if required_amount < 0:
            raise ValueError(f"Required amount must be non-negative, got {required_amount}")
        
        # Try to use recycled tokens first
        from_pool = min(self.pool_balance, required_amount)
        from_mint = required_amount - from_pool
        
        # Deduct from pool
        if from_pool > 0:
            self.pool_balance -= from_pool
            self.total_allocated += from_pool
        
        return (from_pool, from_mint)
    
    def get_pool_stats(self) -> Dict:
        """
        Get pool statistics.
        
        Returns:
            Dictionary with pool metrics
        """
        return {
            'total_balance': self.pool_balance,
            'sources': self.sources.copy(),
            'total_recycled': self.total_recycled,
            'total_allocated': self.total_allocated,
            'utilization_rate': self._calculate_utilization()
        }
    
    def _calculate_utilization(self) -> float:
        """
        Calculate pool utilization rate.
        
        Returns:
            Utilization rate (0.0 to 1.0)
        """
        if self.total_recycled == 0:
            return 0.0
        return min(self.total_allocated / self.total_recycled, 1.0)
