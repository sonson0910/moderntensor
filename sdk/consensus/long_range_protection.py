"""
Long-Range Attack Protection

Ported from luxtensor-consensus/src/long_range_protection.rs

Implements weak subjectivity checkpoints and finality-based pruning
to prevent long-range attacks in Proof-of-Stake systems.
"""

from dataclasses import dataclass
from threading import RLock
from typing import List, Optional
import time
import logging

logger = logging.getLogger(__name__)


@dataclass
class Checkpoint:
    """Checkpoint data for weak subjectivity."""
    block_hash: str
    height: int
    epoch: int
    state_root: Optional[str] = None
    timestamp: int = 0


@dataclass
class LongRangeConfig:
    """Long-range attack protection configuration."""
    # Weak subjectivity period in blocks (~2 weeks at 3s)
    weak_subjectivity_period: int = 403_200
    # Checkpoint interval (blocks between checkpoints)
    checkpoint_interval: int = 100
    # Maximum reorg depth allowed
    max_reorg_depth: int = 1000
    # Minimum finality confirmations
    min_finality_confirmations: int = 32
    # Require recent checkpoint for sync from scratch
    require_recent_checkpoint: bool = True
    # Maximum checkpoint age in seconds (7 days)
    max_checkpoint_age_secs: int = 604_800


@dataclass
class CheckpointStatus:
    """Status of checkpoint system for monitoring."""
    total_checkpoints: int
    latest_checkpoint: Optional[Checkpoint]
    finalized_height: int
    weak_subjectivity_period: int


class LongRangeProtectionError(Exception):
    """Error during long-range protection operation."""
    pass


class LongRangeProtection:
    """
    Long-range attack prevention manager.

    Implements weak subjectivity to prevent attacks where an attacker
    forks from a very old block with acquired old validator keys.

    Thread-safe implementation using RLock.

    Usage:
        protection = LongRangeProtection("genesis_hash", config)

        # Check if reorg is allowed
        if protection.is_reorg_allowed(reorg_depth):
            apply_reorg()

        # Update finalized block
        protection.update_finalized(block_hash, block_height)
    """

    def __init__(
        self,
        genesis_hash: str,
        config: Optional[LongRangeConfig] = None,
    ):
        self._lock = RLock()
        self.config = config or LongRangeConfig()

        # Genesis checkpoint
        genesis_checkpoint = Checkpoint(
            block_hash=genesis_hash,
            height=0,
            epoch=0,
            state_root=None,
            timestamp=0,
        )

        self._checkpoints: List[Checkpoint] = [genesis_checkpoint]
        self._finalized_hash: str = genesis_hash
        self._finalized_height: int = 0

        logger.info(f"LongRangeProtection initialized with genesis {genesis_hash[:16]}...")

    def is_within_weak_subjectivity(self, block_height: int) -> bool:
        """Check if a block is within the weak subjectivity period."""
        with self._lock:
            min_height = max(0, self._finalized_height - self.config.weak_subjectivity_period)
            return block_height >= min_height

    def is_reorg_allowed(self, reorg_depth: int) -> bool:
        """Check if a reorg is allowed (within max depth)."""
        return reorg_depth <= self.config.max_reorg_depth

    def add_checkpoint(self, checkpoint: Checkpoint) -> None:
        """
        Add a new checkpoint.

        Raises:
            LongRangeProtectionError: If checkpoint height <= last
        """
        with self._lock:
            if self._checkpoints:
                last = self._checkpoints[-1]
                if checkpoint.height <= last.height:
                    raise LongRangeProtectionError(
                        "Checkpoint height must be greater than last"
                    )

            self._checkpoints.append(checkpoint)
            logger.info(f"Added checkpoint at height {checkpoint.height}")

    def update_finalized(self, block_hash: str, height: int) -> None:
        """Update finalized block, auto-create checkpoint if at interval."""
        with self._lock:
            self._finalized_hash = block_hash
            self._finalized_height = height

            # Create checkpoint if at interval
            if height > 0 and height % self.config.checkpoint_interval == 0:
                checkpoint = Checkpoint(
                    block_hash=block_hash,
                    height=height,
                    epoch=height // 100,  # Assume 100 blocks per epoch
                    state_root=None,
                    timestamp=int(time.time()),
                )
                try:
                    self.add_checkpoint(checkpoint)
                except LongRangeProtectionError:
                    pass  # Checkpoint already exists

    def get_latest_checkpoint(self) -> Optional[Checkpoint]:
        """Get the most recent checkpoint."""
        with self._lock:
            return self._checkpoints[-1] if self._checkpoints else None

    def validate_against_checkpoints(self, block_hash: str, height: int) -> bool:
        """
        Validate a block against checkpoints.

        Returns True if block is valid, False if conflicts with checkpoint.
        """
        with self._lock:
            for cp in self._checkpoints:
                if cp.height == height:
                    return cp.block_hash == block_hash
            return True  # Not at checkpoint height

    def is_finalized(self, block_height: int) -> bool:
        """Check if block is finalized (cannot be reverted)."""
        with self._lock:
            return block_height <= self._finalized_height

    def finalized_height(self) -> int:
        """Get finalized height."""
        with self._lock:
            return self._finalized_height

    def get_checkpoints(self) -> List[Checkpoint]:
        """Get all checkpoints."""
        with self._lock:
            return list(self._checkpoints)

    def prune_old_checkpoints(self, keep_count: int) -> int:
        """
        Prune old checkpoints, keeping only recent ones.

        Returns number of checkpoints removed.
        """
        with self._lock:
            if len(self._checkpoints) <= keep_count:
                return 0

            remove_count = len(self._checkpoints) - keep_count
            self._checkpoints = self._checkpoints[remove_count:]
            return remove_count

    def can_sync_from_scratch(self) -> tuple[bool, Optional[str]]:
        """
        Check if sync from scratch is allowed.

        Returns:
            Tuple of (allowed, error_message)
        """
        with self._lock:
            if not self.config.require_recent_checkpoint:
                return True, None

            if not self._checkpoints:
                return False, "No checkpoints available"

            latest = self._checkpoints[-1]
            if latest.height == 0:
                return False, "No recent checkpoint, need trusted checkpoint for initial sync"

            now = int(time.time())
            age = now - latest.timestamp
            if age > self.config.max_checkpoint_age_secs:
                return False, "Latest checkpoint is too old for safe initial sync"

            return True, None

    def validate_external_checkpoint(self, checkpoint: Checkpoint) -> tuple[bool, Optional[str]]:
        """
        Validate a checkpoint from external source.

        Returns:
            Tuple of (valid, error_message)
        """
        with self._lock:
            # Check height is reasonable
            if checkpoint.height < self._finalized_height:
                return False, "External checkpoint is below finalized height"

            # Check timestamp not in future
            now = int(time.time())
            if checkpoint.timestamp > now + 60:
                return False, "Checkpoint timestamp is in the future"

            # Check block hash is not empty
            if not checkpoint.block_hash or checkpoint.block_hash == "0" * 64:
                return False, "Invalid checkpoint: empty block hash"

            return True, None

    def get_checkpoint_status(self) -> CheckpointStatus:
        """Get checkpoint status for monitoring."""
        with self._lock:
            return CheckpointStatus(
                total_checkpoints=len(self._checkpoints),
                latest_checkpoint=self._checkpoints[-1] if self._checkpoints else None,
                finalized_height=self._finalized_height,
                weak_subjectivity_period=self.config.weak_subjectivity_period,
            )
