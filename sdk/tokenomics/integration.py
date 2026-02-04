"""
Tokenomics Integration with Layer 1 Blockchain.

This module tightly integrates tokenomics with the Layer 1 consensus system,
providing complete epoch-based token economics.
"""

from dataclasses import dataclass
from typing import Dict, Optional
from sdk.tokenomics.emission_controller import EmissionController
from sdk.tokenomics.recycling_pool import RecyclingPool
from sdk.tokenomics.reward_distributor import RewardDistributor
from sdk.tokenomics.burn_manager import BurnManager
from sdk.tokenomics.claim_manager import ClaimManager
from sdk.tokenomics.metrics_collector import NetworkMetrics
from sdk.tokenomics.config import TokenomicsConfig, DistributionConfig


@dataclass
class ConsensusData:
    """
    Consensus data from Layer 1.
    
    Attributes:
        miner_scores: Dict mapping miner UIDs to performance scores
        validator_stakes: Dict mapping validator addresses to stake amounts
        quality_score: Overall network quality (0.0-1.0)
    """
    miner_scores: Dict[str, float]
    validator_stakes: Dict[str, int]
    quality_score: float


@dataclass
class EpochTokenomics:
    """
    Complete tokenomics result for an epoch.
    
    Attributes:
        epoch: Epoch number
        utility_score: Network utility score
        emission_amount: Total emission for epoch
        burned_amount: Amount burned this epoch
        miner_pool: Total rewards for miners
        validator_pool: Total rewards for validators
        dao_allocation: DAO treasury allocation
        claim_root: Merkle root for claims
        from_pool: Amount from recycling pool
        from_mint: Amount minted
    """
    epoch: int
    utility_score: float
    emission_amount: int
    burned_amount: int
    miner_pool: int
    validator_pool: int
    dao_allocation: int
    claim_root: bytes
    from_pool: int
    from_mint: int


class TokenomicsIntegration:
    """
    Integrates tokenomics with Layer 1 blockchain.
    
    This class processes complete tokenomics for each consensus epoch,
    tightly coupled with Layer1ConsensusIntegrator.
    """
    
    def __init__(
        self,
        tokenomics_config: Optional[TokenomicsConfig] = None,
        distribution_config: Optional[DistributionConfig] = None
    ):
        """
        Initialize tokenomics integration.
        
        Args:
            tokenomics_config: Tokenomics configuration
            distribution_config: Distribution configuration
        """
        self.emission = EmissionController(tokenomics_config)
        self.pool = RecyclingPool()
        self.distributor = RewardDistributor(distribution_config)
        self.burn = BurnManager()
        self.claims = ClaimManager()
    
    def process_epoch_tokenomics(
        self,
        epoch: int,
        consensus_data: ConsensusData,
        network_metrics: NetworkMetrics
    ) -> EpochTokenomics:
        """
        Process complete tokenomics for an epoch.
        
        This is the main integration point called at the end of each
        consensus epoch. It:
        1. Calculates utility score
        2. Determines emission amount
        3. Sources tokens (pool or mint)
        4. Distributes rewards
        5. Handles burns
        6. Creates claim tree
        
        Args:
            epoch: Current epoch number
            consensus_data: Consensus results from Layer 1
            network_metrics: Network activity metrics
            
        Returns:
            EpochTokenomics with complete results
        """
        # 1. Calculate utility score
        utility = self.emission.calculate_utility_score(
            task_volume=network_metrics.task_count,
            avg_task_difficulty=network_metrics.avg_difficulty,
            validator_participation=network_metrics.validator_ratio
        )
        
        # 2. Calculate emission amount
        epoch_emission = self.emission.calculate_epoch_emission(
            utility_score=utility,
            epoch=epoch
        )
        
        # 3. Distribute rewards
        distribution = self.distributor.distribute_epoch_rewards(
            epoch=epoch,
            total_emission=epoch_emission,
            miner_scores=consensus_data.miner_scores,
            validator_stakes=consensus_data.validator_stakes,
            recycling_pool=self.pool
        )
        
        # 4. Handle burns (if network quality is poor)
        burned = self.burn.burn_unmet_quota(
            expected_emission=epoch_emission,
            actual_quality_score=consensus_data.quality_score
        )
        
        # 5. Update supply if minting occurred
        if distribution.from_mint > 0:
            self.emission.update_supply(distribution.from_mint)
        
        # 6. Create claim tree for all participants
        all_rewards = {
            **distribution.miner_rewards,
            **distribution.validator_rewards
        }
        claim_root = self.claims.create_claim_tree(
            epoch=epoch,
            rewards=all_rewards
        )
        
        # 7. Return complete tokenomics data
        return EpochTokenomics(
            epoch=epoch,
            utility_score=utility,
            emission_amount=epoch_emission,
            burned_amount=burned,
            miner_pool=sum(distribution.miner_rewards.values()),
            validator_pool=sum(distribution.validator_rewards.values()),
            dao_allocation=distribution.dao_allocation,
            claim_root=claim_root,
            from_pool=distribution.from_pool,
            from_mint=distribution.from_mint
        )
    
    def add_to_recycling_pool(
        self,
        amount: int,
        source: str
    ) -> None:
        """
        Add tokens to recycling pool.
        
        This should be called when fees or penalties are collected.
        
        Args:
            amount: Amount to add
            source: Source of tokens
        """
        self.pool.add_to_pool(amount, source)
    
    def get_claim_proof(
        self,
        epoch: int,
        address: str
    ) -> Optional[list]:
        """
        Get claim proof for an address.
        
        Args:
            epoch: Epoch number
            address: Claimer address
            
        Returns:
            Merkle proof or None
        """
        return self.claims.get_claim_proof(epoch, address)
    
    def verify_and_claim(
        self,
        epoch: int,
        address: str,
        amount: int,
        proof: list
    ) -> bool:
        """
        Verify and process a claim.
        
        Args:
            epoch: Epoch number
            address: Claimer address
            amount: Claimed amount
            proof: Merkle proof
            
        Returns:
            True if claim successful
        """
        return self.claims.claim_reward(epoch, address, amount, proof)
    
    def get_stats(self) -> Dict:
        """
        Get comprehensive tokenomics statistics.
        
        Returns:
            Dictionary with all tokenomics metrics
        """
        return {
            'supply': self.emission.get_supply_info(),
            'recycling_pool': self.pool.get_pool_stats(),
            'burns': self.burn.get_burn_stats()
        }
