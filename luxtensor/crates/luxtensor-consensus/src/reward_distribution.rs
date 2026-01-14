// Reward Distribution Module for Tokenomics v3.1 (Model C - Progressive Staking)
// Distribution: Miners 35%, Validators 28%, Infrastructure 2%, Delegators 12%, Subnet 10%, DAO 13%

use std::collections::HashMap;

/// Distribution configuration for reward allocation (Model C)
#[derive(Debug, Clone)]
pub struct DistributionConfig {
    /// Miner share (35%) - AI compute providers
    pub miner_share: f64,
    /// Validator share (28%) - AI quality validation
    pub validator_share: f64,
    /// Infrastructure share (2%) - Full node operators
    pub infrastructure_share: f64,
    /// Delegator share (12%) - Passive stakers
    pub delegator_share: f64,
    /// Subnet owner share (10%) - Subnet creators
    pub subnet_owner_share: f64,
    /// DAO treasury share (13%) - Protocol development
    pub dao_share: f64,
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self {
            miner_share: 0.35,
            validator_share: 0.28,        // Reduced from 30%
            infrastructure_share: 0.02,    // NEW: For full node operators
            delegator_share: 0.12,
            subnet_owner_share: 0.10,
            dao_share: 0.13,
        }
    }
}

impl DistributionConfig {
    /// Validate that shares sum to 1.0
    pub fn validate(&self) -> Result<(), &'static str> {
        let total = self.miner_share + self.validator_share + self.infrastructure_share +
                    self.delegator_share + self.subnet_owner_share + self.dao_share;
        if (total - 1.0).abs() > 0.001 {
            return Err("Distribution shares must sum to 1.0");
        }
        Ok(())
    }
}

/// Lock bonus configuration for delegators
#[derive(Debug, Clone)]
pub struct LockBonusConfig {
    /// Bonus for 30-day lock (+10%)
    pub bonus_30d: f64,
    /// Bonus for 90-day lock (+25%)
    pub bonus_90d: f64,
    /// Bonus for 180-day lock (+50%)
    pub bonus_180d: f64,
    /// Bonus for 365-day lock (+100%)
    pub bonus_365d: f64,
}

impl Default for LockBonusConfig {
    fn default() -> Self {
        Self {
            bonus_30d: 0.10,
            bonus_90d: 0.25,
            bonus_180d: 0.50,
            bonus_365d: 1.00,
        }
    }
}

impl LockBonusConfig {
    /// Get bonus multiplier based on lock days
    pub fn get_bonus(&self, lock_days: u32) -> f64 {
        if lock_days >= 365 {
            self.bonus_365d
        } else if lock_days >= 180 {
            self.bonus_180d
        } else if lock_days >= 90 {
            self.bonus_90d
        } else if lock_days >= 30 {
            self.bonus_30d
        } else {
            0.0
        }
    }
}

/// Participant info for reward calculation
#[derive(Debug, Clone)]
pub struct MinerInfo {
    pub address: [u8; 20],
    pub score: f64,  // 0.0 - 1.0
}

#[derive(Debug, Clone)]
pub struct ValidatorInfo {
    pub address: [u8; 20],
    pub stake: u128,
}

#[derive(Debug, Clone)]
pub struct DelegatorInfo {
    pub address: [u8; 20],
    pub stake: u128,
    pub lock_days: u32,
}

#[derive(Debug, Clone)]
pub struct SubnetInfo {
    pub owner: [u8; 20],
    pub emission_weight: u128,
}

/// Result of reward distribution
#[derive(Debug, Clone)]
pub struct DistributionResult {
    pub epoch: u64,
    pub total_distributed: u128,
    pub miner_rewards: HashMap<[u8; 20], u128>,
    pub validator_rewards: HashMap<[u8; 20], u128>,
    pub delegator_rewards: HashMap<[u8; 20], u128>,
    pub subnet_owner_rewards: HashMap<[u8; 20], u128>,
    pub dao_allocation: u128,
}

/// Reward Distributor - implements tokenomics v3
pub struct RewardDistributor {
    config: DistributionConfig,
    lock_bonus: LockBonusConfig,
    dao_address: [u8; 20],
}

impl RewardDistributor {
    /// Create new reward distributor
    pub fn new(config: DistributionConfig, lock_bonus: LockBonusConfig, dao_address: [u8; 20]) -> Self {
        Self { config, lock_bonus, dao_address }
    }

    /// Create with default config
    pub fn default_with_dao(dao_address: [u8; 20]) -> Self {
        Self {
            config: DistributionConfig::default(),
            lock_bonus: LockBonusConfig::default(),
            dao_address,
        }
    }

    /// Distribute rewards for an epoch
    pub fn distribute(
        &self,
        epoch: u64,
        total_emission: u128,
        miners: &[MinerInfo],
        validators: &[ValidatorInfo],
        delegators: &[DelegatorInfo],
        subnets: &[SubnetInfo],
    ) -> DistributionResult {
        // Calculate pool sizes
        let miner_pool = (total_emission as f64 * self.config.miner_share) as u128;
        let validator_pool = (total_emission as f64 * self.config.validator_share) as u128;
        let delegator_pool = (total_emission as f64 * self.config.delegator_share) as u128;
        let subnet_pool = (total_emission as f64 * self.config.subnet_owner_share) as u128;
        let dao_allocation = (total_emission as f64 * self.config.dao_share) as u128;

        // Distribute to each group
        let miner_rewards = self.distribute_by_score(miner_pool, miners);
        let validator_rewards = self.distribute_by_stake(validator_pool, validators);
        let delegator_rewards = self.distribute_to_delegators(delegator_pool, delegators);
        let subnet_owner_rewards = self.distribute_to_subnets(subnet_pool, subnets);

        DistributionResult {
            epoch,
            total_distributed: miner_pool + validator_pool + delegator_pool + subnet_pool + dao_allocation,
            miner_rewards,
            validator_rewards,
            delegator_rewards,
            subnet_owner_rewards,
            dao_allocation,
        }
    }

