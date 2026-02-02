"""
Fork Choice Rule Implementation (GHOST Algorithm).

Matches luxtensor-consensus/src/fork_choice.rs.

Implements Greedy Heaviest-Observed Sub-Tree (GHOST) algorithm
for selecting the canonical chain head during forks.

Key concepts:
- GHOST prefers the subtree with most accumulated weight/work
- Score = accumulated stake-weighted attestations
- Head = block with highest score among tips
"""

from dataclasses import dataclass, field
from typing import Dict, List, Optional, Set
from collections import deque
import threading


@dataclass
class BlockInfo:
    """
    Minimal block information for fork choice.

    Note: This is a simplified representation for SDK use.
    Full block data should be retrieved from chain storage.
    """
    hash: str           # Block hash (0x prefixed)
    parent_hash: str    # Parent block hash
    height: int         # Block height/number
    timestamp: int      # Unix timestamp
    proposer: str       # Validator who proposed the block

    def __hash__(self):
        return hash(self.hash)

    def __eq__(self, other):
        if isinstance(other, BlockInfo):
            return self.hash == other.hash
        return False


class ForkChoiceError(Exception):
    """Errors from fork choice operations."""
    pass


class ForkChoice:
    """
    Fork choice rule implementation (GHOST algorithm).

    Matches luxtensor-consensus ForkChoice struct.

    GHOST (Greedy Heaviest-Observed Sub-Tree) selects the chain
    that has the most accumulated validator support.

    Usage:
        genesis = BlockInfo(
            hash="0x...",
            parent_hash="0x" + "00" * 32,
            height=0,
            timestamp=0,
            proposer="0x..."
        )
        fc = ForkChoice(genesis)

        # Add blocks as they arrive
        fc.add_block(new_block)

        # Get current head
        head = fc.get_head()
    """

    def __init__(self, genesis: BlockInfo):
        """Create a new fork choice instance with genesis block."""
        self._blocks: Dict[str, BlockInfo] = {}
        self._children: Dict[str, List[str]] = {}  # parent_hash -> [child_hashes]
        self._scores: Dict[str, int] = {}          # hash -> score
        self._head_hash: str = genesis.hash
        self._lock = threading.RLock()

        # Initialize with genesis
        self._blocks[genesis.hash] = genesis
        self._children[genesis.hash] = []
        self._scores[genesis.hash] = 1  # Genesis has base score of 1

    def add_block(self, block: BlockInfo) -> None:
        """
        Add a new block to the fork choice.

        Args:
            block: Block to add

        Raises:
            ForkChoiceError: If block is duplicate or orphan
        """
        with self._lock:
            # Check for duplicate
            if block.hash in self._blocks:
                raise ForkChoiceError(f"Block {block.hash[:16]}... already exists")

            # Check parent exists
            if block.parent_hash not in self._blocks:
                raise ForkChoiceError(
                    f"Orphan block: parent {block.parent_hash[:16]}... not found"
                )

            # Add block
            self._blocks[block.hash] = block
            self._children[block.hash] = []

            # Add to parent's children
            if block.parent_hash not in self._children:
                self._children[block.parent_hash] = []
            self._children[block.parent_hash].append(block.hash)

            # Calculate score (parent's score + 1)
            parent_score = self._scores.get(block.parent_hash, 0)
            self._scores[block.hash] = parent_score + 1

            # Update head if this block has higher score
            self._update_head()

    def get_head(self) -> BlockInfo:
        """Get the current head block."""
        with self._lock:
            if self._head_hash not in self._blocks:
                raise ForkChoiceError("Head block not found")
            return self._blocks[self._head_hash]

    def get_head_hash(self) -> str:
        """Get the head hash."""
        with self._lock:
            return self._head_hash

    def get_block(self, hash: str) -> Optional[BlockInfo]:
        """Get a block by hash."""
        with self._lock:
            return self._blocks.get(hash)

    def get_canonical_chain(self) -> List[BlockInfo]:
        """
        Get the canonical chain from genesis to head.

        Returns:
            List of blocks from genesis to head (inclusive)
        """
        with self._lock:
            chain = []
            current_hash = self._head_hash

            while current_hash in self._blocks:
                block = self._blocks[current_hash]
                chain.append(block)

                # Stop at genesis (parent not in blocks)
                if block.parent_hash not in self._blocks:
                    break
                current_hash = block.parent_hash

            chain.reverse()  # Genesis first
            return chain

    def get_score(self, hash: str) -> Optional[int]:
        """Get the score of a block."""
        with self._lock:
            return self._scores.get(hash)

    def has_block(self, hash: str) -> bool:
        """Check if a block exists."""
        with self._lock:
            return hash in self._blocks

    @property
    def block_count(self) -> int:
        """Get the number of blocks in the fork choice."""
        with self._lock:
            return len(self._blocks)

    def _update_head(self) -> None:
        """Update the head to the block with the highest score (GHOST algorithm)."""
        best_hash = self._head_hash
        best_score = self._scores.get(self._head_hash, 0)

        for hash, score in self._scores.items():
            # Prefer higher score, or same score with later block
            if score > best_score:
                best_score = score
                best_hash = hash
            elif score == best_score and hash > best_hash:
                # Tie-break by hash (deterministic)
                best_hash = hash

        self._head_hash = best_hash

    def recompute_scores(self) -> None:
        """
        Recompute scores for all blocks (useful after reorganization).

        Uses BFS from genesis for correct topological ordering.
        """
        with self._lock:
            # Find genesis (block with no parent in our set)
            genesis_hash = None
            for hash, block in self._blocks.items():
                if block.parent_hash not in self._blocks:
                    genesis_hash = hash
                    break

            if genesis_hash is None:
                return

            # BFS from genesis
            self._scores = {genesis_hash: 1}
            queue = deque([genesis_hash])

            while queue:
                current_hash = queue.popleft()
                current_score = self._scores[current_hash]

                for child_hash in self._children.get(current_hash, []):
                    self._scores[child_hash] = current_score + 1
                    queue.append(child_hash)

            self._update_head()

    def get_blocks_at_height(self, height: int) -> List[BlockInfo]:
        """Get all blocks at a specific height."""
        with self._lock:
            return [
                block for block in self._blocks.values()
                if block.height == height
            ]

    def prune(self, keep_depth: int) -> int:
        """
        Prune old blocks (keep only canonical chain + recent forks).

        Args:
            keep_depth: Number of blocks from head to keep

        Returns:
            Number of blocks pruned
        """
        with self._lock:
            head = self.get_head()
            min_height = max(0, head.height - keep_depth)

            # Build set of canonical hashes to keep
            canonical_hashes: Set[str] = set()
            for block in self.get_canonical_chain():
                if block.height >= min_height:
                    canonical_hashes.add(block.hash)

            # Find blocks to prune
            to_prune = [
                hash for hash, block in self._blocks.items()
                if block.height < min_height and hash not in canonical_hashes
            ]

            # Prune
            for hash in to_prune:
                del self._blocks[hash]
                if hash in self._scores:
                    del self._scores[hash]
                if hash in self._children:
                    del self._children[hash]

                # Remove from parent's children list
                for parent_children in self._children.values():
                    if hash in parent_children:
                        parent_children.remove(hash)

            return len(to_prune)

    def get_fork_blocks(self) -> List[BlockInfo]:
        """
        Get all blocks that are NOT on the canonical chain (fork blocks).

        Returns:
            List of non-canonical blocks
        """
        with self._lock:
            canonical_hashes = {b.hash for b in self.get_canonical_chain()}
            return [
                block for block in self._blocks.values()
                if block.hash not in canonical_hashes
            ]

    def get_common_ancestor(self, hash1: str, hash2: str) -> Optional[BlockInfo]:
        """
        Find the common ancestor of two blocks.

        Returns:
            Common ancestor block or None if not found
        """
        with self._lock:
            # Build ancestor set for hash1
            ancestors1: Set[str] = set()
            current = hash1
            while current in self._blocks:
                ancestors1.add(current)
                block = self._blocks[current]
                current = block.parent_hash

            # Walk back from hash2 until we find a common ancestor
            current = hash2
            while current in self._blocks:
                if current in ancestors1:
                    return self._blocks[current]
                block = self._blocks[current]
                current = block.parent_hash

            return None


# Module exports
__all__ = [
    "BlockInfo",
    "ForkChoice",
    "ForkChoiceError",
]
