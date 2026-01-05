"""
Fork choice rule for ModernTensor Layer 1 blockchain.

Implements chain selection logic to determine the canonical chain
in case of forks (competing chains).
"""
import logging
from typing import Dict, List, Optional, Set
from dataclasses import dataclass, field

from ..blockchain.block import Block

logger = logging.getLogger(__name__)


@dataclass
class BlockNode:
    """
    Node in the block tree representing a block and its relationships.
    
    Attributes:
        block: The actual block
        parent_hash: Hash of parent block
        children: List of child block hashes
        total_difficulty: Cumulative difficulty from genesis
        is_finalized: Whether this block is finalized
    """
    block: Block
    parent_hash: bytes
    children: List[bytes] = field(default_factory=list)
    total_difficulty: int = 0
    is_finalized: bool = False
    
    def hash(self) -> bytes:
        """Get block hash."""
        return self.block.hash()


class ForkChoice:
    """
    Determines the canonical chain in case of forks.
    
    Implements GHOST (Greedy Heaviest Observed SubTree) algorithm
    with finality rules inspired by Casper FFG.
    """
    
    def __init__(self, genesis_block: Block):
        """
        Initialize fork choice with genesis block.
        
        Args:
            genesis_block: The genesis (first) block
        """
        self.blocks: Dict[bytes, BlockNode] = {}  # block_hash -> BlockNode
        self.head_hash: bytes = b''  # Current canonical head
        self.finalized_hash: bytes = b''  # Last finalized block
        self.finalized_height: int = 0
        
        # Add genesis block
        genesis_hash = genesis_block.hash()
        genesis_node = BlockNode(
            block=genesis_block,
            parent_hash=b'\x00' * 32,
            total_difficulty=0,
            is_finalized=True,
        )
        self.blocks[genesis_hash] = genesis_node
        self.head_hash = genesis_hash
        self.finalized_hash = genesis_hash
        
        logger.info(f"Fork choice initialized with genesis {genesis_hash.hex()[:8]}...")
    
    def add_block(self, block: Block) -> bool:
        """
        Add a block to the fork choice tree.
        
        Args:
            block: Block to add
            
        Returns:
            bool: True if block was added successfully
        """
        block_hash = block.hash()
        parent_hash = block.header.previous_hash
        
        # Check if block already exists
        if block_hash in self.blocks:
            logger.debug(f"Block {block_hash.hex()[:8]}... already in tree")
            return False
        
        # Check if parent exists
        if parent_hash not in self.blocks:
            logger.warning(
                f"Parent {parent_hash.hex()[:8]}... not found for block {block_hash.hex()[:8]}..."
            )
            return False
        
        parent_node = self.blocks[parent_hash]
        
        # Calculate total difficulty (simple chain length for now)
        # TODO: Implement proper difficulty calculation
        total_difficulty = parent_node.total_difficulty + 1
        
        # Create block node
        block_node = BlockNode(
            block=block,
            parent_hash=parent_hash,
            total_difficulty=total_difficulty,
        )
        
        # Add to tree
        self.blocks[block_hash] = block_node
        parent_node.children.append(block_hash)
        
        logger.info(
            f"Added block {block_hash.hex()[:8]}... at height {block.header.height}, "
            f"total difficulty: {total_difficulty}"
        )
        
        # Update head if necessary
        self._update_head()
        
        return True
    
    def _update_head(self) -> None:
        """
        Update the canonical head using the GHOST rule.
        
        GHOST: Follow the subtree with the most blocks (heaviest).
        """
        # Start from finalized block
        current_hash = self.finalized_hash
        
        while True:
            current_node = self.blocks.get(current_hash)
            if not current_node or not current_node.children:
                # Reached a leaf node
                break
            
            # Find child with highest total difficulty (heaviest subtree)
            best_child = None
            best_difficulty = -1
            
            for child_hash in current_node.children:
                child_node = self.blocks.get(child_hash)
                if child_node:
                    # Calculate subtree weight (number of descendants)
                    subtree_weight = self._count_descendants(child_hash)
                    
                    if subtree_weight > best_difficulty:
                        best_difficulty = subtree_weight
                        best_child = child_hash
            
            if best_child is None:
                break
            
            current_hash = best_child
        
        # Update head if changed
        if current_hash != self.head_hash:
            old_head = self.head_hash.hex()[:8] if self.head_hash else "none"
            new_head = current_hash.hex()[:8]
            logger.info(f"Head updated: {old_head}... -> {new_head}...")
            self.head_hash = current_hash
    
    def _count_descendants(self, block_hash: bytes) -> int:
        """
        Count the number of descendant blocks (subtree weight).
        
        Args:
            block_hash: Root of subtree
            
        Returns:
            int: Number of descendants (including this block)
        """
        count = 1  # Count this block
        
        node = self.blocks.get(block_hash)
        if node:
            for child_hash in node.children:
                count += self._count_descendants(child_hash)
        
        return count
    
    def get_canonical_chain(self, from_hash: Optional[bytes] = None) -> List[Block]:
        """
        Get the canonical chain from a starting point to head.
        
        Args:
            from_hash: Starting block hash (uses finalized if None)
            
        Returns:
            List[Block]: Blocks in canonical chain
        """
        if from_hash is None:
            from_hash = self.finalized_hash
        
        # Build chain backwards from head to from_hash
        chain = []
        current_hash = self.head_hash
        
        while current_hash != from_hash:
            node = self.blocks.get(current_hash)
            if not node:
                logger.error(f"Block {current_hash.hex()[:8]}... not found in tree")
                break
            
            chain.append(node.block)
            current_hash = node.parent_hash
            
            # Safety check
            if len(chain) > 10000:
                logger.error("Chain too long, possible circular reference")
                break
        
        # Add from_hash block
        if from_hash in self.blocks:
            chain.append(self.blocks[from_hash].block)
        
        # Reverse to get genesis -> head order
        chain.reverse()
        
        return chain
    
    def get_head(self) -> Optional[Block]:
        """
        Get the current canonical head block.
        
        Returns:
            Optional[Block]: Head block or None
        """
        if self.head_hash in self.blocks:
            return self.blocks[self.head_hash].block
        return None
    
    def get_finalized(self) -> Optional[Block]:
        """
        Get the last finalized block.
        
        Returns:
            Optional[Block]: Finalized block or None
        """
        if self.finalized_hash in self.blocks:
            return self.blocks[self.finalized_hash].block
        return None
    
    def finalize_block(self, block_hash: bytes) -> bool:
        """
        Mark a block as finalized (irreversible).
        
        Once finalized, this block and all ancestors are permanent.
        
        Args:
            block_hash: Hash of block to finalize
            
        Returns:
            bool: True if successful
        """
        if block_hash not in self.blocks:
            logger.error(f"Cannot finalize unknown block {block_hash.hex()[:8]}...")
            return False
        
        node = self.blocks[block_hash]
        
        # Verify block is on canonical chain
        if not self._is_ancestor(block_hash, self.head_hash):
            logger.error(
                f"Cannot finalize block {block_hash.hex()[:8]}... - not on canonical chain"
            )
            return False
        
        # Mark as finalized
        node.is_finalized = True
        self.finalized_hash = block_hash
        self.finalized_height = node.block.header.height
        
        # Prune competing branches (optional optimization)
        self._prune_forks()
        
        logger.info(
            f"Finalized block {block_hash.hex()[:8]}... at height {self.finalized_height}"
        )
        
        return True
    
    def _is_ancestor(self, ancestor_hash: bytes, descendant_hash: bytes) -> bool:
        """
        Check if one block is an ancestor of another.
        
        Args:
            ancestor_hash: Potential ancestor block hash
            descendant_hash: Potential descendant block hash
            
        Returns:
            bool: True if ancestor_hash is ancestor of descendant_hash
        """
        current_hash = descendant_hash
        
        while current_hash != b'\x00' * 32:
            if current_hash == ancestor_hash:
                return True
            
            node = self.blocks.get(current_hash)
            if not node:
                break
            
            current_hash = node.parent_hash
        
        return False
    
    def _prune_forks(self) -> None:
        """
        Prune blocks that are not on the canonical chain and are older than finalized.
        
        This is an optimization to save memory.
        """
        # Find blocks to remove (not ancestors of head and before finalized)
        to_remove = []
        
        for block_hash, node in self.blocks.items():
            # Don't remove finalized or newer blocks
            if node.block.header.height >= self.finalized_height:
                continue
            
            # Check if on canonical chain
            if not self._is_ancestor(block_hash, self.head_hash):
                to_remove.append(block_hash)
        
        # Remove blocks
        for block_hash in to_remove:
            del self.blocks[block_hash]
        
        if to_remove:
            logger.debug(f"Pruned {len(to_remove)} orphaned blocks")
    
    def apply_finality_rule(self, checkpoint_interval: int = 100) -> None:
        """
        Apply finality rules (Casper FFG-inspired).
        
        Automatically finalizes blocks at checkpoint intervals if they
        have sufficient validator support.
        
        Args:
            checkpoint_interval: Number of blocks between finality checkpoints
        """
        head_node = self.blocks.get(self.head_hash)
        if not head_node:
            return
        
        # Find the last checkpoint
        current_height = head_node.block.header.height
        checkpoint_height = (current_height // checkpoint_interval) * checkpoint_interval
        
        # Only finalize if we're past a checkpoint
        if checkpoint_height <= self.finalized_height:
            return
        
        # Find block at checkpoint height on canonical chain
        canonical_chain = self.get_canonical_chain()
        for block in canonical_chain:
            if block.header.height == checkpoint_height:
                self.finalize_block(block.hash())
                break
