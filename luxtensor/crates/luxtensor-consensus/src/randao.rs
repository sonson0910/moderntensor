//! RANDAO (RANdom DAO) Entropy Accumulator
//!
//! Implements a secure on-chain randomness beacon through validator reveals.
//! Each validator contributes entropy by revealing their precommitment,
//! and the cumulative mix provides unpredictable randomness for consensus.

use luxtensor_core::types::{Hash, Address};
use luxtensor_crypto::keccak256;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};

/// Configuration for RANDAO mixing
#[derive(Debug, Clone)]
pub struct RandaoConfig {
    /// Minimum reveals required before finalizing epoch
    pub min_reveals_for_epoch: usize,
    /// Maximum reveals to store per epoch
    pub max_reveals_per_epoch: usize,
    /// Penalty for missing reveal (in basis points, e.g., 100 = 1%)
    pub missing_reveal_penalty_bps: u16,
}

impl Default for RandaoConfig {
    fn default() -> Self {
        Self {
            min_reveals_for_epoch: 3,      // At least 3 validators must reveal
            max_reveals_per_epoch: 1000,   // Cap on reveals per epoch
            missing_reveal_penalty_bps: 10, // 0.1% penalty for missing reveal
        }
    }
}

/// A reveal from a validator
#[derive(Debug, Clone)]
pub struct ValidatorReveal {
    /// Validator who made the reveal
    pub validator: Address,
    /// The revealed value (should be hash of secret from previous epoch)
    pub reveal: Hash,
    /// Block number when reveal was made
    pub block_number: u64,
}

/// RANDAO entropy mixer
/// Accumulates validator reveals into a shared random beacon
pub struct RandaoMixer {
    config: RandaoConfig,
    /// Current cumulative mix
    current_mix: RwLock<Hash>,
    /// Reveals collected in current epoch
    current_epoch_reveals: RwLock<Vec<ValidatorReveal>>,
    /// Validators who have revealed this epoch (prevent double reveal)
    revealed_validators: RwLock<HashSet<Address>>,
    /// Current epoch number
    current_epoch: RwLock<u64>,
    /// Historical epoch mixes for verification
    epoch_mixes: RwLock<HashMap<u64, Hash>>,
}

impl RandaoMixer {
    /// Create a new RANDAO mixer with initial seed
    pub fn new(config: RandaoConfig, initial_seed: Hash) -> Self {
        Self {
            config,
            current_mix: RwLock::new(initial_seed),
            current_epoch_reveals: RwLock::new(Vec::new()),
            revealed_validators: RwLock::new(HashSet::new()),
            current_epoch: RwLock::new(0),
            epoch_mixes: RwLock::new(HashMap::new()),
        }
    }

    /// Create with genesis seed (all zeros XOR'd with genesis hash)
    pub fn with_genesis(config: RandaoConfig, genesis_hash: Hash) -> Self {
        Self::new(config, genesis_hash)
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        *self.current_epoch.read()
    }

    /// Get current RANDAO mix
    pub fn current_mix(&self) -> Hash {
        *self.current_mix.read()
    }

    /// Mix a new reveal into the accumulator
    /// Returns true if reveal was accepted, false if validator already revealed
    pub fn mix_reveal(
        &self,
        validator: Address,
        reveal: Hash,
        block_number: u64,
    ) -> Result<(), RandaoError> {
        // Check if validator already revealed this epoch
        {
            let revealed = self.revealed_validators.read();
            if revealed.contains(&validator) {
                return Err(RandaoError::DuplicateReveal(validator));
            }
        }

        // Check max reveals limit
        {
            let reveals = self.current_epoch_reveals.read();
            if reveals.len() >= self.config.max_reveals_per_epoch {
                return Err(RandaoError::MaxRevealsReached);
            }
        }

        // Mix the reveal into current mix: new_mix = keccak256(current_mix || reveal)
        {
            let mut mix = self.current_mix.write();
            let mut data = Vec::with_capacity(64);
            data.extend_from_slice(&*mix);
            data.extend_from_slice(&reveal);
            *mix = keccak256(&data);
        }

        // Record the reveal
        {
            let mut reveals = self.current_epoch_reveals.write();
            reveals.push(ValidatorReveal {
                validator,
                reveal,
                block_number,
            });
        }

        // Mark validator as revealed
        {
            let mut revealed = self.revealed_validators.write();
            revealed.insert(validator);
        }

        Ok(())
    }

    /// Check if validator has revealed this epoch
    pub fn has_revealed(&self, validator: &Address) -> bool {
        self.revealed_validators.read().contains(validator)
    }

    /// Get number of reveals in current epoch
    pub fn reveal_count(&self) -> usize {
        self.current_epoch_reveals.read().len()
    }

