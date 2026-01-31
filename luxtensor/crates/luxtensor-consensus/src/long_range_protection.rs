//! Long-Range Attack Prevention
//!
//! Implements weak subjectivity checkpoints and finality-based pruning
//! to prevent long-range attacks in PoS.

use std::collections::HashMap;
use parking_lot::RwLock;
use luxtensor_core::types::Hash;

/// Checkpoint data for weak subjectivity
#[derive(Debug, Clone)]
pub struct Checkpoint {
    /// Block hash at this checkpoint
    pub block_hash: Hash,
    /// Block height at this checkpoint
    pub height: u64,
    /// Epoch number
    pub epoch: u64,
    /// State root at this checkpoint
    pub state_root: Hash,
    /// Timestamp when checkpoint was created
    pub timestamp: u64,
}

/// Long-range attack protection configuration
#[derive(Debug, Clone)]
pub struct LongRangeConfig {
    /// Weak subjectivity period in blocks (default: ~2 weeks at 3s blocks)
    pub weak_subjectivity_period: u64,
    /// Checkpoint interval (blocks between checkpoints)
    pub checkpoint_interval: u64,
    /// Maximum reorg depth allowed
    pub max_reorg_depth: u64,
    /// Minimum finality confirmations
    pub min_finality_confirmations: u64,
    /// Whether to require a recent checkpoint for syncing from scratch
    pub require_recent_checkpoint: bool,
    /// Maximum age of checkpoint in seconds for initial sync
    pub max_checkpoint_age_secs: u64,
}

impl Default for LongRangeConfig {
    fn default() -> Self {
        Self {
            weak_subjectivity_period: 403_200, // ~2 weeks at 3s blocks
            checkpoint_interval: 100,          // Checkpoint every 100 blocks
            max_reorg_depth: 1000,             // Max 1000 block reorg
            min_finality_confirmations: 32,    // 32 blocks for finality
            require_recent_checkpoint: true,   // SECURITY: Require recent checkpoint
            max_checkpoint_age_secs: 604_800,  // 7 days max checkpoint age
        }
    }
}

/// Long-range attack prevention manager
pub struct LongRangeProtection {
    config: LongRangeConfig,
    /// Finalized checkpoints
    checkpoints: RwLock<Vec<Checkpoint>>,
    /// Most recent finalized block hash
    finalized_hash: RwLock<Hash>,
    /// Most recent finalized height
    finalized_height: RwLock<u64>,
}

impl LongRangeProtection {
    /// Create new protection manager with genesis checkpoint
    pub fn new(config: LongRangeConfig, genesis_hash: Hash) -> Self {
        let genesis_checkpoint = Checkpoint {
            block_hash: genesis_hash,
            height: 0,
            epoch: 0,
            state_root: [0u8; 32],
            timestamp: 0,
        };

        Self {
            config,
            checkpoints: RwLock::new(vec![genesis_checkpoint]),
            finalized_hash: RwLock::new(genesis_hash),
            finalized_height: RwLock::new(0),
        }
    }

    /// Check if a block is within the weak subjectivity period
    pub fn is_within_weak_subjectivity(&self, block_height: u64) -> bool {
        let finalized = *self.finalized_height.read();
        block_height >= finalized.saturating_sub(self.config.weak_subjectivity_period)
    }

    /// Check if a reorg is allowed (within max depth)
    pub fn is_reorg_allowed(&self, reorg_depth: u64) -> bool {
        reorg_depth <= self.config.max_reorg_depth
    }

    /// Add a new checkpoint
    pub fn add_checkpoint(&self, checkpoint: Checkpoint) -> Result<(), &'static str> {
        let mut checkpoints = self.checkpoints.write();

        // Verify checkpoint is newer than last
        if let Some(last) = checkpoints.last() {
            if checkpoint.height <= last.height {
                return Err("Checkpoint height must be greater than last");
            }
        }

