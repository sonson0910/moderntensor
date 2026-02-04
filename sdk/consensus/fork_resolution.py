"""
Fork Resolution - Advanced Fork Handling with Reorg Detection

Ported from luxtensor-consensus/src/fork_resolution.rs

Provides sophisticated fork handling beyond basic GHOST:
- Reorganization detection and validation
- Finality tracking to prevent deep reorgs
- Chain validation (gaps, parent links)
"""

from dataclasses import dataclass
from enum import Enum, auto
from threading import RLock
from typing import Dict, List, Optional, Set
import logging

logger = logging.getLogger(__name__)


@dataclass
class BlockInfo:
    """Block information for fork resolution."""
    hash: str
    parent_hash: str
    height: int


@dataclass
class ReorgInfo:
    """Information about a reorganization."""
    common_ancestor_height: int
    reorg_depth: int
    blocks_to_remove: List[BlockInfo]
    blocks_to_add: List[BlockInfo]


class FinalityStatus(Enum):
    """Finality status of a block."""
    FINALIZED = auto()
    NEAR_FINALIZED = auto()
    UNFINALIZED = auto()


@dataclass
class FinalityStats:
    """Statistics about finality."""
    finalized_count: int = 0
    finality_threshold: int = 32
    max_reorg_depth: int = 64


class ForkResolutionError(Exception):
    """Error during fork resolution."""
    pass


class ForkResolver:
    """
    Fork resolver with reorg detection and finality tracking.

    Thread-safe implementation using RLock.

    Usage:
        resolver = ForkResolver(finality_threshold=32, max_reorg_depth=64)

        # Detect reorganization
        reorg = resolver.detect_reorg(current_chain, new_chain)
        if reorg:
            apply_reorg(reorg.blocks_to_remove, reorg.blocks_to_add)

        # Mark blocks as finalized
        resolver.finalize_block(block_hash)
    """

    def __init__(
        self,
        finality_threshold: int = 32,
        max_reorg_depth: int = 64,
    ):
        """
        Initialize fork resolver.

        Args:
            finality_threshold: Blocks deep to consider final (default 32 ~ 6.4 min)
            max_reorg_depth: Maximum allowed reorganization depth (default 64)
        """
        self._lock = RLock()
        self._finality_threshold = finality_threshold
        self._max_reorg_depth = max_reorg_depth
        self._finalized_blocks: Set[str] = set()

    def detect_reorg(
        self,
        current_chain: List[BlockInfo],
        new_chain: List[BlockInfo],
    ) -> Optional[ReorgInfo]:
        """
        Detect if a reorganization is needed.

        Args:
            current_chain: Current canonical chain (genesis to head)
            new_chain: Proposed new chain

        Returns:
            ReorgInfo if reorg needed, None otherwise

        Raises:
            ForkResolutionError: If reorg too deep or would affect finalized blocks
        """
        with self._lock:
            if not current_chain or not new_chain:
                return None

            # Check if chains are identical
            if len(current_chain) == len(new_chain):
                same = all(
                    current_chain[i].hash == new_chain[i].hash
                    for i in range(len(current_chain))
                )
                if same:
                    return None

            # Find common ancestor
            ancestor_height = self._find_common_ancestor(current_chain, new_chain)

            if ancestor_height is None:
                return None

            current_head_height = current_chain[-1].height if current_chain else 0
            reorg_depth = current_head_height - ancestor_height

            # Check reorg depth
            if reorg_depth > self._max_reorg_depth:
                logger.warning(
                    f"Reorg depth {reorg_depth} exceeds max {self._max_reorg_depth}"
                )
                raise ForkResolutionError(f"Reorg too deep: {reorg_depth} blocks")

            # Check finalized blocks
            for block in current_chain[ancestor_height + 1:]:
                if self.is_finalized(block.hash):
                    raise ForkResolutionError("Cannot reorg finalized blocks")

            # Calculate blocks to remove and add
            blocks_to_remove = current_chain[ancestor_height + 1:]
            blocks_to_add = new_chain[ancestor_height + 1:]

            if not blocks_to_remove:
                return None

            logger.info(
                f"Reorg detected: depth={reorg_depth}, "
                f"removing {len(blocks_to_remove)} blocks, "
                f"adding {len(blocks_to_add)} blocks"
            )

            return ReorgInfo(
                common_ancestor_height=ancestor_height,
                reorg_depth=reorg_depth,
                blocks_to_remove=list(blocks_to_remove),
                blocks_to_add=list(blocks_to_add),
            )

    def _find_common_ancestor(
        self,
        chain1: List[BlockInfo],
        chain2: List[BlockInfo],
    ) -> Optional[int]:
        """Find common ancestor height between two chains."""
        chain1_hashes: Dict[str, int] = {b.hash: b.height for b in chain1}

        for block in reversed(chain2):
            if block.hash in chain1_hashes:
                return block.height

        return None

    def finalize_block(self, block_hash: str) -> None:
        """Mark a block as finalized."""
        with self._lock:
            self._finalized_blocks.add(block_hash)
            logger.debug(f"Finalized block: {block_hash[:16]}...")

    def finalize_up_to(self, blocks: List[BlockInfo], height: int) -> None:
        """Mark all blocks up to a certain height as finalized."""
        with self._lock:
            for block in blocks:
                if block.height <= height:
                    self._finalized_blocks.add(block.hash)
            logger.info(f"Finalized blocks up to height {height}")

    def is_finalized(self, block_hash: str) -> bool:
        """Check if a block is finalized."""
        with self._lock:
            return block_hash in self._finalized_blocks

    def process_finalization(self, chain: List[BlockInfo]) -> List[str]:
        """
        Process finalization based on chain depth.

        Args:
            chain: Current canonical chain

        Returns:
            List of newly finalized block hashes
        """
        with self._lock:
            newly_finalized = []

            if len(chain) > self._finality_threshold:
                finalization_point = len(chain) - self._finality_threshold

                for block in chain[:finalization_point]:
                    if not self.is_finalized(block.hash):
                        self.finalize_block(block.hash)
                        newly_finalized.append(block.hash)

            return newly_finalized

    def validate_chain(self, chain: List[BlockInfo]) -> None:
        """
        Validate chain is valid (no gaps, correct parent links).

        Raises:
            ForkResolutionError: If chain is invalid
        """
        with self._lock:
            if not chain:
                return

            for i in range(1, len(chain)):
                prev = chain[i - 1]
                current = chain[i]

                if current.height != prev.height + 1:
                    raise ForkResolutionError(
                        f"Non-sequential heights: {prev.height} → {current.height}"
                    )

                if current.parent_hash != prev.hash:
                    raise ForkResolutionError(
                        f"Invalid parent link at height {current.height}"
                    )

    def get_finality_status(
        self,
        block_hash: str,
        depth: int,
    ) -> tuple[FinalityStatus, int]:
        """
        Get finality status for a block at given depth.

        Args:
            block_hash: Block to check
            depth: Current depth from head

        Returns:
            Tuple of (status, blocks_until_final)
        """
        with self._lock:
            if self.is_finalized(block_hash):
                return FinalityStatus.FINALIZED, 0

            if depth >= self._finality_threshold:
                return FinalityStatus.NEAR_FINALIZED, 0

            blocks_until = self._finality_threshold - depth
            return FinalityStatus.UNFINALIZED, blocks_until

    def get_finality_stats(self) -> FinalityStats:
        """Get statistics about finalized blocks."""
        with self._lock:
            return FinalityStats(
                finalized_count=len(self._finalized_blocks),
                finality_threshold=self._finality_threshold,
                max_reorg_depth=self._max_reorg_depth,
            )
