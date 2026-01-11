use luxtensor_core::types::Address;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Validator information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Validator {
    /// Validator address
    pub address: Address,
    /// Amount of stake (in base units)
    pub stake: u128,
    /// Public key for signing (32 bytes - hash of full pubkey)
    pub public_key: [u8; 32],
    /// Is the validator active?
    pub active: bool,
    /// Accumulated rewards
    pub rewards: u128,
    /// Last slot this validator was active
    pub last_active_slot: u64,
}

impl Validator {
    pub fn new(address: Address, stake: u128, public_key: [u8; 32]) -> Self {
        Self {
            address,
            stake,
            public_key,
            active: true,
            rewards: 0,
            last_active_slot: 0,
        }
    }
}

/// Manages the set of validators
#[derive(Debug, Clone)]
pub struct ValidatorSet {
    validators: HashMap<Address, Validator>,
    total_stake: u128,
}

impl ValidatorSet {
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            total_stake: 0,
        }
    }

    /// Add a new validator to the set
    pub fn add_validator(&mut self, validator: Validator) -> Result<(), &'static str> {
        if self.validators.contains_key(&validator.address) {
            return Err("Validator already exists");
        }

        if validator.stake == 0 {
            return Err("Validator stake must be greater than 0");
        }

        self.total_stake += validator.stake;
        self.validators.insert(validator.address, validator);
        Ok(())
    }

    /// Remove a validator from the set
    pub fn remove_validator(&mut self, address: &Address) -> Result<(), &'static str> {
        if let Some(validator) = self.validators.remove(address) {
            self.total_stake -= validator.stake;
            Ok(())
        } else {
            Err("Validator not found")
        }
    }

    /// Update validator stake
    pub fn update_stake(&mut self, address: &Address, new_stake: u128) -> Result<(), &'static str> {
        if let Some(validator) = self.validators.get_mut(address) {
            self.total_stake = self.total_stake - validator.stake + new_stake;
            validator.stake = new_stake;
            Ok(())
        } else {
            Err("Validator not found")
        }
    }

    /// Slash stake from a validator (for slashing)
    pub fn slash_stake(&mut self, address: &Address, amount: u128) -> Result<u128, &'static str> {
        if let Some(validator) = self.validators.get_mut(address) {
            let slash_amount = amount.min(validator.stake);
            validator.stake -= slash_amount;
            self.total_stake -= slash_amount;
            Ok(slash_amount)
        } else {
            Err("Validator not found")
        }
    }

    /// Deactivate a validator (for jailing)
    pub fn deactivate_validator(&mut self, address: &Address) -> Result<(), &'static str> {
        if let Some(validator) = self.validators.get_mut(address) {
            validator.active = false;
            Ok(())
        } else {
            Err("Validator not found")
        }
    }

    /// Activate a validator (for unjailing)
    pub fn activate_validator(&mut self, address: &Address) -> Result<(), &'static str> {
        if let Some(validator) = self.validators.get_mut(address) {
            validator.active = true;
            Ok(())
        } else {
            Err("Validator not found")
        }
    }

    /// Update last active slot for validator
    pub fn update_last_active(&mut self, address: &Address, slot: u64) -> Result<(), &'static str> {
        if let Some(validator) = self.validators.get_mut(address) {
            validator.last_active_slot = slot;
            Ok(())
        } else {
            Err("Validator not found")
        }
    }

    /// Get a validator by address
    pub fn get_validator(&self, address: &Address) -> Option<&Validator> {
        self.validators.get(address)
    }

    /// Get all validators
    pub fn validators(&self) -> Vec<&Validator> {
        self.validators.values().collect()
    }

    /// Get active validators only
    pub fn active_validators(&self) -> Vec<&Validator> {
        self.validators
            .values()
            .filter(|v| v.active)
            .collect()
    }

    /// Get total stake
    pub fn total_stake(&self) -> u128 {
        self.total_stake
    }

    /// Get number of validators
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if validator set is empty
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Select a validator using weighted random selection based on seed
    pub fn select_by_seed(&self, seed: &[u8; 32]) -> Result<Address, &'static str> {
        let active = self.active_validators();
        if active.is_empty() {
            return Err("No active validators");
        }

        // Calculate active stake
        let active_stake: u128 = active.iter().map(|v| v.stake).sum();
        if active_stake == 0 {
            return Err("No stake in active validators");
        }

        // Convert seed to a number for weighted selection
        let mut seed_value: u128 = 0;
        for (i, &byte) in seed.iter().enumerate().take(16) {
            seed_value |= (byte as u128) << (i * 8);
        }

        // Perform weighted selection
        let target = seed_value % active_stake;
        let mut accumulated = 0u128;

        for validator in &active {
            accumulated += validator.stake;
            if accumulated > target {
                return Ok(validator.address);
            }
        }

        // Fallback to first validator (should not happen)
        Ok(active[0].address)
    }

    /// Add rewards to a validator
    pub fn add_reward(&mut self, address: &Address, amount: u128) -> Result<(), &'static str> {
        if let Some(validator) = self.validators.get_mut(address) {
            validator.rewards += amount;
            Ok(())
        } else {
            Err("Validator not found")
        }
    }
}

