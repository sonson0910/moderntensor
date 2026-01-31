use crate::error::ConsensusError;
use luxtensor_core::block::Block;
use luxtensor_core::types::Hash;
use std::collections::{HashMap, HashSet, VecDeque};
use parking_lot::RwLock;

/// Fork choice rule implementation (GHOST algorithm)
/// Greedy Heaviest-Observed Sub-Tree
pub struct ForkChoice {
    /// All known blocks indexed by hash
    blocks: RwLock<HashMap<Hash, Block>>,
    /// Block scores (higher is better)
    scores: RwLock<HashMap<Hash, u64>>,
    /// Current head of the chain
    head: RwLock<Hash>,
    /// Genesis block hash
    genesis_hash: Hash,
}

impl ForkChoice {
    /// Create a new fork choice instance with genesis block
    pub fn new(genesis: Block) -> Self {
        let genesis_hash = genesis.hash();
        let mut blocks = HashMap::new();
        let mut scores = HashMap::new();

        blocks.insert(genesis_hash, genesis);
        scores.insert(genesis_hash, 0);

        Self {
            blocks: RwLock::new(blocks),
            scores: RwLock::new(scores),
            head: RwLock::new(genesis_hash),
            genesis_hash,
        }
    }

    /// Add a new block to the fork choice
    pub fn add_block(&self, block: Block) -> Result<(), ConsensusError> {
        let block_hash = block.hash();
        let parent_hash = block.header().previous_hash;

        // Check if we already have this block
        {
            let blocks = self.blocks.read();
            if blocks.contains_key(&block_hash) {
                return Err(ConsensusError::DuplicateBlock(block_hash));
            }
        }

        // Verify parent exists
        {
            let blocks = self.blocks.read();
            if !blocks.contains_key(&parent_hash) && parent_hash != [0u8; 32] {
                return Err(ConsensusError::OrphanBlock {
                    block: block_hash,
                    parent: parent_hash,
                });
            }
        }

        // Calculate score for new block (parent score + 1)
        let score = {
            let scores = self.scores.read();
            if parent_hash == [0u8; 32] {
                1 // Genesis block
            } else {
                scores.get(&parent_hash).copied().unwrap_or(0) + 1
            }
        };

        // Add block and score
        {
            let mut blocks = self.blocks.write();
            let mut scores = self.scores.write();
            blocks.insert(block_hash, block);
            scores.insert(block_hash, score);
        }

        // Update head if necessary
        self.update_head();

        Ok(())
    }

    /// Get the current head block
    pub fn get_head(&self) -> Result<Block, ConsensusError> {
        let head_hash = *self.head.read();
        let blocks = self.blocks.read();

        blocks
            .get(&head_hash)
            .cloned()
            .ok_or(ConsensusError::BlockNotFound(head_hash))
    }

    /// Get the head hash
    pub fn get_head_hash(&self) -> Hash {
        *self.head.read()
    }

    /// Get a block by hash
    pub fn get_block(&self, hash: &Hash) -> Result<Block, ConsensusError> {
        let blocks = self.blocks.read();
        blocks
            .get(hash)
            .cloned()
            .ok_or(ConsensusError::BlockNotFound(*hash))
    }

    /// Get the canonical chain from genesis to head
    pub fn get_canonical_chain(&self) -> Vec<Block> {
        let mut chain = Vec::new();
        let blocks = self.blocks.read();
        let mut current_hash = *self.head.read();

        // Walk backwards from head to genesis
        while current_hash != [0u8; 32] {
            if let Some(block) = blocks.get(&current_hash) {
                chain.push(block.clone());
                current_hash = block.header().previous_hash;
            } else {
                break;
            }
        }

        // Reverse to get genesis -> head order
        chain.reverse();
        chain
    }

    /// Get the score of a block
    pub fn get_score(&self, hash: &Hash) -> Option<u64> {
        let scores = self.scores.read();
        scores.get(hash).copied()
    }

    /// Check if a block exists
    pub fn has_block(&self, hash: &Hash) -> bool {
        let blocks = self.blocks.read();
        blocks.contains_key(hash)
    }

    /// Get the number of blocks in the fork choice
    pub fn block_count(&self) -> usize {
        let blocks = self.blocks.read();
        blocks.len()
    }

