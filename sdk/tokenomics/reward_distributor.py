"""
Reward Distributor for fair token distribution.

This module manages reward distribution to miners, validators, and DAO treasury.
"""

from dataclasses import dataclass
from typing import Dict, Optional
from sdk.tokenomics.config import DistributionConfig
from sdk.tokenomics.recycling_pool import RecyclingPool


@dataclass
class DistributionResult:
    """
    Result of reward distribution for an epoch.
    
    Attributes:
        epoch: Epoch number
        total_distributed: Total amount distributed
        from_pool: Amount from recycling pool
        from_mint: Amount from minting
        miner_rewards: Dict mapping miner UIDs to reward amounts
        validator_rewards: Dict mapping validator addresses to reward amounts
        dao_allocation: Amount allocated to DAO treasury
    """
    epoch: int
    total_distributed: int
    from_pool: int
    from_mint: int
    miner_rewards: Dict[str, int]
    validator_rewards: Dict[str, int]
    dao_allocation: int


class RewardDistributor:
    """
    Distributes rewards to miners, validators, and DAO.
    
    Default split:
    - 40% to Miners (based on performance scores)
    - 40% to Validators (based on stake)
    - 20% to DAO treasury
    """
    
    def __init__(self, config: Optional[DistributionConfig] = None):
        """
        Initialize reward distributor.
        
        Args:
            config: Distribution configuration (uses defaults if not provided)
        """
        self.config = config or DistributionConfig()
    
    def distribute_epoch_rewards(
        self,
        epoch: int,
        total_emission: int,
        miner_scores: Dict[str, float],
        validator_stakes: Dict[str, int],
        recycling_pool: RecyclingPool
    ) -> DistributionResult:
        """
        Distribute rewards for an epoch.
        
        Args:
            epoch: Current epoch number
            total_emission: Total tokens to distribute
            miner_scores: Dict mapping miner UIDs to performance scores (0.0-1.0)
            validator_stakes: Dict mapping validator addresses to stake amounts
            recycling_pool: Recycling pool for token sourcing
            
        Returns:
            DistributionResult with details
        """
        if total_emission < 0:
            raise ValueError(f"Total emission must be non-negative, got {total_emission}")
        
        # Get tokens from pool or indicate minting needed
        from_pool, from_mint = recycling_pool.allocate_rewards(total_emission)
        
        # Split into pools
        miner_pool = int(total_emission * self.config.miner_share)
        validator_pool = int(total_emission * self.config.validator_share)
        dao_pool = int(total_emission * self.config.dao_share)
        
        # Distribute to miners (by performance)
        miner_rewards = self._distribute_to_miners(miner_pool, miner_scores)
        
        # Distribute to validators (by stake)
        validator_rewards = self._distribute_to_validators(validator_pool, validator_stakes)
        
        return DistributionResult(
            epoch=epoch,
            total_distributed=total_emission,
            from_pool=from_pool,
            from_mint=from_mint,
            miner_rewards=miner_rewards,
            validator_rewards=validator_rewards,
            dao_allocation=dao_pool
        )
    
    def _distribute_to_miners(
        self,
        pool: int,
        scores: Dict[str, float]
    ) -> Dict[str, int]:
        """
        Distribute pool to miners proportional to scores.
        
        Args:
            pool: Total amount to distribute
            scores: Miner performance scores
            
        Returns:
            Dict mapping miner UIDs to reward amounts
        """
        if pool < 0:
            raise ValueError(f"Pool must be non-negative, got {pool}")
        
        if not scores or pool == 0:
            return {}
        
        # Validate scores
        for uid, score in scores.items():
            if not 0.0 <= score <= 1.0:
                raise ValueError(f"Miner score for {uid} must be between 0.0 and 1.0, got {score}")
        
        # Calculate total score
        total_score = sum(scores.values())
        if total_score == 0:
            return {}
        
        # Distribute proportionally
        rewards = {}
        for uid, score in scores.items():
            reward = int((score / total_score) * pool)
            if reward > 0:
                rewards[uid] = reward
        
        return rewards
    
    def _distribute_to_validators(
        self,
        pool: int,
        stakes: Dict[str, int]
    ) -> Dict[str, int]:
        """
        Distribute pool to validators proportional to stake.
        
        Args:
            pool: Total amount to distribute
            stakes: Validator stakes
            
        Returns:
            Dict mapping validator addresses to reward amounts
        """
        if pool < 0:
            raise ValueError(f"Pool must be non-negative, got {pool}")
        
        if not stakes or pool == 0:
            return {}
        
        # Validate stakes
        for address, stake in stakes.items():
            if stake < 0:
                raise ValueError(f"Validator stake for {address} must be non-negative, got {stake}")
        
        # Calculate total stake
        total_stake = sum(stakes.values())
        if total_stake == 0:
            return {}
        
        # Distribute proportionally
        rewards = {}
        for address, stake in stakes.items():
            reward = int((stake / total_stake) * pool)
            if reward > 0:
                rewards[address] = reward
        
        return rewards
