// Advanced fork resolution with reorg detection and finality
// Provides sophisticated fork handling beyond basic GHOST

use crate::error::ConsensusError;
use luxtensor_core::block::Block;
use luxtensor_core::types::Hash;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, warn};

/// Fork resolution manager with reorg detection
pub struct ForkResolver {
    /// Finality threshold (blocks deep are considered final)
    finality_threshold: u64,
    /// Maximum reorg depth allowed
    max_reorg_depth: u64,
    /// Finalized blocks (cannot be reorganized)
    finalized_blocks: HashSet<Hash>,
}

impl ForkResolver {
    /// Create a new fork resolver
    pub fn new(finality_threshold: u64, max_reorg_depth: u64) -> Self {
        Self {
            finality_threshold,
            max_reorg_depth,
            finalized_blocks: HashSet::new(),
        }
    }

    /// Detect if a reorganization is needed
    pub fn detect_reorg(
        &self,
        current_chain: &[Block],
        new_chain: &[Block],
    ) -> Result<Option<ReorgInfo>, ConsensusError> {
        // If chains are identical or new chain is same as current, no reorg needed
        if current_chain.is_empty() || new_chain.is_empty() {
            return Ok(None);
        }

        // Check if the chains are already the same
        if current_chain.len() == new_chain.len() {
            let mut same = true;
            for i in 0..current_chain.len() {
                if current_chain[i].hash() != new_chain[i].hash() {
                    same = false;
                    break;
                }
            }
            if same {
                return Ok(None);
            }
        }

        // Find common ancestor
        let common_ancestor = self.find_common_ancestor(current_chain, new_chain)?;

        if let Some(ancestor_height) = common_ancestor {
            let current_head_height = current_chain.last().map(|b| b.height()).unwrap_or(0);
            let reorg_depth = current_head_height - ancestor_height;

            // Check if reorg is allowed
            if reorg_depth > self.max_reorg_depth {
                warn!(
                    "Reorg depth {} exceeds maximum {}",
                    reorg_depth, self.max_reorg_depth
                );
                return Err(ConsensusError::ForkChoice(format!(
                    "Reorg too deep: {} blocks",
                    reorg_depth
                )));
            }

            // Check if any finalized blocks would be affected
            for block in &current_chain[(ancestor_height as usize + 1)..] {
                if self.is_finalized(&block.hash()) {
                    return Err(ConsensusError::ForkChoice(
                        "Cannot reorg finalized blocks".to_string(),
                    ));
                }
            }

            // Calculate blocks to remove and add
            let blocks_to_remove: Vec<Block> = current_chain
                [(ancestor_height as usize + 1)..]
                .to_vec();
            let blocks_to_add: Vec<Block> = new_chain[(ancestor_height as usize + 1)..].to_vec();

            // If nothing to remove or add, no reorg needed
            if blocks_to_remove.is_empty() && blocks_to_add.is_empty() {
                return Ok(None);
            }

            info!(
                "Reorg detected: depth={}, removing {} blocks, adding {} blocks",
                reorg_depth,
                blocks_to_remove.len(),
                blocks_to_add.len()
            );

            Ok(Some(ReorgInfo {
                common_ancestor_height: ancestor_height,
                reorg_depth,
                blocks_to_remove,
                blocks_to_add,
            }))
        } else {
            // No common ancestor - chains are completely different
            Ok(None)
        }
    }

    /// Find common ancestor between two chains
    fn find_common_ancestor(
        &self,
        chain1: &[Block],
        chain2: &[Block],
    ) -> Result<Option<u64>, ConsensusError> {
        let chain1_hashes: HashMap<Hash, u64> = chain1
            .iter()
            .map(|b| (b.hash(), b.height()))
            .collect();

        // Iterate through chain2 from newest to oldest
        for block in chain2.iter().rev() {
            if chain1_hashes.contains_key(&block.hash()) {
                return Ok(Some(block.height()));
            }
        }

        Ok(None)
    }

    /// Mark a block as finalized
    pub fn finalize_block(&mut self, block_hash: Hash) {
        self.finalized_blocks.insert(block_hash);
        debug!("Finalized block: {:?}", hex::encode(&block_hash));
    }

    /// Mark blocks up to a certain height as finalized
    pub fn finalize_up_to(&mut self, blocks: &[Block], height: u64) {
        for block in blocks {
            if block.height() <= height {
                self.finalized_blocks.insert(block.hash());
            }
        }
        info!("Finalized blocks up to height {}", height);
    }

