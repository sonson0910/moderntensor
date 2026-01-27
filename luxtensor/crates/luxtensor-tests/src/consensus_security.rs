//! Consensus Security Tests
//!
//! Tests for validator selection, epoch transitions, slashing, and jail/unjail flows.

use luxtensor_consensus::{ProofOfStake, ConsensusConfig, SlashingReason};
use luxtensor_core::Address;

#[cfg(test)]
mod validator_selection_tests {
    use super::*;

    fn create_test_consensus() -> ProofOfStake {
        let config = ConsensusConfig {
            slot_duration: 3,
            min_stake: 1_000_000_000_000_000_000, // 1 token
            block_reward: 2_000_000_000_000_000_000, // 2 tokens
            epoch_length: 10,
        };
        ProofOfStake::new(config)
    }

    /// Test validator registration
    #[test]
    fn test_validator_registration() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let stake = 10_000_000_000_000_000_000u128; // 10 tokens

        let result = consensus.register_validator(validator_addr, stake);
        assert!(result.is_ok(), "Validator registration should succeed");
        assert_eq!(consensus.validator_count(), 1);
    }

    /// Test minimum stake requirement
    #[test]
    fn test_minimum_stake_requirement() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let low_stake = 1_000u128; // Much less than min_stake

        let result = consensus.register_validator(validator_addr, low_stake);
        assert!(result.is_err(), "Registration with low stake should fail");
    }

    /// Test validator selection is deterministic
    #[test]
    fn test_validator_selection_deterministic() {
        let mut consensus = create_test_consensus();

        // Register multiple validators
        for i in 0..5 {
            let addr: Address = [i as u8; 20];
            consensus.register_validator(addr, 10_000_000_000_000_000_000).ok();
        }

        // Selection for same slot should be deterministic
        let slot = 100;
        let selected1 = consensus.get_slot_leader(slot);
        let selected2 = consensus.get_slot_leader(slot);

        assert_eq!(selected1, selected2, "Validator selection should be deterministic");
    }
}

#[cfg(test)]
mod epoch_transition_tests {
    use super::*;

    fn create_test_consensus() -> ProofOfStake {
        let config = ConsensusConfig {
            slot_duration: 3,
            min_stake: 1_000_000_000_000_000_000,
            block_reward: 2_000_000_000_000_000_000,
            epoch_length: 10,
        };
        ProofOfStake::new(config)
    }

    /// Test epoch boundary detection
    #[test]
    fn test_epoch_boundary() {
        let consensus = create_test_consensus();

        // Block 10 should be epoch boundary (epoch_length = 10)
        assert!(consensus.is_epoch_boundary(10));
        assert!(consensus.is_epoch_boundary(20));
        assert!(consensus.is_epoch_boundary(100));

        // These should not be boundaries
        assert!(!consensus.is_epoch_boundary(5));
        assert!(!consensus.is_epoch_boundary(15));
        assert!(!consensus.is_epoch_boundary(99));
    }

    /// Test epoch number calculation
    #[test]
    fn test_epoch_number() {
        let consensus = create_test_consensus();

        assert_eq!(consensus.get_epoch(0), 0);
        assert_eq!(consensus.get_epoch(9), 0);
        assert_eq!(consensus.get_epoch(10), 1);
        assert_eq!(consensus.get_epoch(25), 2);
        assert_eq!(consensus.get_epoch(100), 10);
    }
}

#[cfg(test)]
mod slashing_tests {
    use super::*;

    fn create_test_consensus() -> ProofOfStake {
        let config = ConsensusConfig {
            slot_duration: 3,
            min_stake: 1_000_000_000_000_000_000,
            block_reward: 2_000_000_000_000_000_000,
            epoch_length: 10,
        };
        ProofOfStake::new(config)
    }

    /// Test slashing reduces stake
    #[test]
    fn test_slashing_reduces_stake() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let initial_stake = 10_000_000_000_000_000_000u128;

        consensus.register_validator(validator_addr, initial_stake).unwrap();

        // Slash the validator
        let slash_result = consensus.slash_validator(
            validator_addr,
            SlashingReason::DoubleSign,
        );

        assert!(slash_result.is_ok(), "Slashing should succeed");

        // Verify stake was reduced
        let remaining_stake = consensus.get_validator_stake(validator_addr);
        assert!(remaining_stake < initial_stake, "Stake should be reduced after slashing");
    }

    /// Test double signing penalty is severe
    #[test]
    fn test_double_sign_severe_penalty() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let initial_stake = 10_000_000_000_000_000_000u128;

        consensus.register_validator(validator_addr, initial_stake).unwrap();

        consensus.slash_validator(validator_addr, SlashingReason::DoubleSign).unwrap();

        let remaining = consensus.get_validator_stake(validator_addr);
        // Double sign should slash at least 10% (configurable)
        assert!(remaining <= initial_stake * 90 / 100, "Double sign should slash >= 10%");
    }
}

#[cfg(test)]
mod jail_unjail_tests {
    use super::*;

    fn create_test_consensus() -> ProofOfStake {
        let config = ConsensusConfig {
            slot_duration: 3,
            min_stake: 1_000_000_000_000_000_000,
            block_reward: 2_000_000_000_000_000_000,
            epoch_length: 10,
        };
        ProofOfStake::new(config)
    }

    /// Test jail prevents validator selection
    #[test]
    fn test_jailed_validator_not_selected() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let initial_stake = 10_000_000_000_000_000_000u128;

        consensus.register_validator(validator_addr, initial_stake).unwrap();

        // Jail the validator
        consensus.jail_validator(validator_addr, 100).ok(); // Jailed for 100 blocks

        // Jailed validator should not be in active set
        let active = consensus.get_active_validators();
        assert!(!active.contains(&validator_addr), "Jailed validator should not be active");
    }

    /// Test unjail after jail period
    #[test]
    fn test_unjail_after_period() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let initial_stake = 10_000_000_000_000_000_000u128;

        consensus.register_validator(validator_addr, initial_stake).unwrap();
        consensus.jail_validator(validator_addr, 10).ok(); // Jailed for 10 blocks

        // Simulate passing jail period
        for _ in 0..10 {
            consensus.process_block(0);
        }

        // Unjail
        let result = consensus.unjail_validator(validator_addr);
        assert!(result.is_ok(), "Unjail should succeed after jail period");
    }

    /// Test cannot unjail before period ends
    #[test]
    fn test_cannot_unjail_early() {
        let mut consensus = create_test_consensus();
        let validator_addr: Address = [1u8; 20];
        let initial_stake = 10_000_000_000_000_000_000u128;

        consensus.register_validator(validator_addr, initial_stake).unwrap();
        consensus.jail_validator(validator_addr, 100).ok(); // Jailed for 100 blocks

        // Try to unjail immediately
        let result = consensus.unjail_validator(validator_addr);
        assert!(result.is_err(), "Should not be able to unjail before period ends");
    }
}
