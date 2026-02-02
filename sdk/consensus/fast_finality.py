"""
Fast Finality Mechanism with BFT-style Guarantees.

Matches luxtensor-consensus/src/fast_finality.rs.

Provides immediate finality for blocks with sufficient validator signatures.
Uses stake-weighted voting to determine when a block is considered final.

Key concepts:
- Finality threshold (default 67%): stake percentage needed for finality
- Once finalized, blocks cannot be reverted
- Reduces confirmation time vs probabilistic finality
"""

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set
import threading
import time


@dataclass
class ValidatorInfo:
    """Validator information for finality tracking."""
    address: str
    stake: int
    is_active: bool = True


@dataclass
class BlockSignatures:
    """
    Signatures collected for a block.

    Matches luxtensor-consensus BlockSignatures.
    """
    block_hash: str
    signers: Set[str] = field(default_factory=set)
    total_stake_signed: int = 0
    stake_percent: int = 0           # 0-100
    is_finalized: bool = False
    finalized_at: Optional[int] = None  # timestamp


@dataclass
class FastFinalityStats:
    """
    Fast finality statistics.

    Matches luxtensor-consensus FastFinalityStats.
    """
    total_blocks_tracked: int = 0
    finalized_blocks: int = 0
    pending_blocks: int = 0
    total_signatures: int = 0
    finality_threshold_percent: int = 67
    total_validator_stake: int = 0

    def to_dict(self) -> dict:
        return {
            "total_blocks_tracked": self.total_blocks_tracked,
            "finalized_blocks": self.finalized_blocks,
            "pending_blocks": self.pending_blocks,
            "total_signatures": self.total_signatures,
            "finality_threshold_percent": self.finality_threshold_percent,
            "total_validator_stake": str(self.total_validator_stake),
            "finalization_rate": (
                self.finalized_blocks / self.total_blocks_tracked * 100
                if self.total_blocks_tracked > 0 else 0
            ),
        }


class FastFinalityError(Exception):
    """Errors from fast finality operations."""
    pass