impl Default for ValidatorSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_validator(index: u8) -> Validator {
        let mut addr_bytes = [0u8; 20];
        addr_bytes[0] = index;
        let address = Address::from(addr_bytes);

        let mut pubkey = [0u8; 32];
        pubkey[0] = index;

        Validator::new(address, 1000u128, pubkey)
    }

    #[test]
    fn test_validator_set_creation() {
        let set = ValidatorSet::new();
        assert_eq!(set.len(), 0);
        assert_eq!(set.total_stake(), 0);
    }

    #[test]
    fn test_add_validator() {
        let mut set = ValidatorSet::new();
        let validator = create_test_validator(1);

        assert!(set.add_validator(validator.clone()).is_ok());
        assert_eq!(set.len(), 1);
        assert_eq!(set.total_stake(), 1000);
    }

    #[test]
    fn test_add_duplicate_validator() {
        let mut set = ValidatorSet::new();
        let validator = create_test_validator(1);

        assert!(set.add_validator(validator.clone()).is_ok());
        assert!(set.add_validator(validator).is_err());
    }

    #[test]
    fn test_remove_validator() {
        let mut set = ValidatorSet::new();
        let validator = create_test_validator(1);
        let address = validator.address;

        set.add_validator(validator).unwrap();
        assert_eq!(set.len(), 1);

        assert!(set.remove_validator(&address).is_ok());
        assert_eq!(set.len(), 0);
        assert_eq!(set.total_stake(), 0);
    }

    #[test]
    fn test_update_stake() {
        let mut set = ValidatorSet::new();
        let validator = create_test_validator(1);
        let address = validator.address;

        set.add_validator(validator).unwrap();
        assert_eq!(set.total_stake(), 1000);

        set.update_stake(&address, 2000).unwrap();
        assert_eq!(set.total_stake(), 2000);
    }

    #[test]
    fn test_select_by_seed() {
        let mut set = ValidatorSet::new();

        // Add 3 validators with different stakes
        for i in 1..=3 {
            let mut validator = create_test_validator(i);
            validator.stake = (i as u128) * 1000;
            set.add_validator(validator).unwrap();
        }

        // Selection should work
        let seed = [0u8; 32];
        let selected = set.select_by_seed(&seed);
        assert!(selected.is_ok());
    }

    #[test]
    fn test_add_reward() {
        let mut set = ValidatorSet::new();
        let validator = create_test_validator(1);
        let address = validator.address;

        set.add_validator(validator).unwrap();
        set.add_reward(&address, 100).unwrap();

        let v = set.get_validator(&address).unwrap();
        assert_eq!(v.rewards, 100);
    }
}