    /// Check if a block is finalized
    pub fn is_finalized(&self, block_hash: &Hash) -> bool {
        self.finalized_blocks.contains(block_hash)
    }

    /// Process finalization based on chain depth
    pub fn process_finalization(&mut self, chain: &[Block]) -> Vec<Hash> {
        let mut newly_finalized = Vec::new();

        if chain.len() > self.finality_threshold as usize {
            let finalization_point = chain.len() - self.finality_threshold as usize;

            for block in &chain[..finalization_point] {
                let block_hash = block.hash();
                if !self.is_finalized(&block_hash) {
                    self.finalize_block(block_hash);
                    newly_finalized.push(block_hash);
                }
            }
        }

        newly_finalized
    }

    /// Validate a chain is valid (no gaps, correct parent links)
    pub fn validate_chain(&self, chain: &[Block]) -> Result<(), ConsensusError> {
        if chain.is_empty() {
            return Ok(());
        }

        // Check sequential heights
        for i in 1..chain.len() {
            let prev = &chain[i - 1];
            let current = &chain[i];

            if current.height() != prev.height() + 1 {
                return Err(ConsensusError::ForkChoice(format!(
                    "Non-sequential heights: {} -> {}",
                    prev.height(),
                    current.height()
                )));
            }

            if current.header().previous_hash != prev.hash() {
                return Err(ConsensusError::ForkChoice(format!(
                    "Invalid parent link at height {}",
                    current.height()
                )));
            }
        }

        Ok(())
    }

    /// Get finality status for a block at given depth
    pub fn get_finality_status(&self, block_hash: &Hash, depth: u64) -> FinalityStatus {
        if self.is_finalized(block_hash) {
            FinalityStatus::Finalized
        } else if depth >= self.finality_threshold {
            FinalityStatus::NearFinalized {
                blocks_until_final: 0,
            }
        } else {
            FinalityStatus::Unfinalized {
                blocks_until_final: self.finality_threshold - depth,
            }
        }
    }

    /// Get statistics about finalized blocks
    pub fn get_finality_stats(&self) -> FinalityStats {
        FinalityStats {
            finalized_count: self.finalized_blocks.len(),
            finality_threshold: self.finality_threshold,
            max_reorg_depth: self.max_reorg_depth,
        }
    }
}

impl Default for ForkResolver {
    fn default() -> Self {
        Self::new(
            32, // 32 blocks for finality (~6.4 minutes with 12s blocks)
            64, // Max 64 block reorg allowed
        )
    }
}

/// Information about a reorganization
#[derive(Debug, Clone)]
pub struct ReorgInfo {
    pub common_ancestor_height: u64,
    pub reorg_depth: u64,
    pub blocks_to_remove: Vec<Block>,
    pub blocks_to_add: Vec<Block>,
}

/// Finality status of a block
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalityStatus {
    /// Block is finalized and cannot be reorganized
    Finalized,
    /// Block is near finalization
    NearFinalized { blocks_until_final: u64 },
    /// Block is not yet finalized
    Unfinalized { blocks_until_final: u64 },
}

/// Statistics about finality
#[derive(Debug, Clone)]
pub struct FinalityStats {
    pub finalized_count: usize,
    pub finality_threshold: u64,
    pub max_reorg_depth: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::block::{Block, BlockHeader};

    fn create_test_block(height: u64, previous_hash: Hash) -> Block {
        let mut state_root = [0u8; 32];
        state_root[0] = height as u8;

        let header = BlockHeader::new(
            1,
            height,
            1000 + height,
            previous_hash,
            state_root,
            [0u8; 32],
            [0u8; 32],
            [0u8; 32],
            [0u8; 64],
            0,
            1000000,
            vec![],
        );

        Block::new(header, vec![])
    }

    fn create_test_chain(length: usize) -> Vec<Block> {
        let mut chain = Vec::new();
        let mut prev_hash = [0u8; 32];

        for i in 0..length {
            let block = create_test_block(i as u64, prev_hash);
            prev_hash = block.hash();
            chain.push(block);
        }

        chain
    }

    #[test]
    fn test_fork_resolver_creation() {
        let resolver = ForkResolver::new(32, 64);
        assert_eq!(resolver.finality_threshold, 32);
        assert_eq!(resolver.max_reorg_depth, 64);
    }

    #[test]
    fn test_validate_chain() {
        let resolver = ForkResolver::default();
        let chain = create_test_chain(10);

        assert!(resolver.validate_chain(&chain).is_ok());
    }

