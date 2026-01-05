"""
Emission Controller for adaptive token emission.

This module manages adaptive emission based on network utility,
providing a superior alternative to fixed emission models.
"""

from typing import Optional
from sdk.tokenomics.config import TokenomicsConfig


class EmissionController:
    """
    Manages adaptive token emission based on network utility.
    
    Core Formula:
        MintAmount = BaseReward × UtilityScore × EmissionMultiplier
    
    Where:
        - BaseReward: Base reward per epoch (decreases over time via halving)
        - UtilityScore: 0.0 to 1.0 based on network activity
        - EmissionMultiplier: Halving schedule (0.5^n where n = halvings)
    """
    
    def __init__(self, config: Optional[TokenomicsConfig] = None):
        """
        Initialize emission controller.
        
        Args:
            config: Tokenomics configuration (uses defaults if not provided)
        """
        self.config = config or TokenomicsConfig()
        self.current_supply = 0
    
    def calculate_epoch_emission(
        self,
        utility_score: float,
        epoch: int
    ) -> int:
        """
        Calculate emission for current epoch.
        
        Args:
            utility_score: Network utility score (0.0 to 1.0)
            epoch: Current epoch number
            
        Returns:
            Token amount to mint this epoch
        """
        # Validate utility score
        if not 0.0 <= utility_score <= 1.0:
            raise ValueError(f"Utility score must be between 0.0 and 1.0, got {utility_score}")
        
        # Calculate halving multiplier
        halvings = epoch // self.config.halving_interval
        emission_multiplier = 0.5 ** halvings
        
        # Calculate adaptive emission
        mint_amount = (
            self.config.base_reward * 
            utility_score * 
            emission_multiplier
        )
        
        # Cap at max supply
        if self.current_supply + mint_amount > self.config.max_supply:
            mint_amount = max(0, self.config.max_supply - self.current_supply)
        
        return int(mint_amount)
    
    def calculate_utility_score(
        self,
        task_volume: int,
        avg_task_difficulty: float,
        validator_participation: float
    ) -> float:
        """
        Calculate network utility score.
        
        Formula:
            U = w1 × TaskVolumeScore + 
                w2 × DifficultyScore + 
                w3 × ParticipationScore
        
        Where w1 + w2 + w3 = 1.0
        
        Args:
            task_volume: Number of tasks completed in epoch
            avg_task_difficulty: Average difficulty (0.0 to 1.0)
            validator_participation: Validator participation ratio (0.0 to 1.0)
            
        Returns:
            Utility score (0.0 to 1.0)
        """
        # Get weights
        w1, w2, w3 = self.config.utility_weights
        
        # Normalize task volume (0-1 scale)
        max_tasks = self.config.max_expected_tasks
        task_score = min(task_volume / max_tasks, 1.0) if max_tasks > 0 else 0.0
        
        # Validate inputs
        if not 0.0 <= avg_task_difficulty <= 1.0:
            raise ValueError(f"Task difficulty must be between 0.0 and 1.0, got {avg_task_difficulty}")
        if not 0.0 <= validator_participation <= 1.0:
            raise ValueError(f"Validator participation must be between 0.0 and 1.0, got {validator_participation}")
        
        # Calculate weighted utility
        utility = (
            w1 * task_score +
            w2 * avg_task_difficulty +
            w3 * validator_participation
        )
        
        return min(utility, 1.0)
    
    def update_supply(self, amount: int) -> None:
        """
        Update current supply after minting.
        
        Args:
            amount: Amount minted
        """
        self.current_supply += amount
        if self.current_supply > self.config.max_supply:
            self.current_supply = self.config.max_supply
    
    def get_supply_info(self) -> dict:
        """
        Get supply information.
        
        Returns:
            Dictionary with supply metrics
        """
        return {
            'current_supply': self.current_supply,
            'max_supply': self.config.max_supply,
            'remaining_supply': self.config.max_supply - self.current_supply,
            'supply_percentage': (self.current_supply / self.config.max_supply * 100) if self.config.max_supply > 0 else 0
        }
