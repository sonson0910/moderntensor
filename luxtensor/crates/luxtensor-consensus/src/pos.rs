use crate::error::ConsensusError;
use crate::validator::ValidatorSet;
use luxtensor_core::types::{Address, Hash};
use luxtensor_crypto::keccak256;
use serde::{Deserialize, Serialize};
use parking_lot::RwLock;
use std::sync::Arc;

/// Configuration for PoS consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Number of seconds per slot
    pub slot_duration: u64,
    /// Minimum stake required to become a validator
    pub min_stake: u128,
    /// Block reward amount
    pub block_reward: u128,
    /// Epoch length in slots
    pub epoch_length: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            slot_duration: 12, // 12 seconds per block
            min_stake: 32_000_000_000_000_000_000u128, // 32 tokens minimum
            block_reward: 2_000_000_000_000_000_000u128, // 2 tokens per block
            epoch_length: 32, // 32 slots per epoch
        }
    }
}

/// Proof of Stake consensus mechanism
pub struct ProofOfStake {
    validator_set: Arc<RwLock<ValidatorSet>>,
    config: ConsensusConfig,
    current_epoch: RwLock<u64>,
    /// Last finalized block hash for VRF seed entropy
    last_block_hash: RwLock<Hash>,
}

impl ProofOfStake {
    /// Create a new PoS consensus instance
    pub fn new(config: ConsensusConfig) -> Self {
        Self {
            validator_set: Arc::new(RwLock::new(ValidatorSet::new())),
            config,
            current_epoch: RwLock::new(0),
            last_block_hash: RwLock::new([0u8; 32]),
        }
    }

    /// Create with an existing validator set
    pub fn with_validator_set(config: ConsensusConfig, validator_set: ValidatorSet) -> Self {
        Self {
            validator_set: Arc::new(RwLock::new(validator_set)),
            config,
            current_epoch: RwLock::new(0),
            last_block_hash: RwLock::new([0u8; 32]),
        }
    }

    /// Update last block hash (call after block finalization)
    pub fn update_last_block_hash(&self, hash: Hash) {
        *self.last_block_hash.write() = hash;
    }

    /// Select a validator for a given slot using VRF-based selection
    pub fn select_validator(&self, slot: u64) -> Result<Address, ConsensusError> {
        let seed = self.compute_seed(slot);
        let validator_set = self.validator_set.read();

        validator_set
            .select_by_seed(&seed)
            .map_err(|e| ConsensusError::ValidatorSelection(e.to_string()))
    }

    /// Validate that the correct validator produced the block
    pub fn validate_block_producer(
        &self,
        producer: &Address,
        slot: u64,
    ) -> Result<(), ConsensusError> {
        let expected = self.select_validator(slot)?;

        if producer != &expected {
            return Err(ConsensusError::InvalidProducer {
                expected,
                actual: *producer,
            });
        }

        Ok(())
    }

    /// Compute the randomness seed for validator selection at a given slot
    /// Uses epoch + slot + last_block_hash for unpredictable entropy
    pub fn compute_seed(&self, slot: u64) -> Hash {
        let epoch = slot / self.config.epoch_length;
        let last_hash = *self.last_block_hash.read();

        let mut data = Vec::with_capacity(48);
        data.extend_from_slice(&epoch.to_le_bytes());
        data.extend_from_slice(&slot.to_le_bytes());
        data.extend_from_slice(&last_hash); // Added for entropy
        keccak256(&data)
    }

    /// Calculate and distribute block rewards
    pub fn distribute_reward(&self, producer: &Address) -> Result<(), ConsensusError> {
        let mut validator_set = self.validator_set.write();

        validator_set
            .add_reward(producer, self.config.block_reward)
            .map_err(|e| ConsensusError::RewardDistribution(e.to_string()))
    }

    /// Add a new validator to the set
    pub fn add_validator(
        &self,
        address: Address,
        stake: u128,
        public_key: [u8; 32],
    ) -> Result<(), ConsensusError> {
        if stake < self.config.min_stake {
            return Err(ConsensusError::InsufficientStake {
                provided: stake,
                required: self.config.min_stake,
            });
        }

        let validator = crate::validator::Validator::new(address, stake, public_key);
        let mut validator_set = self.validator_set.write();

        validator_set
            .add_validator(validator)
            .map_err(|e| ConsensusError::ValidatorManagement(e.to_string()))
    }

    /// Remove a validator from the set
    pub fn remove_validator(&self, address: &Address) -> Result<(), ConsensusError> {
        let mut validator_set = self.validator_set.write();

        validator_set
            .remove_validator(address)
            .map_err(|e| ConsensusError::ValidatorManagement(e.to_string()))
    }