        checkpoints.push(checkpoint);
        Ok(())
    }

    /// Update finalized block
    pub fn update_finalized(&self, hash: Hash, height: u64) {
        *self.finalized_hash.write() = hash;
        *self.finalized_height.write() = height;

        // Create checkpoint if at interval
        if height % self.config.checkpoint_interval == 0 {
            let checkpoint = Checkpoint {
                block_hash: hash,
                height,
                epoch: height / 100, // Assuming 100 blocks per epoch
                state_root: [0u8; 32], // Would be actual state root
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };
            let _ = self.add_checkpoint(checkpoint);
        }
    }

    /// Get the most recent checkpoint
    pub fn get_latest_checkpoint(&self) -> Option<Checkpoint> {
        self.checkpoints.read().last().cloned()
    }

    /// Validate a block against checkpoints (for sync from scratch)
    pub fn validate_against_checkpoints(&self, block_hash: Hash, height: u64) -> bool {
        let checkpoints = self.checkpoints.read();

        // Check if this block is at a checkpoint height
        for cp in checkpoints.iter() {
            if cp.height == height {
                // Block at checkpoint must match
                return cp.block_hash == block_hash;
            }
        }

        // Not at a checkpoint height - allow
        true
    }

    /// Check if block is finalized (cannot be reverted)
    pub fn is_finalized(&self, block_height: u64) -> bool {
        let finalized = *self.finalized_height.read();
        block_height <= finalized
    }

    /// Get finalized height
    pub fn finalized_height(&self) -> u64 {
        *self.finalized_height.read()
    }

    /// Get all checkpoints
    pub fn get_checkpoints(&self) -> Vec<Checkpoint> {
        self.checkpoints.read().clone()
    }

    /// Prune old checkpoints (keep recent ones)
    pub fn prune_old_checkpoints(&self, keep_count: usize) {
        let mut checkpoints = self.checkpoints.write();
        if checkpoints.len() > keep_count {
            let remove_count = checkpoints.len() - keep_count;
            checkpoints.drain(0..remove_count);
        }
    }

    /// Check if we have a recent enough checkpoint for initial sync
    /// Returns true if sync can proceed, false if we need a more recent checkpoint
    pub fn can_sync_from_scratch(&self, current_timestamp: u64) -> Result<bool, &'static str> {
        if !self.config.require_recent_checkpoint {
            return Ok(true); // Not required
        }

        let checkpoints = self.checkpoints.read();

        if let Some(latest) = checkpoints.last() {
            if latest.height == 0 {
                // Only genesis checkpoint - need more recent
                return Err("No recent checkpoint available, need trusted checkpoint for initial sync");
            }

            let age = current_timestamp.saturating_sub(latest.timestamp);
            if age > self.config.max_checkpoint_age_secs {
                return Err("Latest checkpoint is too old for safe initial sync");
            }

            Ok(true)
        } else {
            Err("No checkpoints available")
        }
    }

    /// Validate a checkpoint from external source (e.g., trusted node)
    pub fn validate_external_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), &'static str> {
        // Check height is reasonable
        let finalized = *self.finalized_height.read();
        if checkpoint.height < finalized {
            return Err("External checkpoint is below finalized height");
        }

        // Check timestamp is not in the future
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if checkpoint.timestamp > now + 60 {
            return Err("Checkpoint timestamp is in the future");
        }

        // Check block hash is not empty
        if checkpoint.block_hash == [0u8; 32] {
            return Err("Invalid checkpoint: empty block hash");
        }

        Ok(())
    }

    /// Get checkpoint status (for RPC)
    pub fn get_checkpoint_status(&self) -> CheckpointStatus {
        let checkpoints = self.checkpoints.read();
        let latest = checkpoints.last().cloned();
        let finalized = *self.finalized_height.read();

        CheckpointStatus {
            total_checkpoints: checkpoints.len(),
            latest_checkpoint: latest,
            finalized_height: finalized,
            weak_subjectivity_period: self.config.weak_subjectivity_period,
        }
    }
}