    #[test]
    fn test_validate_chain_invalid() {
        let resolver = ForkResolver::default();
        let mut chain = create_test_chain(3);

        // Break the chain by modifying a block's height
        // This is done by creating an invalid chain differently
        let _ = chain.get_mut(2); // Just to use chain

        // Create an actually invalid chain
        let block1 = create_test_block(0, [0u8; 32]);
        let block2 = create_test_block(0, [0u8; 32]); // Same height as block1
        let invalid_chain = vec![block1, block2];

        // This should fail due to non-sequential heights
        assert!(resolver.validate_chain(&invalid_chain).is_err());
    }

    #[test]
    fn test_finalize_block() {
        let mut resolver = ForkResolver::default();
        let chain = create_test_chain(5);
        let block_hash = chain[0].hash();

        resolver.finalize_block(block_hash);
        assert!(resolver.is_finalized(&block_hash));
    }

    #[test]
    fn test_process_finalization() {
        let mut resolver = ForkResolver::new(5, 10);
        let chain = create_test_chain(10);

        let newly_finalized = resolver.process_finalization(&chain);

        // Should finalize first 5 blocks (10 - 5 = 5)
        assert_eq!(newly_finalized.len(), 5);

        // Check that they are finalized
        for i in 0..5 {
            assert!(resolver.is_finalized(&chain[i].hash()));
        }

        // Later blocks should not be finalized
        for i in 5..10 {
            assert!(!resolver.is_finalized(&chain[i].hash()));
        }
    }

    #[test]
    fn test_detect_reorg_no_change() {
        let resolver = ForkResolver::default();
        let chain = create_test_chain(10);

        let result = resolver.detect_reorg(&chain, &chain);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_detect_reorg_simple() {
        let resolver = ForkResolver::default();
        let main_chain = create_test_chain(10);

        // Create a fork: take first 8 blocks (0-7), then create different block at height 8
        let mut fork_chain = main_chain[..8].to_vec();

        // Create a different block 8 by using a different state_root
        let different_state_root = [1u8; 32]; // Different from normal
        let fork_block_header = BlockHeader::new(
            1,
            8,
            1000 + 8,
            main_chain[7].hash(),
            different_state_root,
            [0u8; 32],
            [0u8; 32],
            [0u8; 32],
            [0u8; 64],
            0,
            1000000,
            vec![],
        );
        let fork_block = Block::new(fork_block_header, vec![]);
        fork_chain.push(fork_block);

        let result = resolver.detect_reorg(&main_chain, &fork_chain);
        assert!(result.is_ok());

        if let Some(reorg) = result.unwrap() {
            assert_eq!(reorg.common_ancestor_height, 7); // Block at height 7 is common
            assert_eq!(reorg.reorg_depth, 2); // Blocks 8, 9 need to be removed
            assert_eq!(reorg.blocks_to_remove.len(), 2); // Blocks 8 and 9
            assert_eq!(reorg.blocks_to_add.len(), 1); // New block 8
        }
    }

    #[test]
    fn test_detect_reorg_too_deep() {
        let resolver = ForkResolver::new(5, 3); // Max reorg depth of 3
        let main_chain = create_test_chain(10);

        // Create a fork at height 5 (reorg depth would be 4)
        let fork_chain = create_test_chain(6);

        let result = resolver.detect_reorg(&main_chain, &fork_chain);
        assert!(result.is_err());
    }

    #[test]
    fn test_finality_status() {
        let mut resolver = ForkResolver::new(10, 20);
        let chain = create_test_chain(15);
        let block_hash = chain[0].hash();

        // Initially unfinalized with depth 0
        let status = resolver.get_finality_status(&block_hash, 0);
        assert!(matches!(status, FinalityStatus::Unfinalized { .. }));

        // At depth 10, should be near finalized
        let status = resolver.get_finality_status(&block_hash, 10);
        assert!(matches!(status, FinalityStatus::NearFinalized { .. }));

        // After finalization
        resolver.finalize_block(block_hash);
        let status = resolver.get_finality_status(&block_hash, 15);
        assert_eq!(status, FinalityStatus::Finalized);
    }

    #[test]
    fn test_get_finality_stats() {
        let mut resolver = ForkResolver::new(32, 64);
        let chain = create_test_chain(5);

        for block in &chain {
            resolver.finalize_block(block.hash());
        }

        let stats = resolver.get_finality_stats();
        assert_eq!(stats.finalized_count, 5);
        assert_eq!(stats.finality_threshold, 32);
        assert_eq!(stats.max_reorg_depth, 64);
    }
}
