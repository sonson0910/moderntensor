// Fast finality mechanism with BFT-style guarantees
// Provides immediate finality for blocks with sufficient validator signatures

use crate::error::ConsensusError;
use crate::validator::ValidatorSet;
use luxtensor_core::types::{Address, Hash};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};

/// Fast finality manager using validator signatures
pub struct FastFinality {
    /// Required percentage of stake for finality (e.g., 67 for 2/3)
    finality_threshold_percent: u8,
    /// Validator set
    validator_set: ValidatorSet,
    /// Signatures collected per block
    signatures: HashMap<Hash, BlockSignatures>,
}

/// Signatures for a block
#[derive(Debug, Clone)]
struct BlockSignatures {
    /// Validators who signed
    signers: HashSet<Address>,
    /// Total stake that has signed
    total_stake: u128,
    /// Whether block reached finality
    finalized: bool,
}

impl FastFinality {
    /// Create a new fast finality instance
    pub fn new(finality_threshold_percent: u8, validator_set: ValidatorSet) -> Self {
        assert!(
            finality_threshold_percent > 50 && finality_threshold_percent <= 100,
            "Threshold must be between 51 and 100"
        );

        Self {
            finality_threshold_percent,
            validator_set,
            signatures: HashMap::new(),
        }
    }

    /// Add a validator signature for a block
    pub fn add_signature(
        &mut self,
        block_hash: Hash,
        validator: Address,
    ) -> Result<bool, ConsensusError> {
        // Verify validator exists and get their stake
        let validator_info = self
            .validator_set
            .get_validator(&validator)
            .ok_or(ConsensusError::ValidatorNotFound(format!("{:?}", validator)))?;

        // Get or create signature entry
        let entry = self.signatures.entry(block_hash).or_insert_with(|| {
            BlockSignatures {
                signers: HashSet::new(),
                total_stake: 0,
                finalized: false,
            }
        });

        // Check if already finalized
        if entry.finalized {
            return Ok(true);
        }

        // Check if validator already signed
        if entry.signers.contains(&validator) {
            return Ok(entry.finalized);
        }

        // Add signature
        entry.signers.insert(validator);
        entry.total_stake += validator_info.stake;

        debug!(
            "Added signature from validator {} for block {}",
            hex::encode(&validator),
            hex::encode(&block_hash)
        );

        // Check if finality threshold reached
        let total_stake = self.validator_set.total_stake();
        let required_stake = (total_stake * self.finality_threshold_percent as u128) / 100;

        if entry.total_stake >= required_stake {
            entry.finalized = true;
            info!(
                "Block {} reached fast finality with {}/{} stake ({}%)",
                hex::encode(&block_hash),
                entry.total_stake,
                total_stake,
                (entry.total_stake * 100) / total_stake
            );
            Ok(true)
        } else {
            debug!(
                "Block {} has {}/{} stake ({}%), needs {}% for finality",
                hex::encode(&block_hash),
                entry.total_stake,
                total_stake,
                (entry.total_stake * 100) / total_stake,
                self.finality_threshold_percent
            );
            Ok(false)
        }
    }

    /// Check if a block has reached finality
    pub fn is_finalized(&self, block_hash: &Hash) -> bool {
        self.signatures
            .get(block_hash)
            .map(|s| s.finalized)
            .unwrap_or(false)
    }

    /// Get finality progress for a block (percentage of stake signed)
    pub fn get_finality_progress(&self, block_hash: &Hash) -> Option<u8> {
        self.signatures.get(block_hash).map(|s| {
            let total_stake = self.validator_set.total_stake();
            if total_stake == 0 {
                0
            } else {
                ((s.total_stake * 100) / total_stake) as u8
            }
        })
    }

    /// Get number of validators who signed a block
    pub fn get_signer_count(&self, block_hash: &Hash) -> usize {
        self.signatures
            .get(block_hash)
            .map(|s| s.signers.len())
            .unwrap_or(0)
    }