/// Status of checkpoint system for monitoring
#[derive(Debug, Clone)]
pub struct CheckpointStatus {
    pub total_checkpoints: usize,
    pub latest_checkpoint: Option<Checkpoint>,
    pub finalized_height: u64,
    pub weak_subjectivity_period: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_long_range_protection_creation() {
        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(LongRangeConfig::default(), genesis_hash);

        assert_eq!(protection.finalized_height(), 0);
        assert!(protection.get_latest_checkpoint().is_some());
    }

    #[test]
    fn test_weak_subjectivity_check() {
        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(LongRangeConfig::default(), genesis_hash);

        // Update finalized to block 1000
        protection.update_finalized([2u8; 32], 1000);

        // Block 500 should be within weak subjectivity (0 is still recent)
        assert!(protection.is_within_weak_subjectivity(500));
    }

    #[test]
    fn test_reorg_depth_limit() {
        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(LongRangeConfig::default(), genesis_hash);

        // Reorg of 100 blocks - should be allowed
        assert!(protection.is_reorg_allowed(100));

        // Reorg of 2000 blocks - should NOT be allowed
        assert!(!protection.is_reorg_allowed(2000));
    }

    #[test]
    fn test_checkpoint_validation() {
        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(LongRangeConfig::default(), genesis_hash);

        // Add checkpoint at height 100
        let cp = Checkpoint {
            block_hash: [5u8; 32],
            height: 100,
            epoch: 1,
            state_root: [0u8; 32],
            timestamp: 1000,
        };
        protection.add_checkpoint(cp).unwrap();

        // Correct block at checkpoint
        assert!(protection.validate_against_checkpoints([5u8; 32], 100));

        // Wrong block at checkpoint
        assert!(!protection.validate_against_checkpoints([6u8; 32], 100));

        // Any block at non-checkpoint height
        assert!(protection.validate_against_checkpoints([7u8; 32], 101));
    }

    #[test]
    fn test_checkpoint_status() {
        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(LongRangeConfig::default(), genesis_hash);

        let status = protection.get_checkpoint_status();
        assert_eq!(status.total_checkpoints, 1); // Genesis
        assert_eq!(status.finalized_height, 0);
        assert!(status.latest_checkpoint.is_some());
    }

    #[test]
    fn test_external_checkpoint_validation() {
        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(LongRangeConfig::default(), genesis_hash);

        // Valid checkpoint
        let valid_cp = Checkpoint {
            block_hash: [5u8; 32],
            height: 100,
            epoch: 1,
            state_root: [0u8; 32],
            timestamp: 1000,
        };
        assert!(protection.validate_external_checkpoint(&valid_cp).is_ok());

        // Invalid: empty block hash
        let invalid_cp = Checkpoint {
            block_hash: [0u8; 32],
            height: 100,
            epoch: 1,
            state_root: [0u8; 32],
            timestamp: 1000,
        };
        assert!(protection.validate_external_checkpoint(&invalid_cp).is_err());
    }

    #[test]
    fn test_sync_from_scratch_requires_checkpoint() {
        let mut config = LongRangeConfig::default();
        config.require_recent_checkpoint = true;

        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(config, genesis_hash);

        // With only genesis checkpoint, sync should fail
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let result = protection.can_sync_from_scratch(now);
        assert!(result.is_err()); // Only genesis, needs more recent
    }

    #[test]
    fn test_sync_with_recent_checkpoint() {
        let mut config = LongRangeConfig::default();
        config.require_recent_checkpoint = true;
        config.max_checkpoint_age_secs = 3600; // 1 hour

        let genesis_hash = [1u8; 32];
        let protection = LongRangeProtection::new(config, genesis_hash);

        // Add a recent checkpoint
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cp = Checkpoint {
            block_hash: [5u8; 32],
            height: 100,
            epoch: 1,
            state_root: [0u8; 32],
            timestamp: now - 1800, // 30 minutes ago
        };
        protection.add_checkpoint(cp).unwrap();

        // Sync should succeed with recent checkpoint
        let result = protection.can_sync_from_scratch(now);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}
