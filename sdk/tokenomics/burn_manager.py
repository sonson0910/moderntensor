"""
Burn Manager for token burning mechanisms.

This module manages various token burning scenarios to maintain
token value and incentivize network quality.
"""

from typing import Dict


class BurnManager:
    """
    Manages token burning mechanisms.
    
    Burn scenarios:
    - Unmet quality quota: Burns tokens if network quality is below threshold
    - Transaction fees: Burns a percentage of transaction fees
    - Quality penalties: Burns tokens for poor performance
    """
    
    def __init__(self):
        """Initialize burn manager."""
        self.total_burned = 0
        self.burn_reasons = {
            'unmet_quota': 0,
            'transaction_fees': 0,
            'quality_penalty': 0
        }
    
    def burn_unmet_quota(
        self,
        expected_emission: int,
        actual_quality_score: float,
        threshold: float = 0.5
    ) -> int:
        """
        Burn tokens if network quality doesn't meet threshold.
        
        This mechanism incentivizes network participants to maintain
        high quality standards.
        
        Args:
            expected_emission: Emission planned for epoch
            actual_quality_score: Actual network quality (0.0 to 1.0)
            threshold: Minimum quality threshold (default 0.5)
            
        Returns:
            Amount burned
        """
        if not 0.0 <= actual_quality_score <= 1.0:
            raise ValueError(f"Quality score must be between 0.0 and 1.0, got {actual_quality_score}")
        
        if not 0.0 <= threshold <= 1.0:
            raise ValueError(f"Threshold must be between 0.0 and 1.0, got {threshold}")
        
        if expected_emission < 0:
            raise ValueError(f"Expected emission must be non-negative, got {expected_emission}")
        
        if actual_quality_score < threshold:
            # Burn proportional to quality deficit
            quality_deficit = threshold - actual_quality_score
            burn_amount = int(expected_emission * quality_deficit)
            self._burn(burn_amount, 'unmet_quota')
            return burn_amount
        
        return 0
    
    def burn_transaction_fees(
        self,
        fee_amount: int,
        burn_percentage: float = 0.5
    ) -> int:
        """
        Burn percentage of transaction fees.
        
        This creates deflationary pressure and aligns with
        network usage growth.
        
        Args:
            fee_amount: Total fees collected
            burn_percentage: Percentage to burn (default 50%)
            
        Returns:
            Amount burned
        """
        if fee_amount < 0:
            raise ValueError(f"Fee amount must be non-negative, got {fee_amount}")
        
        if not 0.0 <= burn_percentage <= 1.0:
            raise ValueError(f"Burn percentage must be between 0.0 and 1.0, got {burn_percentage}")
        
        burn_amount = int(fee_amount * burn_percentage)
        self._burn(burn_amount, 'transaction_fees')
        return burn_amount
    
    def burn_quality_penalty(
        self,
        penalty_amount: int
    ) -> int:
        """
        Burn tokens as quality penalty.
        
        Args:
            penalty_amount: Amount to burn as penalty
            
        Returns:
            Amount burned
        """
        if penalty_amount < 0:
            raise ValueError(f"Penalty amount must be non-negative, got {penalty_amount}")
        
        self._burn(penalty_amount, 'quality_penalty')
        return penalty_amount
    
    def _burn(self, amount: int, reason: str) -> None:
        """
        Internal burn function.
        
        Args:
            amount: Amount to burn
            reason: Reason for burning
        """
        if reason not in self.burn_reasons:
            raise ValueError(f"Invalid burn reason: {reason}")
        
        self.total_burned += amount
        self.burn_reasons[reason] += amount
    
    def get_burn_stats(self) -> Dict:
        """
        Get burn statistics.
        
        Returns:
            Dictionary with burn metrics
        """
        return {
            'total_burned': self.total_burned,
            'burn_reasons': self.burn_reasons.copy()
        }