    /// Get list of validators who signed a block
    pub fn get_signers(&self, block_hash: &Hash) -> Option<Vec<Address>> {
        self.signatures
            .get(block_hash)
            .map(|s| s.signers.iter().copied().collect())
    }

    /// Clear old signatures for blocks that are no longer needed
    pub fn prune_old_signatures(&mut self, keep_blocks: &[Hash]) {
        let keep_set: HashSet<_> = keep_blocks.iter().copied().collect();
        self.signatures.retain(|hash, _| keep_set.contains(hash));
    }

    /// Get statistics about fast finality
    pub fn get_stats(&self) -> FastFinalityStats {
        let total_blocks = self.signatures.len();
        let finalized_blocks = self
            .signatures
            .values()
            .filter(|s| s.finalized)
            .count();

        let avg_stake = if total_blocks > 0 {
            let total: u128 = self.signatures.values().map(|s| s.total_stake).sum();
            total / total_blocks as u128
        } else {
            0
        };

        FastFinalityStats {
            total_blocks,
            finalized_blocks,
            pending_blocks: total_blocks - finalized_blocks,
            average_stake: avg_stake,
            threshold_percent: self.finality_threshold_percent,
        }
    }

    /// Update validator set
    pub fn update_validator_set(&mut self, validator_set: ValidatorSet) {
        self.validator_set = validator_set;

        // Re-check all pending blocks for finality
        let total_stake = self.validator_set.total_stake();
        let required_stake = (total_stake * self.finality_threshold_percent as u128) / 100;

        for entry in self.signatures.values_mut() {
            if !entry.finalized && entry.total_stake >= required_stake {
                entry.finalized = true;
            }
        }
    }
}

/// Fast finality statistics
#[derive(Debug, Clone)]
pub struct FastFinalityStats {
    pub total_blocks: usize,
    pub finalized_blocks: usize,
    pub pending_blocks: usize,
    pub average_stake: u128,
    pub threshold_percent: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::Validator;
    use luxtensor_crypto::KeyPair;

    fn create_test_validator(stake: u128) -> (Address, Validator) {
        let keypair = KeyPair::generate();
        let address = Address::from(keypair.address());
        let mut public_key = [0u8; 32];
        let pk_bytes = keypair.public_key_bytes();
        public_key.copy_from_slice(&pk_bytes[..32.min(pk_bytes.len())]);

        let validator = Validator {
            address,
            stake,
            public_key,
            active: true,
            rewards: 0,
            last_active_slot: 0,
            activation_epoch: 0,
        };

        (address, validator)
    }

    fn create_validator_set(count: usize, stake_per_validator: u128) -> (ValidatorSet, Vec<Address>) {
        let mut set = ValidatorSet::new();
        let mut addresses = Vec::new();

        for _ in 0..count {
            let (addr, validator) = create_test_validator(stake_per_validator);
            set.add_validator(validator).unwrap();
            addresses.push(addr);
        }

        (set, addresses)
    }

    #[test]
    fn test_fast_finality_creation() {
        let (validator_set, _) = create_validator_set(4, 100);
        let finality = FastFinality::new(67, validator_set);

        assert_eq!(finality.finality_threshold_percent, 67);
    }

    #[test]
    #[should_panic(expected = "Threshold must be between 51 and 100")]
    fn test_fast_finality_invalid_threshold() {
        let (validator_set, _) = create_validator_set(4, 100);
        let _ = FastFinality::new(50, validator_set); // Should panic
    }

