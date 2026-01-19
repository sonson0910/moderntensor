// Comprehensive Tests for Stake and Reward System
// Tests for reward distribution, slashing, emission, and token allocation

use luxtensor_consensus::{
    DistributionConfig, LockBonusConfig, RewardDistributor,
    MinerInfo, ValidatorInfo, DelegatorInfo, SubnetInfo,
    RewardExecutor, UtilityMetrics,
};

// ============================================================
// Test Helpers
// ============================================================

fn test_address(id: u8) -> [u8; 20] {
    let mut addr = [0u8; 20];
    addr[0] = id;
    addr
}

fn test_utility() -> UtilityMetrics {
    UtilityMetrics {
        active_validators: 10,
        active_subnets: 5,
        epoch_transactions: 100,
        epoch_ai_tasks: 50,
        block_utilization: 50,
    }
}

// ============================================================
// DistributionConfig Tests
// ============================================================

#[cfg(test)]
mod distribution_config_tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = DistributionConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_shares_sum_to_one() {
        let config = DistributionConfig::default();
        let total = config.miner_share + config.validator_share +
                   config.infrastructure_share + config.delegator_share +
                   config.subnet_owner_share + config.dao_share;
        assert!((total - 1.0).abs() < 0.001, "Shares should sum to 1.0, got {}", total);
    }

    #[test]
    fn test_model_c_percentages() {
        let config = DistributionConfig::default();
        assert_eq!(config.miner_share, 0.35, "Miners should get 35%");
        assert_eq!(config.validator_share, 0.28, "Validators should get 28%");
        assert_eq!(config.delegator_share, 0.12, "Delegators should get 12%");
        assert_eq!(config.subnet_owner_share, 0.10, "Subnet owners should get 10%");
        assert_eq!(config.dao_share, 0.13, "DAO should get 13%");
        assert_eq!(config.infrastructure_share, 0.02, "Infrastructure should get 2%");
    }
}

// ============================================================
// Lock Bonus Tests
// ============================================================

#[cfg(test)]
mod lock_bonus_tests {
    use super::*;

    #[test]
    fn test_no_lock_no_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus(0), 0.0);
        assert_eq!(config.get_bonus(1), 0.0);
        assert_eq!(config.get_bonus(29), 0.0);
    }

    #[test]
    fn test_30_day_lock_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus(30), 0.10, "30 days should get +10%");
        assert_eq!(config.get_bonus(45), 0.10, "45 days should get +10%");
        assert_eq!(config.get_bonus(89), 0.10, "89 days should get +10%");
    }

    #[test]
    fn test_90_day_lock_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus(90), 0.25, "90 days should get +25%");
        assert_eq!(config.get_bonus(120), 0.25, "120 days should get +25%");
        assert_eq!(config.get_bonus(179), 0.25, "179 days should get +25%");
    }

    #[test]
    fn test_180_day_lock_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus(180), 0.50, "180 days should get +50%");
        assert_eq!(config.get_bonus(300), 0.50, "300 days should get +50%");
        assert_eq!(config.get_bonus(364), 0.50, "364 days should get +50%");
    }

    #[test]
    fn test_365_day_lock_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus(365), 1.00, "365 days should get +100%");
        assert_eq!(config.get_bonus(730), 1.00, "730 days should get +100%");
    }
}

// ============================================================
// Reward Distribution Edge Cases
// ============================================================

#[cfg(test)]
mod distribution_edge_cases {
    use super::*;

    #[test]
    fn test_no_miners() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let total_emission: u128 = 1_000_000_000_000_000_000;
        let result = distributor.distribute(1, total_emission, &[], &[], &[], &[]);