    /// Update the head to the block with the highest score (GHOST algorithm)
    fn update_head(&self) {
        let scores = self.scores.read();

        // Find the block with the highest score
        if let Some((&new_head, _)) = scores.iter().max_by_key(|(_, &score)| score) {
            let mut head = self.head.write();
            *head = new_head;
        }
    }

    /// Recompute scores for all blocks (useful after reorganization)
    /// Uses BFS from genesis for correct topological ordering
    pub fn recompute_scores(&self) {
        let blocks = self.blocks.read();
        let mut scores = self.scores.write();

        // Clear existing scores
        scores.clear();
        scores.insert(self.genesis_hash, 0);

        // Build parent -> children index for BFS
        let mut children: HashMap<Hash, Vec<Hash>> = HashMap::new();
        for (hash, block) in blocks.iter() {
            let parent_hash = block.header().previous_hash;
            children.entry(parent_hash).or_default().push(*hash);
        }

        // BFS from genesis ensures parents are scored before children
        let mut queue = VecDeque::new();
        queue.push_back(self.genesis_hash);

        while let Some(current) = queue.pop_front() {
            let current_score = scores.get(&current).copied().unwrap_or(0);

            if let Some(child_hashes) = children.get(&current) {
                for child_hash in child_hashes {
                    scores.insert(*child_hash, current_score + 1);
                    queue.push_back(*child_hash);
                }
            }
        }

        drop(scores);
        self.update_head();
    }

    /// Get all blocks at a specific height
    pub fn get_blocks_at_height(&self, height: u64) -> Vec<Block> {
        let blocks = self.blocks.read();
        blocks
            .values()
            .filter(|block| block.height() == height)
            .cloned()
            .collect()
    }