    #[test]
    fn test_add_signature() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];

        // Add first signature (33% stake)
        let finalized = finality.add_signature(block_hash, addresses[0]).unwrap();
        assert!(!finalized);

        // Add second signature (67% stake) - should NOT be finalized yet (200/300 = 66.6%)
        let finalized = finality.add_signature(block_hash, addresses[1]).unwrap();
        assert!(!finalized);

        // Add third signature (100% stake) - NOW it's finalized
        let finalized = finality.add_signature(block_hash, addresses[2]).unwrap();
        assert!(finalized);
    }

    #[test]
    fn test_duplicate_signature() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];

        // Add signature
        finality.add_signature(block_hash, addresses[0]).unwrap();

        // Add same signature again - should be idempotent
        finality.add_signature(block_hash, addresses[0]).unwrap();

        // Should still only count once
        assert_eq!(finality.get_signer_count(&block_hash), 1);
    }

    #[test]
    fn test_is_finalized() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];

        assert!(!finality.is_finalized(&block_hash));

        // Add all 3 signatures for 100% stake
        finality.add_signature(block_hash, addresses[0]).unwrap();
        finality.add_signature(block_hash, addresses[1]).unwrap();
        finality.add_signature(block_hash, addresses[2]).unwrap();

        assert!(finality.is_finalized(&block_hash));
    }

    #[test]
    fn test_get_finality_progress() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];

        // Initially 0%
        assert_eq!(finality.get_finality_progress(&block_hash), None);

        // After 1 signature: 33%
        finality.add_signature(block_hash, addresses[0]).unwrap();
        assert_eq!(finality.get_finality_progress(&block_hash), Some(33));

        // After 2 signatures: 66%
        finality.add_signature(block_hash, addresses[1]).unwrap();
        assert_eq!(finality.get_finality_progress(&block_hash), Some(66));

        // After 3 signatures: 100%
        finality.add_signature(block_hash, addresses[2]).unwrap();
        assert_eq!(finality.get_finality_progress(&block_hash), Some(100));
    }

    #[test]
    fn test_get_signers() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];

        finality.add_signature(block_hash, addresses[0]).unwrap();
        finality.add_signature(block_hash, addresses[1]).unwrap();

        let signers = finality.get_signers(&block_hash).unwrap();
        assert_eq!(signers.len(), 2);
        assert!(signers.contains(&addresses[0]));
        assert!(signers.contains(&addresses[1]));
    }

    #[test]
    fn test_prune_old_signatures() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block1 = [1u8; 32];
        let block2 = [2u8; 32];

        finality.add_signature(block1, addresses[0]).unwrap();
        finality.add_signature(block2, addresses[0]).unwrap();

        assert_eq!(finality.signatures.len(), 2);

        // Prune block1
        finality.prune_old_signatures(&[block2]);

        assert_eq!(finality.signatures.len(), 1);
        assert!(finality.signatures.contains_key(&block2));
        assert!(!finality.signatures.contains_key(&block1));
    }

    #[test]
    fn test_get_stats() {
        let (validator_set, addresses) = create_validator_set(4, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block1 = [1u8; 32];
        let block2 = [2u8; 32];

        // Block 1: finalized (3/4 validators)
        finality.add_signature(block1, addresses[0]).unwrap();
        finality.add_signature(block1, addresses[1]).unwrap();
        finality.add_signature(block1, addresses[2]).unwrap();

        // Block 2: not finalized (2/4 validators)
        finality.add_signature(block2, addresses[0]).unwrap();
        finality.add_signature(block2, addresses[1]).unwrap();

        let stats = finality.get_stats();
        assert_eq!(stats.total_blocks, 2);
        assert_eq!(stats.finalized_blocks, 1);
        assert_eq!(stats.pending_blocks, 1);
        assert_eq!(stats.threshold_percent, 67);
    }

    #[test]
    fn test_invalid_validator() {
        let (validator_set, _) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];
        let invalid_validator = Address::from([99u8; 20]);

        let result = finality.add_signature(block_hash, invalid_validator);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_validator_set() {
        let (validator_set, addresses) = create_validator_set(3, 100);
        let mut finality = FastFinality::new(67, validator_set);

        let block_hash = [1u8; 32];

        // Add signatures but don't reach threshold
        finality.add_signature(block_hash, addresses[0]).unwrap();
        assert!(!finality.is_finalized(&block_hash));

        // Create new validator set with lower total stake
        let (new_set, _) = create_validator_set(2, 100);

        // Update validator set - block might now be finalized if threshold reached
        finality.update_validator_set(new_set);
    }
}