        // Miner rewards should be empty
        assert!(result.miner_rewards.is_empty());
        // DAO should still receive 13%
        assert!(result.dao_allocation > 0);
    }

    #[test]
    fn test_no_validators() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let miners = vec![MinerInfo { address: test_address(1), score: 1.0 }];
        let result = distributor.distribute(1, 1_000_000_000, &miners, &[], &[], &[]);

        // Validator rewards should be empty
        assert!(result.validator_rewards.is_empty());
        // Miner should still receive rewards
        assert!(!result.miner_rewards.is_empty());
    }

    #[test]
    fn test_single_miner_gets_full_pool() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let total: u128 = 1_000_000_000;
        let miners = vec![MinerInfo { address: test_address(1), score: 1.0 }];
        let result = distributor.distribute(1, total, &miners, &[], &[], &[]);

        let miner_reward = *result.miner_rewards.get(&test_address(1)).unwrap();
        let expected = (total as f64 * 0.35) as u128;
        assert_eq!(miner_reward, expected, "Single miner should get full 35%");
    }

    #[test]
    fn test_equal_validators_equal_rewards() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let validators = vec![
            ValidatorInfo { address: test_address(1), stake: 1000 },
            ValidatorInfo { address: test_address(2), stake: 1000 },
        ];
        let result = distributor.distribute(1, 1_000_000, &[], &validators, &[], &[]);

        let v1_reward = *result.validator_rewards.get(&test_address(1)).unwrap_or(&0);
        let v2_reward = *result.validator_rewards.get(&test_address(2)).unwrap_or(&0);

        assert_eq!(v1_reward, v2_reward, "Equal stake should get equal rewards");
    }

    #[test]
    fn test_higher_stake_gets_more() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let validators = vec![
            ValidatorInfo { address: test_address(1), stake: 1000 },
            ValidatorInfo { address: test_address(2), stake: 3000 }, // 3x stake
        ];
        let result = distributor.distribute(1, 1_000_000, &[], &validators, &[], &[]);

        let v1_reward = *result.validator_rewards.get(&test_address(1)).unwrap_or(&0);
        let v2_reward = *result.validator_rewards.get(&test_address(2)).unwrap_or(&0);

        assert!(v2_reward > v1_reward, "Higher stake should get more rewards");
        // v2 has 3x stake, should get ~3x rewards
        let ratio = v2_reward as f64 / v1_reward as f64;
        assert!((ratio - 3.0).abs() < 0.1, "3x stake should get ~3x rewards, got {}x", ratio);
    }

    #[test]
    fn test_zero_stake_validator_no_reward() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let validators = vec![
            ValidatorInfo { address: test_address(1), stake: 0 },
            ValidatorInfo { address: test_address(2), stake: 1000 },
        ];
        let result = distributor.distribute(1, 1_000_000, &[], &validators, &[], &[]);

        let v1_reward = *result.validator_rewards.get(&test_address(1)).unwrap_or(&0);
        assert_eq!(v1_reward, 0, "Zero stake should get zero rewards");
    }

    #[test]
    fn test_delegator_lock_bonus_applied() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        // Same stake, different lock periods
        let delegators = vec![
            DelegatorInfo { address: test_address(1), stake: 1000, lock_days: 0 },
            DelegatorInfo { address: test_address(2), stake: 1000, lock_days: 365 },
        ];
        let result = distributor.distribute(1, 1_000_000_000, &[], &[], &delegators, &[]);

        let d1_reward = *result.delegator_rewards.get(&test_address(1)).unwrap_or(&0);
        let d2_reward = *result.delegator_rewards.get(&test_address(2)).unwrap_or(&0);

        // 365-day lock gets +100% bonus, so should get ~2x rewards
        assert!(d2_reward > d1_reward, "365-day lock should get more");
        let ratio = d2_reward as f64 / d1_reward as f64;
        assert!((ratio - 2.0).abs() < 0.1, "365-day lock should get ~2x, got {}x", ratio);
    }

    #[test]
    fn test_many_participants() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        // Create 100 miners with varying scores
        let miners: Vec<MinerInfo> = (1..=100)
            .map(|i| MinerInfo {
                address: { let mut a = [0u8; 20]; a[0] = i; a },
                score: (i as f64) / 100.0
            })
            .collect();

        // Create 50 validators
        let validators: Vec<ValidatorInfo> = (1..=50)
            .map(|i| ValidatorInfo {
                address: { let mut a = [0u8; 20]; a[1] = i; a },
                stake: i as u128 * 1000
            })
            .collect();

        let total: u128 = 10_000_000_000_000_000_000; // 10 tokens
        let result = distributor.distribute(1, total, &miners, &validators, &[], &[]);

        assert_eq!(result.miner_rewards.len(), 100);
        assert_eq!(result.validator_rewards.len(), 50);

        // Total distributed should be close to total emission
        let total_distributed: u128 =
            result.miner_rewards.values().sum::<u128>() +
            result.validator_rewards.values().sum::<u128>() +
            result.dao_allocation;

        assert!(total_distributed <= total);
        // With no delegators/subnets: miners 35% + validators 28% + DAO 13% = 76%
        assert!(total_distributed > total * 75 / 100, "At least 75% should be distributed");
    }
}