    /// Prune old blocks (keep only canonical chain + recent forks)
    /// Optimized: Uses HashSet for O(1) lookups instead of O(n) Vec::contains
    pub fn prune(&self, keep_depth: u64) -> Result<usize, ConsensusError> {
        let head = self.get_head()?;
        let head_height = head.height();

        if head_height < keep_depth {
            return Ok(0);
        }

        let prune_before_height = head_height - keep_depth;
        let canonical_chain = self.get_canonical_chain();

        // Use HashSet for O(1) lookup (was O(n) with Vec::contains)
        let mut keep_set: HashSet<Hash> = canonical_chain
            .iter()
            .filter(|block| block.height() >= prune_before_height)
            .map(|block| block.hash())
            .collect();

        // Also keep recent fork blocks
        {
            let blocks = self.blocks.read();
            for block in blocks.values() {
                if block.height() >= prune_before_height {
                    keep_set.insert(block.hash());
                }
            }
        }

        // Remove blocks not in keep set - O(n) instead of O(n²)
        let mut blocks = self.blocks.write();
        let mut scores = self.scores.write();
        let initial_count = blocks.len();

        blocks.retain(|hash, _| keep_set.contains(hash));
        scores.retain(|hash, _| keep_set.contains(hash));

        let pruned_count = initial_count - blocks.len();
        Ok(pruned_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::block::{Block, BlockHeader};

    fn create_test_block(height: u64, previous_hash: Hash) -> Block {
        let header = BlockHeader::new(
            1,                           // version
            height,
            1000 + height,               // timestamp
            previous_hash,
            [0u8; 32],                   // state_root
            [0u8; 32],                   // txs_root
            [0u8; 32],                   // receipts_root
            [0u8; 32],                   // validator (32 bytes)
            [0u8; 64],                   // signature
            0,                           // gas_used
            1000000,                     // gas_limit
            vec![],                      // extra_data
        );
        Block::new(header, vec![])
    }

    #[test]
    fn test_fork_choice_creation() {
        let genesis = create_test_block(0, [0u8; 32]);
        let fork_choice = ForkChoice::new(genesis);

        assert_eq!(fork_choice.block_count(), 1);
        assert!(fork_choice.get_head().is_ok());
    }

    #[test]
    fn test_add_block() {
        let genesis = create_test_block(0, [0u8; 32]);
        let genesis_hash = genesis.hash();
        let fork_choice = ForkChoice::new(genesis);

        let block1 = create_test_block(1, genesis_hash);
        assert!(fork_choice.add_block(block1).is_ok());
        assert_eq!(fork_choice.block_count(), 2);
    }

    #[test]
    fn test_add_duplicate_block() {
        let genesis = create_test_block(0, [0u8; 32]);
        let genesis_hash = genesis.hash();
        let fork_choice = ForkChoice::new(genesis);

        let block1 = create_test_block(1, genesis_hash);
        let block1_copy = block1.clone();

        assert!(fork_choice.add_block(block1).is_ok());
        assert!(fork_choice.add_block(block1_copy).is_err());
    }

    #[test]
    fn test_add_orphan_block() {
        let genesis = create_test_block(0, [0u8; 32]);
        let fork_choice = ForkChoice::new(genesis);

        // Block with non-existent parent
        let orphan = create_test_block(1, [1u8; 32]);
        assert!(fork_choice.add_block(orphan).is_err());
    }

    #[test]
    fn test_get_canonical_chain() {
        let genesis = create_test_block(0, [0u8; 32]);
        let genesis_hash = genesis.hash();
        let fork_choice = ForkChoice::new(genesis);

        // Add 3 blocks in sequence
        let block1 = create_test_block(1, genesis_hash);
        let block1_hash = block1.hash();
        fork_choice.add_block(block1).unwrap();

        let block2 = create_test_block(2, block1_hash);
        let block2_hash = block2.hash();
        fork_choice.add_block(block2).unwrap();

        let block3 = create_test_block(3, block2_hash);
        fork_choice.add_block(block3).unwrap();

        let chain = fork_choice.get_canonical_chain();
        assert_eq!(chain.len(), 4); // genesis + 3 blocks
        assert_eq!(chain[0].height(), 0);
        assert_eq!(chain[3].height(), 3);
    }

    #[test]
    fn test_fork_selection() {
        let genesis = create_test_block(0, [0u8; 32]);
        let genesis_hash = genesis.hash();
        let fork_choice = ForkChoice::new(genesis);

        // Create main chain: genesis -> block1 -> block2
        let block1 = create_test_block(1, genesis_hash);
        let block1_hash = block1.hash();
        fork_choice.add_block(block1).unwrap();

        let block2 = create_test_block(2, block1_hash);
        fork_choice.add_block(block2).unwrap();

        // Create fork: genesis -> block1_alt -> block2_alt -> block3_alt
        let mut block1_alt = create_test_block(1, genesis_hash);
        // Modify to make it different
        block1_alt.header_mut().timestamp += 1;
        let block1_alt_hash = block1_alt.hash();
        fork_choice.add_block(block1_alt).unwrap();

        let block2_alt = create_test_block(2, block1_alt_hash);
        let block2_alt_hash = block2_alt.hash();
        fork_choice.add_block(block2_alt).unwrap();

        let block3_alt = create_test_block(3, block2_alt_hash);
        fork_choice.add_block(block3_alt).unwrap();

        // The longer chain (3 blocks) should be selected
        let head = fork_choice.get_head().unwrap();
        assert_eq!(head.height(), 3);
    }

    #[test]
    fn test_get_blocks_at_height() {
        let genesis = create_test_block(0, [0u8; 32]);
        let genesis_hash = genesis.hash();
        let fork_choice = ForkChoice::new(genesis);

        // Add two blocks at height 1 (fork)
        let block1a = create_test_block(1, genesis_hash);
        fork_choice.add_block(block1a).unwrap();

        let mut block1b = create_test_block(1, genesis_hash);
        block1b.header_mut().timestamp += 1;
        fork_choice.add_block(block1b).unwrap();

        let blocks_at_height_1 = fork_choice.get_blocks_at_height(1);
        assert_eq!(blocks_at_height_1.len(), 2);
    }

    #[test]
    fn test_has_block() {
        let genesis = create_test_block(0, [0u8; 32]);
        let genesis_hash = genesis.hash();
        let fork_choice = ForkChoice::new(genesis);

        assert!(fork_choice.has_block(&genesis_hash));
        assert!(!fork_choice.has_block(&[1u8; 32]));
    }
}
