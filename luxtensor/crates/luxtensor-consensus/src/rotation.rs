// Validator rotation and management system
// Implements automatic validator set updates based on epochs

use crate::error::ConsensusError;
use crate::validator::{Validator, ValidatorSet};
use luxtensor_core::types::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

/// Validator rotation manager
pub struct ValidatorRotation {
    /// Current validator set
    current_validators: ValidatorSet,
    /// Pending validators waiting to join
    pending_validators: HashMap<Address, PendingValidator>,
    /// Validators scheduled to exit: address -> exit_epoch
    exiting_validators: HashMap<Address, u64>,
    /// Rotation configuration
    config: RotationConfig,
    /// Current epoch
    current_epoch: u64,
}

/// Configuration for validator rotation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationConfig {
    /// Number of slots per epoch
    pub epoch_length: u64,
    /// Minimum epochs a validator must wait before joining
    pub activation_delay_epochs: u64,
    /// Minimum epochs a validator must wait before exiting
    pub exit_delay_epochs: u64,
    /// Maximum number of validators in the active set
    pub max_validators: usize,
    /// Minimum stake required to become a validator
    pub min_stake: u128,
}

impl Default for RotationConfig {
    fn default() -> Self {
        Self {
            epoch_length: 32,
            activation_delay_epochs: 2,
            exit_delay_epochs: 2,
            max_validators: 100,
            min_stake: 32_000_000_000_000_000_000u128, // 32 tokens
        }
    }
}

/// A pending validator waiting to join
#[derive(Debug, Clone)]
pub struct PendingValidator {
    pub validator: Validator,
    pub activation_epoch: u64,
}

/// Result of epoch transition
#[derive(Debug, Clone)]
pub struct EpochTransitionResult {
    pub activated_validators: Vec<Address>,
    pub exited_validators: Vec<Address>,
    pub new_epoch: u64,
}

impl ValidatorRotation {
    /// Create a new validator rotation manager
    pub fn new(config: RotationConfig) -> Self {
        Self {
            current_validators: ValidatorSet::new(),
            pending_validators: HashMap::new(),
            exiting_validators: HashMap::new(),
            config,
            current_epoch: 0,
        }
    }

    /// Create with existing validator set
    pub fn with_validators(config: RotationConfig, validators: ValidatorSet) -> Self {
        Self {
            current_validators: validators,
            pending_validators: HashMap::new(),
            exiting_validators: HashMap::new(),
            config,
            current_epoch: 0,
        }
    }

    /// Get current validator set
    pub fn current_validators(&self) -> &ValidatorSet {
        &self.current_validators
    }

    /// Request to add a new validator
    pub fn request_validator_addition(
        &mut self,
        validator: Validator,
    ) -> Result<u64, ConsensusError> {
        // Validate minimum stake
        if validator.stake < self.config.min_stake {
            return Err(ConsensusError::InsufficientStake {
                provided: validator.stake,
                required: self.config.min_stake,
            });
        }

        // Check if validator already exists
        if self.current_validators.get_validator(&validator.address).is_some() {
            return Err(ConsensusError::ValidatorAlreadyExists(validator.address));
        }

        // Check if already pending
        if self.pending_validators.contains_key(&validator.address) {
            return Err(ConsensusError::ValidatorAlreadyExists(validator.address));
        }

        // Calculate activation epoch
        let activation_epoch = self.current_epoch + self.config.activation_delay_epochs;

        info!(
            "Validator {} requested to join, activation at epoch {}",
            hex::encode(&validator.address),
            activation_epoch
        );

        self.pending_validators.insert(
            validator.address,
            PendingValidator {
                validator,
                activation_epoch,
            },
        );

        Ok(activation_epoch)
    }

    /// Request validator exit
    pub fn request_validator_exit(&mut self, address: Address) -> Result<u64, ConsensusError> {
        // Check if validator exists
        if self.current_validators.get_validator(&address).is_none() {
            return Err(ConsensusError::ValidatorNotFound(format!("{:?}", address)));
        }

        // Check if already scheduled to exit
        if self.exiting_validators.contains_key(&address) {
            return Err(ConsensusError::InvalidOperation(
                "Validator already scheduled to exit".to_string(),
            ));
        }

        let exit_epoch = self.current_epoch + self.config.exit_delay_epochs;

        info!(
            "Validator {} requested to exit, exit at epoch {}",
            hex::encode(&address),
            exit_epoch
        );

        self.exiting_validators.insert(address, exit_epoch);

        Ok(exit_epoch)
    }

    /// Process epoch transition
    /// Refactored: Split into helper functions for lower complexity
    pub fn process_epoch_transition(&mut self, new_epoch: u64) -> EpochTransitionResult {
        self.current_epoch = new_epoch;

        // Process pending activations (extracted helper)
        let activated = self.activate_pending_validators(new_epoch);

        // Process exits (extracted helper)
        let exited = self.process_validator_exits(new_epoch);

        EpochTransitionResult {
            activated_validators: activated,
            exited_validators: exited,
            new_epoch,
        }
    }