// ============================================================
// Reward Executor Tests
// ============================================================

#[cfg(test)]
mod reward_executor_tests {
    use super::*;

    #[test]
    fn test_process_multiple_epochs() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miners = vec![MinerInfo { address: test_address(1), score: 1.0 }];

        // Process 5 epochs
        for epoch in 1..=5 {
            let result = executor.process_epoch(
                epoch,
                epoch * 100,
                &test_utility(),
                &miners,
                &[],
                &[],
                &[],
            );
            assert_eq!(result.epoch, epoch);
            assert!(result.total_emission > 0);
        }

        assert_eq!(executor.current_epoch(), 5);
    }

    #[test]
    fn test_rewards_accumulate() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miner_addr = test_address(1);
        let miners = vec![MinerInfo { address: miner_addr, score: 1.0 }];

        // Process first epoch
        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);
        let reward1 = executor.get_pending_rewards(miner_addr);

        // Process second epoch
        executor.process_epoch(2, 200, &test_utility(), &miners, &[], &[], &[]);
        let reward2 = executor.get_pending_rewards(miner_addr);

        assert!(reward2 > reward1, "Rewards should accumulate");
    }

    #[test]
    fn test_claim_clears_pending() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miner_addr = test_address(1);
        let miners = vec![MinerInfo { address: miner_addr, score: 1.0 }];

        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);

        let pending_before = executor.get_pending_rewards(miner_addr);
        assert!(pending_before > 0);

        let claim_result = executor.claim_rewards(miner_addr);
        assert!(claim_result.success);
        assert_eq!(claim_result.amount, pending_before);

        let pending_after = executor.get_pending_rewards(miner_addr);
        assert_eq!(pending_after, 0, "Pending should be cleared after claim");
    }

    #[test]
    fn test_double_claim_fails() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miner_addr = test_address(1);
        let miners = vec![MinerInfo { address: miner_addr, score: 1.0 }];

        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);

        // First claim succeeds
        let result1 = executor.claim_rewards(miner_addr);
        assert!(result1.success);

        // Second claim should fail (no pending)
        let result2 = executor.claim_rewards(miner_addr);
        assert!(!result2.success);
        assert_eq!(result2.amount, 0);
    }

    #[test]
    fn test_claim_updates_balance() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miner_addr = test_address(1);
        let miners = vec![MinerInfo { address: miner_addr, score: 1.0 }];

        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);

        let pending = executor.get_pending_rewards(miner_addr);
        executor.claim_rewards(miner_addr);

        let balance = executor.get_balance(miner_addr);
        assert_eq!(balance.available, pending, "Claimed amount should be in available balance");
    }

    #[test]
    fn test_dao_allocation_grows() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miners = vec![MinerInfo { address: test_address(1), score: 1.0 }];

        assert_eq!(executor.get_dao_balance(), 0, "Initial DAO balance should be 0");

        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);
        let dao_balance1 = executor.get_dao_balance();
        assert!(dao_balance1 > 0, "DAO should receive allocation");

        executor.process_epoch(2, 200, &test_utility(), &miners, &[], &[], &[]);
        let dao_balance2 = executor.get_dao_balance();
        assert!(dao_balance2 > dao_balance1, "DAO balance should grow");
    }

    #[test]
    fn test_stats() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miners = vec![MinerInfo { address: test_address(1), score: 1.0 }];
        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);

        let stats = executor.stats();
        assert_eq!(stats.current_epoch, 1);
        assert!(stats.total_pending > 0);
        assert!(stats.dao_balance > 0);
        assert_eq!(stats.accounts_with_pending, 1);
    }
}
