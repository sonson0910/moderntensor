"""
Configuration classes for tokenomics module.
"""

from dataclasses import dataclass
from typing import Optional


@dataclass
class TokenomicsConfig:
    """
    Configuration for tokenomics system.
    
    Attributes:
        max_supply: Maximum token supply (e.g., 21M)
        base_reward: Initial base reward per epoch
        halving_interval: Number of epochs between halvings
        max_expected_tasks: Maximum expected tasks per epoch (for normalization)
        utility_weights: Weights for utility score calculation (task, difficulty, participation)
    """
    max_supply: int = 21_000_000
    base_reward: int = 1000
    halving_interval: int = 210_000  # ~4 years at 10min/epoch
    max_expected_tasks: int = 10_000
    utility_weights: tuple[float, float, float] = (0.5, 0.3, 0.2)  # (task, difficulty, participation)
    
    def __post_init__(self):
        """Validate configuration."""
        w1, w2, w3 = self.utility_weights
        if not abs(w1 + w2 + w3 - 1.0) < 0.001:
            raise ValueError(f"Utility weights must sum to 1.0, got {w1 + w2 + w3}")


@dataclass
class DistributionConfig:
    """
    Configuration for reward distribution.
    
    Attributes:
        miner_share: Percentage of rewards for miners (0.0-1.0)
        validator_share: Percentage of rewards for validators (0.0-1.0)
        dao_share: Percentage of rewards for DAO treasury (0.0-1.0)
    """
    miner_share: float = 0.40
    validator_share: float = 0.40
    dao_share: float = 0.20
    
    def __post_init__(self):
        """Validate configuration."""
        total = self.miner_share + self.validator_share + self.dao_share
        if not abs(total - 1.0) < 0.001:
            raise ValueError(f"Distribution shares must sum to 1.0, got {total}")