    /// Activate validators whose activation epoch has arrived
    fn activate_pending_validators(&mut self, new_epoch: u64) -> Vec<Address> {
        let mut activated = Vec::new();

        // Collect ready validators
        let ready_to_activate: Vec<Address> = self
            .pending_validators
            .iter()
            .filter(|(_, pending)| pending.activation_epoch <= new_epoch)
            .map(|(addr, _)| *addr)
            .collect();

        for address in ready_to_activate {
            if let Some(pending) = self.pending_validators.remove(&address) {
                if self.current_validators.len() < self.config.max_validators {
                    if let Err(e) = self.current_validators.add_validator(pending.validator) {
                        warn!(
                            "Failed to activate validator {}: {}",
                            hex::encode(&address), e
                        );
                        continue;
                    }
                    activated.push(address);
                    info!(
                        "Activated validator {} at epoch {}",
                        hex::encode(&address),
                        new_epoch
                    );
                } else {
                    warn!(
                        "Cannot activate validator {}, max validator count reached",
                        hex::encode(&address)
                    );
                    // Re-queue for next epoch
                    self.pending_validators.insert(
                        address,
                        PendingValidator {
                            validator: pending.validator,
                            activation_epoch: new_epoch + 1,
                        },
                    );
                }
            }
        }

        activated
    }

    /// Process validators scheduled for exit
    ///
    /// SECURITY: Only removes validators whose exit delay has elapsed.
    /// Each validator's exit_epoch was set when they requested exit:
    ///   exit_epoch = request_epoch + exit_delay_epochs
    /// Only validators with exit_epoch <= new_epoch are actually removed.
    fn process_validator_exits(&mut self, new_epoch: u64) -> Vec<Address> {
        let mut exited = Vec::new();

        // Collect validators whose exit epoch has arrived
        let ready_to_exit: Vec<Address> = self
            .exiting_validators
            .iter()
            .filter(|(_, exit_epoch)| **exit_epoch <= new_epoch)
            .map(|(addr, _)| *addr)
            .collect();

        for address in ready_to_exit {
            if let Err(e) = self.current_validators.remove_validator(&address) {
                warn!(
                    "Failed to remove exiting validator {}: {}",
                    hex::encode(&address), e
                );
            }
            self.exiting_validators.remove(&address);
            exited.push(address);
            info!(
                "Exited validator {} at epoch {}",
                hex::encode(&address),
                new_epoch,
            );
        }

        exited
    }


    /// Get pending validator count
    pub fn pending_count(&self) -> usize {
        self.pending_validators.len()
    }

    /// Get exiting validator count
    pub fn exiting_count(&self) -> usize {
        self.exiting_validators.len()
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Get rotation statistics
    pub fn get_stats(&self) -> RotationStats {
        RotationStats {
            current_epoch: self.current_epoch,
            active_validators: self.current_validators.len(),
            pending_validators: self.pending_validators.len(),
            exiting_validators: self.exiting_validators.len(),
            total_stake: self.current_validators.total_stake(),
        }
    }

    /// Slash a validator for misbehavior
    pub fn slash_validator(
        &mut self,
        address: &Address,
        slash_amount: u128,
    ) -> Result<(), ConsensusError> {
        // Get validator
        let validator = self
            .current_validators
            .get_validator(address)
            .ok_or(ConsensusError::ValidatorNotFound(format!("{:?}", address)))?;

        let new_stake = validator
            .stake
            .checked_sub(slash_amount)
            .ok_or(ConsensusError::InvalidOperation(
                "Slash amount exceeds stake".to_string(),
            ))?;

        warn!(
            "Slashing validator {} by {}, new stake: {}",
            hex::encode(address),
            slash_amount,
            new_stake
        );

        // Update stake
        let mut updated_validator = validator.clone();
        updated_validator.stake = new_stake;

        if let Err(e) = self.current_validators.remove_validator(address) {
            warn!(
                "Failed to remove validator {} during slash: {}",
                hex::encode(address), e
            );
            return Err(ConsensusError::InvalidOperation(
                format!("Failed to remove validator for stake update: {}", e)
            ));
        }
        if let Err(e) = self.current_validators.add_validator(updated_validator) {
            warn!(
                "Failed to re-add validator {} after slash: {}",
                hex::encode(address), e
            );
            return Err(ConsensusError::InvalidOperation(
                format!("Failed to re-add validator after slash: {}", e)
            ));
        }

        // If stake falls below minimum, schedule for exit
        if new_stake < self.config.min_stake {
            warn!(
                "Validator {} stake below minimum, scheduling exit",
                hex::encode(address)
            );
            let exit_epoch = self.current_epoch + self.config.exit_delay_epochs;
            self.exiting_validators.insert(*address, exit_epoch);
        }

        Ok(())
    }
}

/// Validator rotation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationStats {
    pub current_epoch: u64,
    pub active_validators: usize,
    pub pending_validators: usize,
    pub exiting_validators: usize,
    pub total_stake: u128,
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_crypto::KeyPair;