    /// Update validator stake
    pub fn update_validator_stake(
        &self,
        address: &Address,
        new_stake: u128,
    ) -> Result<(), ConsensusError> {
        if new_stake < self.config.min_stake {
            return Err(ConsensusError::InsufficientStake {
                provided: new_stake,
                required: self.config.min_stake,
            });
        }

        let mut validator_set = self.validator_set.write();

        validator_set
            .update_stake(address, new_stake)
            .map_err(|e| ConsensusError::ValidatorManagement(e.to_string()))
    }

    /// Get the current epoch
    pub fn current_epoch(&self) -> u64 {
        *self.current_epoch.read()
    }

    /// Advance to the next epoch
    pub fn advance_epoch(&self) {
        let mut epoch = self.current_epoch.write();
        *epoch += 1;
    }

    /// Get slot from timestamp
    pub fn get_slot(&self, timestamp: u64, genesis_time: u64) -> u64 {
        if timestamp < genesis_time {
            return 0;
        }
        (timestamp - genesis_time) / self.config.slot_duration
    }

    /// Get validator set reference
    pub fn validator_set(&self) -> Arc<RwLock<ValidatorSet>> {
        Arc::clone(&self.validator_set)
    }

    /// Get configuration
    pub fn config(&self) -> &ConsensusConfig {
        &self.config
    }

    /// Get total stake in the network
    pub fn total_stake(&self) -> u128 {
        self.validator_set.read().total_stake()
    }

    /// Get number of validators
    pub fn validator_count(&self) -> usize {
        self.validator_set.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_address(index: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[0] = index;
        Address::from(bytes)
    }

    #[test]
    fn test_pos_creation() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config);

        assert_eq!(pos.validator_count(), 0);
        assert_eq!(pos.current_epoch(), 0);
    }

    #[test]
    fn test_add_validator() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let address = create_test_address(1);
        let pubkey = [1u8; 32];

        let result = pos.add_validator(address, config.min_stake, pubkey);
        assert!(result.is_ok());
        assert_eq!(pos.validator_count(), 1);
    }

    #[test]
    fn test_add_validator_insufficient_stake() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let address = create_test_address(1);
        let pubkey = [1u8; 32];

        let result = pos.add_validator(address, config.min_stake - 1, pubkey);
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_selection() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        // Add validators
        for i in 1..=3 {
            let address = create_test_address(i);
            let pubkey = [i; 32];
            pos.add_validator(address, config.min_stake * (i as u128), pubkey)
                .unwrap();
        }

        // Select validator for slot 0
        let selected = pos.select_validator(0);
        assert!(selected.is_ok());
    }

    #[test]
    fn test_validate_block_producer() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        // Add validator
        let address = create_test_address(1);
        let pubkey = [1u8; 32];
        pos.add_validator(address, config.min_stake, pubkey).unwrap();

        // Select validator for slot 0
        let selected = pos.select_validator(0).unwrap();

        // Validate correct producer
        assert!(pos.validate_block_producer(&selected, 0).is_ok());

        // Validate wrong producer
        let wrong_address = create_test_address(2);
        assert!(pos.validate_block_producer(&wrong_address, 0).is_err());
    }

    #[test]
    fn test_reward_distribution() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let address = create_test_address(1);
        let pubkey = [1u8; 32];
        pos.add_validator(address, config.min_stake, pubkey).unwrap();

        // Distribute reward
        let result = pos.distribute_reward(&address);
        assert!(result.is_ok());

        // Check reward was added
        let validator_set = pos.validator_set.read();
        let validator = validator_set.get_validator(&address).unwrap();
        assert_eq!(validator.rewards, config.block_reward);
    }

    #[test]
    fn test_seed_computation() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config);

        // Same slot should produce same seed
        let seed1 = pos.compute_seed(0);
        let seed2 = pos.compute_seed(0);
        assert_eq!(seed1, seed2);

        // Different slots should produce different seeds
        let seed3 = pos.compute_seed(1);
        assert_ne!(seed1, seed3);
    }

    #[test]
    fn test_get_slot() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config.clone());

        let genesis_time = 1000u64;

        // At genesis
        assert_eq!(pos.get_slot(genesis_time, genesis_time), 0);

        // After one slot duration
        assert_eq!(
            pos.get_slot(genesis_time + config.slot_duration, genesis_time),
            1
        );

        // After multiple slot durations
        assert_eq!(
            pos.get_slot(genesis_time + config.slot_duration * 5, genesis_time),
            5
        );
    }

    #[test]
    fn test_epoch_advancement() {
        let config = ConsensusConfig::default();
        let pos = ProofOfStake::new(config);

        assert_eq!(pos.current_epoch(), 0);

        pos.advance_epoch();
        assert_eq!(pos.current_epoch(), 1);

        pos.advance_epoch();
        assert_eq!(pos.current_epoch(), 2);
    }
}
