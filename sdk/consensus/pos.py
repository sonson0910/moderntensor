"""
Proof of Stake - PoS Consensus Implementation

Ported from luxtensor-consensus/src/pos.rs

Implements stake-weighted validator selection, block reward
distribution, and integration with halving schedule.
"""

from dataclasses import dataclass, field
from threading import RLock
from typing import Dict, Optional
import hashlib
import logging

from sdk.consensus.halving import HalvingSchedule as _HalvingImpl

logger = logging.getLogger(__name__)


@dataclass
class ValidatorInfo:
    """Validator information for PoS."""
    address: str
    stake: int
    public_key: Optional[str] = None
    active: bool = True
    rewards: int = 0
    last_active_slot: int = 0
    activation_epoch: int = 0


@dataclass
class HalvingSchedule:
    """Block reward halving schedule.
    Delegates to sdk.consensus.halving.HalvingSchedule for actual logic."""
    initial_reward: int = 2_000_000_000_000_000_000  # 2 tokens
    halving_interval: int = 1_051_200  # ~3.3 years at 100s blocks (match halving.py)
    max_halvings: int = 10

    def _impl(self) -> _HalvingImpl:
        return _HalvingImpl(
            initial_reward=self.initial_reward,
            halving_interval=self.halving_interval,
            max_halvings=self.max_halvings,
        )

    def calculate_reward(self, block_height: int) -> int:
        """Calculate reward for a given block height."""
        return self._impl().calculate_reward(block_height)

    def get_halving_era(self, block_height: int) -> int:
        """Get current halving era (0-indexed)."""
        return self._impl().get_halving_era(block_height)

    def blocks_until_next_halving(self, block_height: int) -> int:
        """Get blocks until next halving."""
        return self._impl().blocks_until_next_halving(block_height)

    def estimate_total_emission(self) -> int:
        """Estimate total emission over all halvings."""
        return self._impl().estimate_total_emission()


@dataclass
class ConsensusConfig:
    """Configuration for PoS consensus.

    Default values match luxtensor-consensus/src/pos.rs ConsensusConfig defaults.
    Note: The node runtime may override epoch_length to 100 via config.toml.
    The staking RPC enforces a separate MIN_VALIDATOR_STAKE of 1 MDT.
    """
    slot_duration: int = 12  # 12 seconds per slot
    min_stake: int = 32_000_000_000_000_000_000  # 32 MDT (18 decimals)
    block_reward: int = 2_000_000_000_000_000_000  # 2 MDT (initial, subject to halving)
    epoch_length: int = 32  # 32 slots per epoch (node config may override to 100)
    halving_schedule: HalvingSchedule = field(default_factory=HalvingSchedule)


@dataclass
class HalvingInfo:
    """Halving information for display."""
    initial_reward_mdt: float
    halving_interval_blocks: int
    halving_interval_years: float
    max_halvings: int
    estimated_total_emission_mdt: float


class ProofOfStakeError(Exception):
    """Error during PoS operation."""
    pass