    fn create_test_validator(stake: u128) -> Validator {
        let keypair = KeyPair::generate();
        let mut public_key = [0u8; 32];
        let pk_bytes = keypair.public_key_bytes();
        public_key.copy_from_slice(&pk_bytes[..32.min(pk_bytes.len())]);

        Validator {
            address: Address::from(keypair.address()),
            stake,
            public_key,
            active: true,
            rewards: 0,
            last_active_slot: 0,
            activation_epoch: 0,
        }
    }

    #[test]
    fn test_validator_rotation_creation() {
        let config = RotationConfig::default();
        let rotation = ValidatorRotation::new(config);

        assert_eq!(rotation.current_epoch(), 0);
        assert_eq!(rotation.pending_count(), 0);
        assert_eq!(rotation.exiting_count(), 0);
    }

    #[test]
    fn test_request_validator_addition() {
        let config = RotationConfig::default();
        let mut rotation = ValidatorRotation::new(config.clone());

        let validator = create_test_validator(config.min_stake);
        let activation_epoch = rotation.request_validator_addition(validator).unwrap();

        assert_eq!(activation_epoch, config.activation_delay_epochs);
        assert_eq!(rotation.pending_count(), 1);
    }

    #[test]
    fn test_request_validator_addition_insufficient_stake() {
        let config = RotationConfig::default();
        let mut rotation = ValidatorRotation::new(config.clone());

        let validator = create_test_validator(config.min_stake - 1);
        let result = rotation.request_validator_addition(validator);

        assert!(result.is_err());
    }

    #[test]
    fn test_process_epoch_transition_activation() {
        let config = RotationConfig::default();
        let mut rotation = ValidatorRotation::new(config.clone());

        let validator = create_test_validator(config.min_stake);
        rotation.request_validator_addition(validator).unwrap();

        // Advance to activation epoch
        let result = rotation.process_epoch_transition(config.activation_delay_epochs);

        assert_eq!(result.activated_validators.len(), 1);
        assert_eq!(rotation.current_validators().len(), 1);
        assert_eq!(rotation.pending_count(), 0);
    }

    #[test]
    fn test_request_validator_exit() {
        let config = RotationConfig::default();
        let validator = create_test_validator(config.min_stake);
        let address = validator.address;

        let mut validator_set = ValidatorSet::new();
        validator_set.add_validator(validator).unwrap();

        let mut rotation = ValidatorRotation::with_validators(config.clone(), validator_set);

        let exit_epoch = rotation.request_validator_exit(address).unwrap();

        assert_eq!(exit_epoch, config.exit_delay_epochs);
        assert_eq!(rotation.exiting_count(), 1);
    }

    #[test]
    fn test_slash_validator() {
        let config = RotationConfig::default();
        let validator = create_test_validator(config.min_stake * 2);
        let address = validator.address;

        let mut validator_set = ValidatorSet::new();
        validator_set.add_validator(validator).unwrap();

        let mut rotation = ValidatorRotation::with_validators(config.clone(), validator_set);

        let slash_amount = config.min_stake / 2;
        rotation.slash_validator(&address, slash_amount).unwrap();

        let validator = rotation.current_validators().get_validator(&address).unwrap();
        assert_eq!(validator.stake, config.min_stake * 2 - slash_amount);
    }

    #[test]
    fn test_slash_validator_below_minimum() {
        let config = RotationConfig::default();
        let validator = create_test_validator(config.min_stake);
        let address = validator.address;

        let mut validator_set = ValidatorSet::new();
        validator_set.add_validator(validator).unwrap();

        let mut rotation = ValidatorRotation::with_validators(config.clone(), validator_set);

        let slash_amount = 1;
        rotation.slash_validator(&address, slash_amount).unwrap();

        // Should be scheduled for exit
        assert_eq!(rotation.exiting_count(), 1);
    }

    #[test]
    fn test_get_stats() {
        let config = RotationConfig::default();
        let validator = create_test_validator(config.min_stake);

        let mut validator_set = ValidatorSet::new();
        validator_set.add_validator(validator).unwrap();

        let rotation = ValidatorRotation::with_validators(config, validator_set);
        let stats = rotation.get_stats();

        assert_eq!(stats.active_validators, 1);
        assert_eq!(stats.pending_validators, 0);
        assert_eq!(stats.exiting_validators, 0);
    }
}
