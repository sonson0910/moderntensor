"""
Reward Distributor for fair token distribution.

Implements the epoch reward distribution scheme (IMPROVED v2):
- 35% Miners (based on performance scores)
- 30% Validators (based on stake)
- 10% Subnet Owners (emission-based)
- 10% Delegators (passive stakers)
- 10% DAO Treasury
- 5% Staking Bonus Pool (long-term lock rewards)
"""

from dataclasses import dataclass
from typing import Dict, Optional, List
from sdk.tokenomics.config import DistributionConfig, DEFAULT_DISTRIBUTION_CONFIG
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
        subnet_owner_rewards: Dict mapping subnet owner addresses to rewards
        delegator_rewards: Dict mapping delegator addresses to rewards
        staking_bonus_rewards: Dict mapping long-term stakers to bonus rewards
        dao_allocation: Amount allocated to DAO treasury
    """
    epoch: int
    total_distributed: int
    from_pool: int
    from_mint: int
    miner_rewards: Dict[str, int]
    validator_rewards: Dict[str, int]
    subnet_owner_rewards: Dict[str, int]
    delegator_rewards: Dict[str, int]
    staking_bonus_rewards: Dict[str, int] = None  # NEW: Long-term staking bonus
    dao_allocation: int = 0

    def __post_init__(self):
        if self.staking_bonus_rewards is None:
            self.staking_bonus_rewards = {}


@dataclass
class ParticipantInfo:
    """Information about network participants for distribution."""
    miner_scores: Dict[str, float]  # uid -> performance score (0-1)
    validator_stakes: Dict[str, int]  # address -> stake amount
    subnet_owners: Dict[str, int]  # address -> subnet emission weight
    delegator_stakes: Dict[str, int]  # address -> delegated stake
    long_term_stakers: Dict[str, tuple] = None  # address -> (stake, lock_days)

    def __post_init__(self):
        if self.long_term_stakers is None:
            self.long_term_stakers = {}


class RewardDistributor:
    """
    Distributes epoch rewards to all network participants.

    Distribution (IMPROVED v2):
    - 35% to Miners (performance-based)
    - 30% to Validators (stake-based)
    - 10% to Subnet Owners (emission-based)
    - 10% to Delegators (passive stakers)
    - 10% to DAO Treasury
    - 5% to Staking Bonus Pool (long-term lock rewards)
    - 15% to Subnet Owners (emission-based)
    - 10% to DAO Treasury
    - 5% to Delegators (stake-based)
    """

    def __init__(self, config: Optional[DistributionConfig] = None):
        """
        Initialize reward distributor.

        Args:
            config: Distribution configuration (uses defaults if not provided)
        """
        self.config = config or DEFAULT_DISTRIBUTION_CONFIG

    def distribute_epoch_rewards(
        self,
        epoch: int,
        total_emission: int,
        participants: ParticipantInfo,
        recycling_pool: Optional[RecyclingPool] = None
    ) -> DistributionResult:
        """
        Distribute rewards for an epoch.

        Args:
            epoch: Current epoch number
            total_emission: Total tokens to distribute
            participants: Information about all participants
            recycling_pool: Optional recycling pool for token sourcing

        Returns:
            DistributionResult with details
        """
        if total_emission < 0:
            raise ValueError(f"Total emission must be non-negative, got {total_emission}")

        # Get tokens from pool or indicate minting needed
        from_pool = 0
        from_mint = total_emission
        if recycling_pool:
            from_pool, from_mint = recycling_pool.allocate_rewards(total_emission)

        # Split into pools
        miner_pool = int(total_emission * self.config.miner_share)
        validator_pool = int(total_emission * self.config.validator_share)
        subnet_pool = int(total_emission * self.config.subnet_owner_share)
        dao_pool = int(total_emission * self.config.dao_share)
        delegator_pool = int(total_emission * self.config.delegator_share)

        # Distribute to each group
        miner_rewards = self._distribute_by_score(
            miner_pool,
            participants.miner_scores
        )

        validator_rewards = self._distribute_by_stake(
            validator_pool,
            participants.validator_stakes
        )

        subnet_owner_rewards = self._distribute_by_stake(
            subnet_pool,
            participants.subnet_owners
        )

        delegator_rewards = self._distribute_by_stake(
            delegator_pool,
            participants.delegator_stakes
        )

        return DistributionResult(
            epoch=epoch,
            total_distributed=total_emission,
            from_pool=from_pool,
            from_mint=from_mint,
            miner_rewards=miner_rewards,
            validator_rewards=validator_rewards,
            subnet_owner_rewards=subnet_owner_rewards,
            delegator_rewards=delegator_rewards,
            dao_allocation=dao_pool
        )

    def _distribute_by_score(
        self,
        pool: int,
        scores: Dict[str, float]
    ) -> Dict[str, int]:
        """
        Distribute pool proportional to performance scores.

        Args:
            pool: Total amount to distribute
            scores: Performance scores (0.0-1.0)

        Returns:
            Dict mapping addresses to reward amounts
        """
        if pool < 0:
            raise ValueError(f"Pool must be non-negative, got {pool}")

        if not scores or pool == 0:
            return {}

        # Validate scores
        for uid, score in scores.items():
            if not 0.0 <= score <= 1.0:
                raise ValueError(f"Score for {uid} must be between 0.0 and 1.0, got {score}")

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

    def _distribute_by_stake(
        self,
        pool: int,
        stakes: Dict[str, int]
    ) -> Dict[str, int]:
        """
        Distribute pool proportional to stake.

        Args:
            pool: Total amount to distribute
            stakes: Stake amounts

        Returns:
            Dict mapping addresses to reward amounts
        """
        if pool < 0:
            raise ValueError(f"Pool must be non-negative, got {pool}")

        if not stakes or pool == 0:
            return {}

        # Validate stakes
        for address, stake in stakes.items():
            if stake < 0:
                raise ValueError(f"Stake for {address} must be non-negative, got {stake}")

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

    def calculate_trust_bonus(
        self,
        base_reward: int,
        epochs_participated: int,
        k_t: float = 0.5
    ) -> int:
        """
        Calculate trust bonus for long-term participants.

        Formula: bonus = base_reward * k_t * log(1 + epochs_participated)

        Args:
            base_reward: Base reward amount
            epochs_participated: Number of epochs as participant
            k_t: Trust coefficient (default 0.5)

        Returns:
            Bonus amount to add
        """
        import math

        if epochs_participated < 0:
            raise ValueError(f"Epochs must be non-negative, got {epochs_participated}")

        # Trust multiplier: up to 2x for very long-term participants
        trust_multiplier = 1 + k_t * math.log(1 + epochs_participated / 100)
        trust_multiplier = min(trust_multiplier, 2.0)  # Cap at 2x

        bonus = int(base_reward * (trust_multiplier - 1))
        return bonus

    def calculate_quality_multiplier(
        self,
        consensus_score: float,
        k: float = 10.0
    ) -> float:
        """
        Calculate quality multiplier based on consensus score.

        Uses sigmoid function: multiplier = 0.6 + 0.8 * sigmoid(score - 0.5)

        Args:
            consensus_score: Consensus quality score (0-1)
            k: Sigmoid steepness (default 10)

        Returns:
            Multiplier (0.6 to 1.4)
        """
        import math

        if not 0.0 <= consensus_score <= 1.0:
            raise ValueError(f"Consensus score must be between 0 and 1, got {consensus_score}")

        # Sigmoid centered at 0.5
        x = consensus_score - 0.5
        sigmoid = 1 / (1 + math.exp(-k * x))

        # Scale to 0.6 - 1.4 range
        multiplier = 0.6 + 0.8 * sigmoid

        return multiplier

    def get_distribution_summary(self) -> Dict:
        """Get summary of distribution configuration."""
        return {
            "miner_share": f"{self.config.miner_share * 100:.0f}%",
            "validator_share": f"{self.config.validator_share * 100:.0f}%",
            "subnet_owner_share": f"{self.config.subnet_owner_share * 100:.0f}%",
            "dao_share": f"{self.config.dao_share * 100:.0f}%",
            "delegator_share": f"{self.config.delegator_share * 100:.0f}%",
        }