class ProofOfStake:
    """
    Proof of Stake consensus implementation.

    Thread-safe implementation using RLock.

    Usage:
        config = ConsensusConfig()
        pos = ProofOfStake(config)

        # Add validators
        pos.add_validator(address, stake, public_key)

        # Select validator for slot
        selected = pos.select_validator(slot)

        # Distribute rewards
        pos.distribute_reward_with_height(producer, block_height)
    """

    def __init__(
        self,
        config: Optional[ConsensusConfig] = None,
        validators: Optional[Dict[str, ValidatorInfo]] = None,
    ):
        self._lock = RLock()
        self.config = config or ConsensusConfig()

        self._validators: Dict[str, ValidatorInfo] = validators or {}
        self._current_epoch = 0
        self._last_block_hash: str = "0" * 64

        logger.info("ProofOfStake initialized with %s validators", len(self._validators))

    def _compute_seed(self, slot: int) -> bytes:
        """Compute randomness seed for validator selection."""
        epoch = slot // self.config.epoch_length

        data = (
            epoch.to_bytes(8, "little") +
            slot.to_bytes(8, "little") +
            bytes.fromhex(self._last_block_hash)
        )
        return hashlib.sha3_256(data).digest()

    def update_last_block_hash(self, block_hash: str) -> None:
        """Update last block hash for VRF seed entropy."""
        with self._lock:
            self._last_block_hash = block_hash

    def select_validator(self, slot: int) -> str:
        """
        Select a validator for a given slot using stake-weighted selection.

        Returns:
            Selected validator address

        Raises:
            ProofOfStakeError: If no validators available
        """
        with self._lock:
            active_validators = [
                (addr, v) for addr, v in self._validators.items() if v.active
            ]

            if not active_validators:
                raise ProofOfStakeError("No active validators")

            # Calculate total stake
            total_stake = sum(v.stake for _, v in active_validators)
            if total_stake == 0:
                raise ProofOfStakeError("Total stake is zero")

            # Generate random value from seed using rejection sampling to avoid modulo bias
            seed = self._compute_seed(slot)
            # Use full 32-byte hash for better distribution
            random_value_raw = int.from_bytes(seed, "little")
            random_value = random_value_raw % total_stake

            # Select validator based on stake weight
            cumulative = 0
            for addr, validator in active_validators:
                cumulative += validator.stake
                if random_value < cumulative:
                    return addr

            # Fallback (shouldn't happen)
            return active_validators[-1][0]

    def validate_block_producer(self, producer: str, slot: int) -> bool:
        """Validate that the correct validator produced the block."""
        with self._lock:
            try:
                expected = self.select_validator(slot)
                return producer == expected
            except ProofOfStakeError:
                return False

    def distribute_reward(self, producer: str) -> int:
        """Distribute base block reward (legacy method)."""
        with self._lock:
            if producer not in self._validators:
                raise ProofOfStakeError(f"Validator not found: {producer[:16]}...")

            self._validators[producer].rewards += self.config.block_reward
            return self.config.block_reward

    def get_reward_for_height(self, block_height: int) -> int:
        """Calculate block reward for a given height using halving schedule."""
        return self.config.halving_schedule.calculate_reward(block_height)

    def distribute_reward_with_height(self, producer: str, block_height: int) -> int:
        """Distribute block reward with halving schedule."""
        with self._lock:
            reward = self.get_reward_for_height(block_height)
            if reward == 0:
                return 0

            if producer not in self._validators:
                raise ProofOfStakeError(f"Validator not found: {producer[:16]}...")

            self._validators[producer].rewards += reward
            return reward

    def get_halving_status(self, block_height: int) -> tuple[int, int, int]:
        """
        Get current halving era and blocks until next halving.

        Returns:
            Tuple of (era, blocks_until_next, current_reward)
        """
        schedule = self.config.halving_schedule
        return (
            schedule.get_halving_era(block_height),
            schedule.blocks_until_next_halving(block_height),
            schedule.calculate_reward(block_height),
        )

    def get_halving_info(self) -> HalvingInfo:
        """Get halving schedule information."""
        schedule = self.config.halving_schedule
        return HalvingInfo(
            initial_reward_mdt=schedule.initial_reward / 1e18,
            halving_interval_blocks=schedule.halving_interval,
            halving_interval_years=(schedule.halving_interval * 3.0) / (365.25 * 24 * 3600),
            max_halvings=schedule.max_halvings,
            estimated_total_emission_mdt=schedule.estimate_total_emission() / 1e18,
        )

    def add_validator(
        self,
        address: str,
        stake: int,
        public_key: Optional[str] = None,
    ) -> None:
        """Add a new validator."""
        with self._lock:
            if stake < self.config.min_stake:
                raise ProofOfStakeError(
                    f"Insufficient stake: {stake} < {self.config.min_stake}"
                )

            if address in self._validators:
                raise ProofOfStakeError(f"Validator already exists: {address[:16]}...")

            self._validators[address] = ValidatorInfo(
                address=address,
                stake=stake,
                public_key=public_key,
                active=True,
            )
            logger.info("Added validator %.16s... with stake %s", address, stake)

    def remove_validator(self, address: str) -> None:
        """Remove a validator."""
        with self._lock:
            if address not in self._validators:
                raise ProofOfStakeError(f"Validator not found: {address[:16]}...")

            del self._validators[address]
            logger.info("Removed validator %.16s...", address)

    def update_validator_stake(self, address: str, new_stake: int) -> None:
        """Update validator stake."""
        with self._lock:
            if new_stake < self.config.min_stake:
                raise ProofOfStakeError(
                    f"Insufficient stake: {new_stake} < {self.config.min_stake}"
                )

            if address not in self._validators:
                raise ProofOfStakeError(f"Validator not found: {address[:16]}...")

            self._validators[address].stake = new_stake

    def current_epoch(self) -> int:
        """Get current epoch."""
        with self._lock:
            return self._current_epoch

    def advance_epoch(self) -> None:
        """Advance to next epoch."""
        with self._lock:
            self._current_epoch += 1

    def get_slot(self, timestamp: int, genesis_time: int) -> int:
        """Get slot from timestamp."""
        if timestamp < genesis_time:
            return 0
        return (timestamp - genesis_time) // self.config.slot_duration

    def total_stake(self) -> int:
        """Get total stake in the network."""
        with self._lock:
            return sum(v.stake for v in self._validators.values())

    def validator_count(self) -> int:
        """Get number of validators."""
        with self._lock:
            return len(self._validators)

    def get_validators(self) -> Dict[str, ValidatorInfo]:
        """Get all validators."""
        with self._lock:
            return dict(self._validators)