class FastFinality:
    """
    Fast finality manager using validator signatures.

    Matches luxtensor-consensus FastFinality struct.

    Usage:
        validators = {
            "0x...": ValidatorInfo(address="0x...", stake=1000),
            "0x...": ValidatorInfo(address="0x...", stake=2000),
        }
        ff = FastFinality(
            finality_threshold_percent=67,
            validators=validators,
        )

        # Track signatures
        is_final = ff.add_signature(block_hash, validator_address)

        # Check finality
        if ff.is_finalized(block_hash):
            print("Block is final!")
    """

    def __init__(
        self,
        finality_threshold_percent: int = 67,
        validators: Optional[Dict[str, ValidatorInfo]] = None,
    ):
        """
        Create a new fast finality instance.

        Args:
            finality_threshold_percent: Stake percentage needed for finality (1-100)
            validators: Dict of validator address -> ValidatorInfo

        Raises:
            FastFinalityError: If threshold is invalid
        """
        if not 1 <= finality_threshold_percent <= 100:
            raise FastFinalityError(
                f"Finality threshold must be 1-100, got {finality_threshold_percent}"
            )

        self._threshold = finality_threshold_percent
        self._validators: Dict[str, ValidatorInfo] = validators or {}
        self._signatures: Dict[str, BlockSignatures] = {}  # block_hash -> signatures
        self._finalized_blocks: Set[str] = set()
        self._lock = threading.RLock()

    @property
    def total_stake(self) -> int:
        """Get total stake from active validators."""
        return sum(
            v.stake for v in self._validators.values()
            if v.is_active
        )

    def add_signature(
        self,
        block_hash: str,
        validator: str,
    ) -> bool:
        """
        Add a validator signature for a block.

        Args:
            block_hash: Block hash to sign
            validator: Validator address

        Returns:
            True if block became finalized with this signature

        Raises:
            FastFinalityError: If validator is not in active set
        """
        with self._lock:
            validator = validator.lower()

            # Validate the signer
            if validator not in self._validators:
                raise FastFinalityError(f"Unknown validator: {validator}")

            validator_info = self._validators[validator]
            if not validator_info.is_active:
                raise FastFinalityError(f"Validator {validator} is not active")

            # Initialize signatures if needed
            if block_hash not in self._signatures:
                self._signatures[block_hash] = BlockSignatures(block_hash=block_hash)

            sigs = self._signatures[block_hash]

            # Check if already finalized
            if sigs.is_finalized:
                return False

            # Check for duplicate signature
            if validator in sigs.signers:
                return False  # Already signed

            # Add signature
            sigs.signers.add(validator)
            sigs.total_stake_signed += validator_info.stake

            # Calculate stake percent
            total_stake = self.total_stake
            if total_stake > 0:
                sigs.stake_percent = (sigs.total_stake_signed * 100) // total_stake

            # Check for finality
            if sigs.stake_percent >= self._threshold:
                sigs.is_finalized = True
                sigs.finalized_at = int(time.time())
                self._finalized_blocks.add(block_hash)
                return True

            return False

    def is_finalized(self, block_hash: str) -> bool:
        """Check if a block has reached finality."""
        with self._lock:
            return block_hash in self._finalized_blocks

    def get_finality_progress(self, block_hash: str) -> Optional[int]:
        """
        Get finality progress for a block (percentage of stake signed).

        Returns:
            Stake percentage (0-100) or None if block not tracked
        """
        with self._lock:
            sigs = self._signatures.get(block_hash)
            return sigs.stake_percent if sigs else None

    def get_signer_count(self, block_hash: str) -> int:
        """Get number of validators who signed a block."""
        with self._lock:
            sigs = self._signatures.get(block_hash)
            return len(sigs.signers) if sigs else 0

    def get_signers(self, block_hash: str) -> Optional[List[str]]:
        """Get list of validators who signed a block."""
        with self._lock:
            sigs = self._signatures.get(block_hash)
            return list(sigs.signers) if sigs else None

    def prune_old_signatures(self, keep_blocks: List[str]) -> int:
        """
        Clear old signatures for blocks that are no longer needed.

        Args:
            keep_blocks: List of block hashes to keep

        Returns:
            Number of blocks pruned
        """
        with self._lock:
            keep_set = set(keep_blocks)
            to_remove = [
                hash for hash in self._signatures.keys()
                if hash not in keep_set
            ]

            for hash in to_remove:
                del self._signatures[hash]
                self._finalized_blocks.discard(hash)

            return len(to_remove)

    def get_stats(self) -> FastFinalityStats:
        """Get statistics about fast finality."""
        with self._lock:
            total_signatures = sum(
                len(sigs.signers) for sigs in self._signatures.values()
            )

            return FastFinalityStats(
                total_blocks_tracked=len(self._signatures),
                finalized_blocks=len(self._finalized_blocks),
                pending_blocks=len(self._signatures) - len(self._finalized_blocks),
                total_signatures=total_signatures,
                finality_threshold_percent=self._threshold,
                total_validator_stake=self.total_stake,
            )

    def update_validator_set(self, validators: Dict[str, ValidatorInfo]) -> None:
        """
        Update validator set.

        Note: This may affect finality calculations for pending blocks.
        """
        with self._lock:
            self._validators = validators

            # Recalculate stake percentages for non-finalized blocks
            total_stake = self.total_stake
            if total_stake == 0:
                return

            for sigs in self._signatures.values():
                if sigs.is_finalized:
                    continue

                # Recalculate total stake signed
                sigs.total_stake_signed = sum(
                    self._validators[addr].stake
                    for addr in sigs.signers
                    if addr in self._validators and self._validators[addr].is_active
                )
                sigs.stake_percent = (sigs.total_stake_signed * 100) // total_stake

                # Check for new finality
                if sigs.stake_percent >= self._threshold:
                    sigs.is_finalized = True
                    sigs.finalized_at = int(time.time())
                    self._finalized_blocks.add(sigs.block_hash)

    def get_block_signatures(self, block_hash: str) -> Optional[BlockSignatures]:
        """Get full signature info for a block."""
        with self._lock:
            sigs = self._signatures.get(block_hash)
            if sigs:
                return BlockSignatures(
                    block_hash=sigs.block_hash,
                    signers=sigs.signers.copy(),
                    total_stake_signed=sigs.total_stake_signed,
                    stake_percent=sigs.stake_percent,
                    is_finalized=sigs.is_finalized,
                    finalized_at=sigs.finalized_at,
                )
            return None

    def get_pending_blocks(self) -> List[str]:
        """Get list of blocks that are not yet finalized."""
        with self._lock:
            return [
                hash for hash in self._signatures.keys()
                if hash not in self._finalized_blocks
            ]

    def get_finalized_blocks(self) -> List[str]:
        """Get list of finalized block hashes."""
        with self._lock:
            return list(self._finalized_blocks)


# Module exports
__all__ = [
    "ValidatorInfo",
    "BlockSignatures",
    "FastFinalityStats",
    "FastFinality",
    "FastFinalityError",
]