    /// Finalize the current epoch and return the final RANDAO mix
    /// Advances to next epoch
    pub fn finalize_epoch(&self) -> Result<Hash, RandaoError> {
        let reveal_count = self.reveal_count();

        // Check minimum reveals
        if reveal_count < self.config.min_reveals_for_epoch {
            return Err(RandaoError::InsufficientReveals {
                have: reveal_count,
                need: self.config.min_reveals_for_epoch,
            });
        }

        let final_mix = *self.current_mix.read();
        let current_epoch = *self.current_epoch.read();

        // Store epoch mix for historical reference
        {
            let mut epoch_mixes = self.epoch_mixes.write();
            epoch_mixes.insert(current_epoch, final_mix);
        }

        // Advance to next epoch
        {
            *self.current_epoch.write() = current_epoch + 1;
        }

        // Clear reveals for new epoch
        {
            self.current_epoch_reveals.write().clear();
            self.revealed_validators.write().clear();
        }

        // Note: We DON'T reset current_mix - it carries forward
        // This provides continuity and unpredictability

        Ok(final_mix)
    }

    /// Get historical epoch mix (for verification)
    pub fn get_epoch_mix(&self, epoch: u64) -> Option<Hash> {
        self.epoch_mixes.read().get(&epoch).copied()
    }

    /// Get list of validators who revealed this epoch
    pub fn get_revealed_validators(&self) -> Vec<Address> {
        self.revealed_validators.read().iter().copied().collect()
    }

    /// Get all reveals for current epoch
    pub fn get_current_reveals(&self) -> Vec<ValidatorReveal> {
        self.current_epoch_reveals.read().clone()
    }
}

/// RANDAO errors
#[derive(Debug, Clone)]
pub enum RandaoError {
    /// Validator already revealed this epoch
    DuplicateReveal(Address),
    /// Maximum reveals per epoch reached
    MaxRevealsReached,
    /// Not enough reveals to finalize epoch
    InsufficientReveals { have: usize, need: usize },
}

impl std::fmt::Display for RandaoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RandaoError::DuplicateReveal(addr) => {
                write!(f, "Validator {:?} already revealed this epoch", addr)
            }
            RandaoError::MaxRevealsReached => {
                write!(f, "Maximum reveals per epoch reached")
            }
            RandaoError::InsufficientReveals { have, need } => {
                write!(f, "Insufficient reveals: have {}, need {}", have, need)
            }
        }
    }
}

impl std::error::Error for RandaoError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(n: u8) -> Address {
        let mut addr = [0u8; 20];
        addr[0] = n;
        Address::new(addr)
    }

    fn test_hash(n: u8) -> Hash {
        let mut hash = [0u8; 32];
        hash[0] = n;
        hash
    }


    #[test]
    fn test_basic_mixing() {
        let config = RandaoConfig::default();
        let mixer = RandaoMixer::new(config, [0u8; 32]);

        // Mix first reveal
        mixer.mix_reveal(test_address(1), test_hash(1), 100).unwrap();
        let mix1 = mixer.current_mix();

        // Mix second reveal - should change mix
        mixer.mix_reveal(test_address(2), test_hash(2), 101).unwrap();
        let mix2 = mixer.current_mix();

        assert_ne!(mix1, mix2);
        assert_eq!(mixer.reveal_count(), 2);
    }

    #[test]
    fn test_duplicate_reveal_rejected() {
        let config = RandaoConfig::default();
        let mixer = RandaoMixer::new(config, [0u8; 32]);

        // First reveal succeeds
        mixer.mix_reveal(test_address(1), test_hash(1), 100).unwrap();

        // Second reveal from same validator fails
        let result = mixer.mix_reveal(test_address(1), test_hash(2), 101);
        assert!(matches!(result, Err(RandaoError::DuplicateReveal(_))));
    }

    #[test]
    fn test_epoch_finalization() {
        let config = RandaoConfig {
            min_reveals_for_epoch: 2,
            ..Default::default()
        };
        let mixer = RandaoMixer::new(config, [0u8; 32]);

        // Not enough reveals
        mixer.mix_reveal(test_address(1), test_hash(1), 100).unwrap();
        let result = mixer.finalize_epoch();
        assert!(matches!(result, Err(RandaoError::InsufficientReveals { .. })));

        // Add another reveal
        mixer.mix_reveal(test_address(2), test_hash(2), 101).unwrap();

        // Now finalization should succeed
        let final_mix = mixer.finalize_epoch().unwrap();
        assert_ne!(final_mix, [0u8; 32]);

        // Epoch advanced
        assert_eq!(mixer.current_epoch(), 1);

        // Reveals cleared
        assert_eq!(mixer.reveal_count(), 0);

        // Historical mix stored
        assert_eq!(mixer.get_epoch_mix(0), Some(final_mix));
    }

    #[test]
    fn test_mix_determinism() {
        let config = RandaoConfig::default();
        let mixer1 = RandaoMixer::new(config.clone(), [0u8; 32]);
        let mixer2 = RandaoMixer::new(config, [0u8; 32]);

        // Same reveals in same order = same result
        mixer1.mix_reveal(test_address(1), test_hash(1), 100).unwrap();
        mixer1.mix_reveal(test_address(2), test_hash(2), 101).unwrap();

        mixer2.mix_reveal(test_address(1), test_hash(1), 100).unwrap();
        mixer2.mix_reveal(test_address(2), test_hash(2), 101).unwrap();

        assert_eq!(mixer1.current_mix(), mixer2.current_mix());
    }
}
