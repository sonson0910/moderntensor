//! Consensus Security Tests
//!
//! Tests for validator selection, epoch transitions, slashing, and basic consensus flows.
//! Updated to match current API

use luxtensor_consensus::{ProofOfStake, ConsensusConfig, SlashReason, HalvingSchedule};
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
            ..Default::default()
        };
        ProofOfStake::new(config)
    }

    /// Test validator registration via add_validator
    #[test]
    fn test_validator_registration() {
        let consensus = create_test_consensus();
        let validator_addr: Address = Address::new([1u8; 20]);
        let stake = 10_000_000_000_000_000_000u128; // 10 tokens
        let public_key = [0u8; 32];

        let result = consensus.add_validator(validator_addr, stake, public_key);
        assert!(result.is_ok(), "Validator registration should succeed");
        assert_eq!(consensus.validator_count(), 1);
    }

    /// Test minimum stake requirement
    #[test]
    fn test_minimum_stake_requirement() {
        let consensus = create_test_consensus();
        let validator_addr: Address = Address::new([1u8; 20]);
        let low_stake = 1_000u128; // Much less than min_stake
        let public_key = [0u8; 32];

        let result = consensus.add_validator(validator_addr, low_stake, public_key);
        assert!(result.is_err(), "Registration with low stake should fail");
    }

    /// Test validator selection is deterministic
    #[test]
    fn test_validator_selection_deterministic() {
        let consensus = create_test_consensus();
        let public_key = [0u8; 32];

        // Register multiple validators
        for i in 0..5u8 {
            let addr = Address::new([i; 20]);
            consensus.add_validator(addr, 10_000_000_000_000_000_000, public_key).ok();
        }

        // Selection for same slot should be deterministic
        let slot = 100;
        let selected1 = consensus.select_validator(slot);
        let selected2 = consensus.select_validator(slot);

        // Both should return same Result
        assert!(selected1.is_ok() == selected2.is_ok(), "Validator selection should be deterministic");
        if let (Ok(addr1), Ok(addr2)) = (selected1, selected2) {
            assert_eq!(addr1, addr2);
        }
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
            ..Default::default()
        };
        ProofOfStake::new(config)
    }

    /// Test epoch advancement
    #[test]
    fn test_epoch_advancement() {
        let consensus = create_test_consensus();

        assert_eq!(consensus.current_epoch(), 0);

        consensus.advance_epoch();
        assert_eq!(consensus.current_epoch(), 1);

        consensus.advance_epoch();
        assert_eq!(consensus.current_epoch(), 2);
    }

    /// Test slot calculation
    #[test]
    fn test_slot_calculation() {
        let consensus = create_test_consensus();
        let genesis_time = 1000;

        // slot_duration = 3
        assert_eq!(consensus.get_slot(1000, genesis_time), 0);
        assert_eq!(consensus.get_slot(1003, genesis_time), 1);
        assert_eq!(consensus.get_slot(1030, genesis_time), 10);
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
            ..Default::default()
        };
        ProofOfStake::new(config)
    }

    /// Test that SlashReason enum is available
    #[test]
    fn test_slash_reason_exists() {
        // Verify correct variant names
        let _reason = SlashReason::DoubleSigning;
        let _reason2 = SlashReason::Offline;
        let _reason3 = SlashReason::InvalidBlock;
    }

    /// Test validator stake update (reduction)
    #[test]
    fn test_validator_stake_update() {
        let consensus = create_test_consensus();
        let validator_addr = Address::new([1u8; 20]);
        let initial_stake = 10_000_000_000_000_000_000u128;
        let public_key = [0u8; 32];

        consensus.add_validator(validator_addr, initial_stake, public_key).unwrap();

        // Reduce stake (simulate slashing)
        let reduced_stake = initial_stake * 90 / 100; // 90% remaining
        let result = consensus.update_validator_stake(&validator_addr, reduced_stake);

        assert!(result.is_ok(), "Stake update should succeed");
    }
}

#[cfg(test)]
mod reward_tests {
    use super::*;

    fn create_test_consensus() -> ProofOfStake {
        let config = ConsensusConfig {
            slot_duration: 3,
            min_stake: 1_000_000_000_000_000_000,
            block_reward: 2_000_000_000_000_000_000,
            epoch_length: 10,
            halving_schedule: HalvingSchedule::default(),
        };
        ProofOfStake::new(config)
    }

    /// Test reward calculation with halving
    #[test]
    fn test_reward_for_height() {
        let consensus = create_test_consensus();

        // Early block - should get full reward
        let reward_0 = consensus.get_reward_for_height(0);
        assert!(reward_0 > 0, "Reward should be positive");

        // Much later block - may be reduced due to halving
        let reward_far = consensus.get_reward_for_height(10_000_000);
        assert!(reward_far <= reward_0, "Reward should decrease or stay same with halving");
    }

    /// Test halving info returns valid data
    #[test]
    fn test_halving_info() {
        let consensus = create_test_consensus();

        let info = consensus.get_halving_info(0);
        // Check that info has valid fields
        assert!(info.initial_reward_mdt > 0.0);
        assert!(info.halving_interval_blocks > 0);
    }

    /// Test halving status
    #[test]
    fn test_halving_status() {
        let consensus = create_test_consensus();

        let (era, blocks_until, reward) = consensus.get_halving_status(0);
        assert_eq!(era, 0, "Era should be 0 at genesis");
        assert!(blocks_until > 0, "Should have blocks until next halving");
        assert!(reward > 0, "Reward should be positive");
    }
}

#[cfg(test)]
mod validator_set_tests {
    use super::*;

    fn create_test_consensus() -> ProofOfStake {
        // For unit tests, use a low min_stake (1 LUX = 10^18) so that
        // the test stakes (100 tokens = 100 * 10^18) pass validation.
        // ConsensusConfig::default() uses the production MIN_STAKE (10^24) which
        // would require 1,000,000 MDT per validator - too large for unit tests.
        let config = ConsensusConfig {
            min_stake: 1_000_000_000_000_000_000u128, // 1 LUX (18 decimals)
            ..ConsensusConfig::default()
        };
        ProofOfStake::new(config)
    }

    /// Test adding and removing validators
    #[test]
    fn test_add_remove_validator() {
        let consensus = create_test_consensus();
        let addr = Address::new([1u8; 20]);
        let stake = 100_000_000_000_000_000_000u128; // 100 tokens
        let public_key = [0u8; 32];

        // Add
        consensus.add_validator(addr, stake, public_key).unwrap();
        assert_eq!(consensus.validator_count(), 1);

        // Remove
        consensus.remove_validator(&addr).unwrap();
        assert_eq!(consensus.validator_count(), 0);
    }

    /// Test total stake calculation
    #[test]
    fn test_total_stake() {
        let consensus = create_test_consensus();
        let stake = 100_000_000_000_000_000_000u128; // 100 tokens
        let public_key = [0u8; 32];

        consensus.add_validator(Address::new([1u8; 20]), stake, public_key).unwrap();
        consensus.add_validator(Address::new([2u8; 20]), stake, public_key).unwrap();

        assert_eq!(consensus.total_stake(), stake * 2);
    }

    /// Test duplicate validator rejected
    #[test]
    fn test_duplicate_validator_rejected() {
        let consensus = create_test_consensus();
        let addr = Address::new([1u8; 20]);
        let stake = 100_000_000_000_000_000_000u128; // 100 tokens
        let public_key = [0u8; 32];

        consensus.add_validator(addr, stake, public_key).unwrap();

        let result = consensus.add_validator(addr, stake + 1000, public_key);
        assert!(result.is_err(), "Duplicate validator should be rejected");
    }
}
