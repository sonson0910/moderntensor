"""
Block Reward Halving Schedule

Ported from luxtensor-consensus/src/halving.rs

Implements Bitcoin-like halving to ensure sustainable token emission:
- Initial reward: 2 MDT per block
- Halving interval: ~3.3 years
- Total emission from rewards: 45% of 21M = 9.45M MDT
"""

from dataclasses import dataclass
import logging

logger = logging.getLogger(__name__)


# Constants matching Rust implementation
INITIAL_BLOCK_REWARD = 2_000_000_000_000_000_000  # 2 MDT with 18 decimals
HALVING_INTERVAL = 1_051_200  # ~3.33 years at 100s block time
MINIMUM_REWARD = 1_000_000_000_000_000  # 0.001 MDT
MAX_HALVINGS = 10


@dataclass
class HalvingInfo:
    """Human-readable halving information."""
    initial_reward_mdt: float
    halving_interval_blocks: int
    halving_interval_years: float
    max_halvings: int
    estimated_total_emission_mdt: float


class HalvingSchedule:
    """
    Block reward halving schedule.

    Implements Bitcoin-like halving where block rewards are halved
    at regular intervals to ensure sustainable token emission.

    Usage:
        schedule = HalvingSchedule()

        # Calculate reward for a block
        reward = schedule.calculate_reward(block_height)

        # Get info
        info = schedule.summary()
    """

    def __init__(
        self,
        initial_reward: int = INITIAL_BLOCK_REWARD,
        halving_interval: int = HALVING_INTERVAL,
        minimum_reward: int = MINIMUM_REWARD,
        max_halvings: int = MAX_HALVINGS,
    ):
        self.initial_reward = initial_reward
        self.halving_interval = halving_interval
        self.minimum_reward = minimum_reward
        self.max_halvings = max_halvings

    def calculate_reward(self, block_height: int) -> int:
        """
        Calculate block reward for a given block height.

        Formula: reward = initial_reward / 2^halvings

        Examples:
            - Block 0:         2.0 MDT
            - Block 1M:        2.0 MDT (before first halving)
            - Block 1,051,200: 1.0 MDT (first halving)
            - Block 2,102,400: 0.5 MDT (second halving)
        """
        halvings = block_height // self.halving_interval

        # After max halvings, reward is 0
        if halvings > self.max_halvings:
            return 0

        effective_halvings = min(halvings, self.max_halvings)

        # Calculate reward: initial_reward >> halvings
        reward = self.initial_reward >> effective_halvings

        # Return 0 if below minimum threshold
        if reward < self.minimum_reward:
            return 0

        return reward

    def get_halving_era(self, block_height: int) -> int:
        """Get current halving era (0 = before first halving)."""
        return min(block_height // self.halving_interval, self.max_halvings)

    def blocks_until_next_halving(self, block_height: int) -> int:
        """Calculate remaining blocks until next halving."""
        current_era = self.get_halving_era(block_height)
        if current_era >= self.max_halvings:
            return 0  # No more halvings

        next_halving_block = (current_era + 1) * self.halving_interval
        return max(0, next_halving_block - block_height)

    def total_emitted(self, block_height: int) -> int:
        """Calculate total emitted tokens up to a block height."""
        total = 0
        current_block = 0

        for era in range(self.max_halvings + 1):
            era_start = era * self.halving_interval
            era_end = min((era + 1) * self.halving_interval, block_height)

            if current_block >= block_height:
                break

            if era_start < block_height:
                blocks_in_era = min(
                    era_end - era_start,
                    block_height - era_start
                )
                reward_per_block = self.initial_reward >> era

                if reward_per_block >= self.minimum_reward:
                    total += blocks_in_era * reward_per_block

            current_block = era_end

        return total

    def estimate_total_emission(self) -> int:
        """Estimate total supply from block rewards (after all halvings)."""
        blocks_per_halving = self.halving_interval
        total = 0

        for era in range(self.max_halvings + 1):
            reward = self.initial_reward >> era
            if reward >= self.minimum_reward:
                total += blocks_per_halving * reward

        return total

    def summary(self) -> HalvingInfo:
        """Get halving schedule info as human-readable summary."""
        return HalvingInfo(
            initial_reward_mdt=self.initial_reward / 1e18,
            halving_interval_blocks=self.halving_interval,
            halving_interval_years=(self.halving_interval * 100.0) / (365.25 * 24 * 3600),
            max_halvings=self.max_halvings,
            estimated_total_emission_mdt=self.estimate_total_emission() / 1e18,
        )

    def get_status(self, block_height: int) -> dict:
        """Get current halving status."""
        return {
            "era": self.get_halving_era(block_height),
            "current_reward": self.calculate_reward(block_height),
            "current_reward_mdt": self.calculate_reward(block_height) / 1e18,
            "blocks_until_next": self.blocks_until_next_halving(block_height),
            "total_emitted": self.total_emitted(block_height),
            "total_emitted_mdt": self.total_emitted(block_height) / 1e18,
        }