    /// Distribute by performance score (for miners)
    fn distribute_by_score(&self, pool: u128, miners: &[MinerInfo]) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        let total_score: f64 = miners.iter().map(|m| m.score).sum();
        if total_score == 0.0 {
            return rewards;
        }

        for miner in miners {
            let share = miner.score / total_score;
            let reward = (pool as f64 * share) as u128;
            if reward > 0 {
                rewards.insert(miner.address, reward);
            }
        }

        rewards
    }

    /// Distribute by stake (for validators)
    fn distribute_by_stake(&self, pool: u128, validators: &[ValidatorInfo]) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        let total_stake: u128 = validators.iter().map(|v| v.stake).sum();
        if total_stake == 0 {
            return rewards;
        }

        for validator in validators {
            // Use u128 arithmetic to avoid overflow
            let share = (pool as u128)
                .checked_mul(validator.stake)
                .map(|x| x / total_stake)
                .unwrap_or(0);
            if share > 0 {
                rewards.insert(validator.address, share);
            }
        }

        rewards
    }

    /// Distribute to delegators with lock bonus
    fn distribute_to_delegators(&self, pool: u128, delegators: &[DelegatorInfo]) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        // Calculate weighted stake (stake * (1 + lock_bonus))
        let weighted_stakes: Vec<(_, u128)> = delegators.iter().map(|d| {
            let bonus = self.lock_bonus.get_bonus(d.lock_days);
            let weight = (d.stake as f64 * (1.0 + bonus)) as u128;
            (d.address, weight)
        }).collect();

        let total_weighted: u128 = weighted_stakes.iter().map(|(_, w)| w).sum();
        if total_weighted == 0 {
            return rewards;
        }

        for (address, weight) in weighted_stakes {
            let share = (pool as u128)
                .checked_mul(weight)
                .map(|x| x / total_weighted)
                .unwrap_or(0);
            if share > 0 {
                rewards.insert(address, share);
            }
        }

        rewards
    }

    /// Distribute to subnet owners by emission weight
    fn distribute_to_subnets(&self, pool: u128, subnets: &[SubnetInfo]) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        let total_weight: u128 = subnets.iter().map(|s| s.emission_weight).sum();
        if total_weight == 0 {
            return rewards;
        }

        for subnet in subnets {
            let share = (pool as u128)
                .checked_mul(subnet.emission_weight)
                .map(|x| x / total_weight)
                .unwrap_or(0);
            if share > 0 {
                *rewards.entry(subnet.owner).or_insert(0) += share;
            }
        }

        rewards
    }

    /// Get DAO address
    pub fn dao_address(&self) -> [u8; 20] {
        self.dao_address
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(id: u8) -> [u8; 20] {
        let mut addr = [0u8; 20];
        addr[0] = id;
        addr
    }

    #[test]
    fn test_distribution_config_default() {
        let config = DistributionConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.miner_share, 0.35);
        assert_eq!(config.validator_share, 0.28);  // Updated from 0.30
        assert_eq!(config.delegator_share, 0.12);
        assert_eq!(config.subnet_owner_share, 0.10);
        assert_eq!(config.dao_share, 0.13);
    }

    #[test]
    fn test_lock_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus(0), 0.0);
        assert_eq!(config.get_bonus(29), 0.0);
        assert_eq!(config.get_bonus(30), 0.10);
        assert_eq!(config.get_bonus(90), 0.25);
        assert_eq!(config.get_bonus(180), 0.50);
        assert_eq!(config.get_bonus(365), 1.00);
        assert_eq!(config.get_bonus(1000), 1.00);
    }

    #[test]
    fn test_full_distribution() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let total_emission: u128 = 1_000_000_000_000_000_000; // 1 token

        let miners = vec![
            MinerInfo { address: test_address(1), score: 0.6 },
            MinerInfo { address: test_address(2), score: 0.4 },
        ];

        let validators = vec![
            ValidatorInfo { address: test_address(10), stake: 100 },
            ValidatorInfo { address: test_address(11), stake: 100 },
        ];

        let delegators = vec![
            DelegatorInfo { address: test_address(20), stake: 100, lock_days: 0 },
            DelegatorInfo { address: test_address(21), stake: 100, lock_days: 365 },
        ];

        let subnets = vec![
            SubnetInfo { owner: test_address(30), emission_weight: 50 },
        ];

        let result = distributor.distribute(1, total_emission, &miners, &validators, &delegators, &subnets);

        // Check total distributed
        let total_rewards: u128 =
            result.miner_rewards.values().sum::<u128>() +
            result.validator_rewards.values().sum::<u128>() +
            result.delegator_rewards.values().sum::<u128>() +
            result.subnet_owner_rewards.values().sum::<u128>() +
            result.dao_allocation;

        // Should be close to total_emission (may have rounding)
        assert!(total_rewards > 0);
        assert!(total_rewards <= total_emission);

        // Check DAO got 13%
        let expected_dao = (total_emission as f64 * 0.13) as u128;
        assert_eq!(result.dao_allocation, expected_dao);

        // Check delegator with lock gets more than one without
        let d20_reward = *result.delegator_rewards.get(&test_address(20)).unwrap_or(&0);
        let d21_reward = *result.delegator_rewards.get(&test_address(21)).unwrap_or(&0);
        assert!(d21_reward > d20_reward, "Locked delegator should get more: {} vs {}", d21_reward, d20_reward);
    }
}
