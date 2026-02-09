// Reward Distribution Module for Tokenomics v3.1 (Model C - Progressive Staking)
// Distribution: Miners 35%, Validators 28%, Infrastructure 2%, Delegators 12%, Subnet 8%, DAO 5%, Community 10%
// Enhanced: Logarithmic stake curve for whale protection

use super::node_tier::logarithmic_stake;
use std::collections::HashMap;

/// Distribution configuration for reward allocation (Model C v2 - Community Focus)
/// All shares are in basis points (BPS): 10_000 BPS = 100%.
/// Using integer BPS instead of f64 prevents precision loss on large token amounts.
#[derive(Debug, Clone)]
pub struct DistributionConfig {
    /// Miner share in BPS (3500 = 35%) - AI compute providers
    pub miner_share_bps: u32,
    /// Validator share in BPS (2800 = 28%) - AI quality validation
    pub validator_share_bps: u32,
    /// Infrastructure share in BPS (200 = 2%) - Full node operators
    pub infrastructure_share_bps: u32,
    /// Delegator share in BPS (1200 = 12%) - Passive stakers
    pub delegator_share_bps: u32,
    /// Subnet owner share in BPS (800 = 8%) - Subnet creators
    pub subnet_owner_share_bps: u32,
    /// DAO treasury share in BPS (500 = 5%) - Protocol development
    pub dao_share_bps: u32,
    /// Community ecosystem share in BPS (1000 = 10%) - Developer grants, hackathons
    pub community_ecosystem_share_bps: u32,
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self {
            miner_share_bps: 3500,               // 35%
            validator_share_bps: 2800,           // 28%
            infrastructure_share_bps: 200,       // 2%
            delegator_share_bps: 1200,           // 12%
            subnet_owner_share_bps: 800,         // 8%
            dao_share_bps: 500,                  // 5%
            community_ecosystem_share_bps: 1000, // 10%
        }
    }
}

impl DistributionConfig {
    /// Validate that shares sum to 10_000 BPS (100%)
    pub fn validate(&self) -> Result<(), &'static str> {
        let total = self.miner_share_bps
            + self.validator_share_bps
            + self.infrastructure_share_bps
            + self.delegator_share_bps
            + self.subnet_owner_share_bps
            + self.dao_share_bps
            + self.community_ecosystem_share_bps;
        if total != 10_000 {
            return Err("Distribution shares must sum to 10_000 BPS (100%)");
        }
        Ok(())
    }
}

/// Lock bonus configuration for delegators.
/// Bonuses are in basis points (BPS): 1000 BPS = +10%.
#[derive(Debug, Clone)]
pub struct LockBonusConfig {
    /// Bonus for 30-day lock (+10% = 1000 BPS)
    pub bonus_30d_bps: u32,
    /// Bonus for 90-day lock (+25% = 2500 BPS)
    pub bonus_90d_bps: u32,
    /// Bonus for 180-day lock (+50% = 5000 BPS)
    pub bonus_180d_bps: u32,
    /// Bonus for 365-day lock (+100% = 10000 BPS)
    pub bonus_365d_bps: u32,
}

impl Default for LockBonusConfig {
    fn default() -> Self {
        Self {
            bonus_30d_bps: 1000,   // +10%
            bonus_90d_bps: 2500,   // +25%
            bonus_180d_bps: 5000,  // +50%
            bonus_365d_bps: 10000, // +100%
        }
    }
}

impl LockBonusConfig {
    /// Get bonus in BPS based on lock days (0 = no bonus, 1000 = +10%, etc.)
    pub fn get_bonus_bps(&self, lock_days: u32) -> u32 {
        if lock_days >= 365 {
            self.bonus_365d_bps
        } else if lock_days >= 180 {
            self.bonus_180d_bps
        } else if lock_days >= 90 {
            self.bonus_90d_bps
        } else if lock_days >= 30 {
            self.bonus_30d_bps
        } else {
            0
        }
    }
}

/// Participant info for reward calculation
///
/// **NOTE:** The `has_gpu` field is deprecated.
/// Use `MinerEpochStats` with task-based GPU verification instead (via `process_epoch_v2`).
#[derive(Debug, Clone)]
pub struct MinerInfo {
    pub address: [u8; 20],
    pub score: f64, // 0.0 - 1.0
    /// Whether miner has GPU capability
    ///
    /// **DEPRECATED:** This field uses self-declaration which is exploitable.
    /// Use `MinerEpochStats.gpu_tasks_completed` for verified GPU activity.
    pub has_gpu: bool,
}

impl MinerInfo {
    pub fn new(address: [u8; 20], score: f64) -> Self {
        Self { address, score, has_gpu: false }
    }

    /// **DEPRECATED:** Use MinerEpochStats instead for verified GPU capability
    pub fn with_gpu(address: [u8; 20], score: f64) -> Self {
        Self { address, score, has_gpu: true }
    }
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

/// Miner statistics for an epoch with task-based GPU verification
/// GPU bonus is calculated from actual task completion, not self-declaration
#[derive(Debug, Clone)]
pub struct MinerEpochStats {
    /// Miner address
    pub address: [u8; 20],
    /// Base performance score (0.0 - 1.0)
    pub base_score: f64,
    /// Number of CPU tasks completed this epoch
    pub cpu_tasks_completed: u32,
    /// Number of GPU tasks completed this epoch (verified by validators)
    pub gpu_tasks_completed: u32,
    /// Number of GPU tasks assigned this epoch
    pub gpu_tasks_assigned: u32,
}

impl MinerEpochStats {
    pub fn new(address: [u8; 20], base_score: f64) -> Self {
        Self {
            address,
            base_score,
            cpu_tasks_completed: 0,
            gpu_tasks_completed: 0,
            gpu_tasks_assigned: 0,
        }
    }

    /// Create with task completion data
    pub fn with_tasks(
        address: [u8; 20],
        base_score: f64,
        cpu_tasks: u32,
        gpu_completed: u32,
        gpu_assigned: u32,
    ) -> Self {
        Self {
            address,
            base_score,
            cpu_tasks_completed: cpu_tasks,
            gpu_tasks_completed: gpu_completed,
            gpu_tasks_assigned: gpu_assigned,
        }
    }

    /// Calculate GPU completion ratio (0.0 - 1.0)
    /// Returns 0.0 if no GPU tasks were assigned
    pub fn gpu_ratio(&self) -> f64 {
        if self.gpu_tasks_assigned == 0 {
            return 0.0;
        }
        (self.gpu_tasks_completed as f64) / (self.gpu_tasks_assigned as f64)
    }

    /// Calculate effective GPU bonus based on task completion ratio
    /// max_bonus_rate: 1.4 means max 40% bonus
    /// Returns: 1.0 (no bonus) to max_bonus_rate (full bonus)
    /// Example: 80% completion @ 1.4 max → 1.0 + 0.4*0.8 = 1.32x
    pub fn effective_gpu_bonus(&self, max_bonus_rate: f64) -> f64 {
        let bonus_factor = (max_bonus_rate - 1.0).max(0.0);
        1.0 + bonus_factor * self.gpu_ratio()
    }

    /// Total tasks completed
    pub fn total_tasks(&self) -> u32 {
        self.cpu_tasks_completed + self.gpu_tasks_completed
    }
}

/// Infrastructure node info for reward calculation
#[derive(Debug, Clone)]
pub struct InfrastructureNodeInfo {
    pub address: [u8; 20],
    /// Uptime score (0.0 - 1.0): proportion of blocks served / expected
    pub uptime_score: f64,
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
    /// Infrastructure node rewards (2% of emission)
    /// Previously unallocated — now distributed to full-node operators by uptime
    pub infrastructure_rewards: HashMap<[u8; 20], u128>,
    pub dao_allocation: u128,
    pub community_ecosystem_allocation: u128,
}

/// Reward Distributor - implements tokenomics v3
pub struct RewardDistributor {
    config: DistributionConfig,
    lock_bonus: LockBonusConfig,
    #[allow(dead_code)] // Accessed via dao_address() getter
    dao_address: [u8; 20],
}

impl RewardDistributor {
    /// Create new reward distributor
    pub fn new(
        config: DistributionConfig,
        lock_bonus: LockBonusConfig,
        dao_address: [u8; 20],
    ) -> Self {
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
    ///
    /// # Fix: Infrastructure share (2%) is now distributed to full-node operators
    /// Previously, the 200 BPS infrastructure allocation was calculated but never
    /// distributed — effectively burning 2% of each epoch's emission.
    /// Now infrastructure nodes receive rewards proportional to uptime.
    pub fn distribute(
        &self,
        epoch: u64,
        total_emission: u128,
        miners: &[MinerInfo],
        validators: &[ValidatorInfo],
        delegators: &[DelegatorInfo],
        subnets: &[SubnetInfo],
    ) -> DistributionResult {
        self.distribute_with_infra(
            epoch,
            total_emission,
            miners,
            validators,
            delegators,
            subnets,
            &[],
        )
    }

    /// Distribute rewards including infrastructure node operators
    pub fn distribute_with_infra(
        &self,
        epoch: u64,
        total_emission: u128,
        miners: &[MinerInfo],
        validators: &[ValidatorInfo],
        delegators: &[DelegatorInfo],
        subnets: &[SubnetInfo],
        infra_nodes: &[InfrastructureNodeInfo],
    ) -> DistributionResult {
        // Calculate pool sizes using integer BPS arithmetic (no f64 precision loss)
        let miner_pool = total_emission * self.config.miner_share_bps as u128 / 10_000;
        let validator_pool = total_emission * self.config.validator_share_bps as u128 / 10_000;
        let infra_pool = total_emission * self.config.infrastructure_share_bps as u128 / 10_000;
        let delegator_pool = total_emission * self.config.delegator_share_bps as u128 / 10_000;
        let subnet_pool = total_emission * self.config.subnet_owner_share_bps as u128 / 10_000;
        let dao_allocation = total_emission * self.config.dao_share_bps as u128 / 10_000;
        let community_ecosystem_allocation =
            total_emission * self.config.community_ecosystem_share_bps as u128 / 10_000;

        // Distribute to each group
        let miner_rewards = self.distribute_by_score(miner_pool, miners);
        let validator_rewards = self.distribute_by_stake(validator_pool, validators);
        let infrastructure_rewards = self.distribute_to_infrastructure(infra_pool, infra_nodes);
        let delegator_rewards = self.distribute_to_delegators(delegator_pool, delegators);
        let subnet_owner_rewards = self.distribute_to_subnets(subnet_pool, subnets);

        DistributionResult {
            epoch,
            total_distributed: miner_pool
                + validator_pool
                + infra_pool
                + delegator_pool
                + subnet_pool
                + dao_allocation
                + community_ecosystem_allocation,
            miner_rewards,
            validator_rewards,
            delegator_rewards,
            subnet_owner_rewards,
            infrastructure_rewards,
            dao_allocation,
            community_ecosystem_allocation,
        }
    }

    /// Distribute by performance score (for miners)
    ///
    /// SECURITY: Uses fixed-point integer arithmetic instead of f64 to prevent
    /// precision loss for large reward pools. Scores are scaled by 10^12 before
    /// integer division to maintain accuracy.
    fn distribute_by_score(&self, pool: u128, miners: &[MinerInfo]) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        let total_score: f64 = miners.iter().map(|m| m.score).sum();
        if total_score == 0.0 {
            return rewards;
        }

        // SECURITY: Scale scores to integer for precision-safe division
        // Each score is scaled by 10^12 relative to total, then used for pro-rata
        const PRECISION: u128 = 1_000_000_000_000; // 10^12
        let mut _distributed: u128 = 0;

        // Convert scores to scaled integer shares to avoid f64→u128 cast precision loss
        let scaled_scores: Vec<([u8; 20], u128)> = miners
            .iter()
            .map(|m| {
                // Scale: (score * PRECISION) / total_score — both f64, result fits u128
                let share = if total_score > 0.0 {
                    ((m.score / total_score) * PRECISION as f64).min(PRECISION as f64) as u128
                } else {
                    0
                };
                (m.address, share)
            })
            .collect();

        for (address, scaled_share) in scaled_scores {
            let reward = pool.checked_mul(scaled_share).map(|x| x / PRECISION).unwrap_or(0);
            if reward > 0 {
                rewards.insert(address, reward);
                _distributed += reward;
            }
        }

        rewards
    }

    /// Distribute by performance score with GPU bonus (for miners in AI subnets)
    /// gpu_bonus_rate: 1.0 = no bonus, 1.2 = 20% bonus, max 1.4 = 40% bonus
    pub fn distribute_by_score_with_gpu(
        &self,
        pool: u128,
        miners: &[MinerInfo],
        gpu_bonus_rate: f64,
    ) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        // Calculate effective scores with GPU bonus
        let effective_scores: Vec<([u8; 20], f64)> = miners
            .iter()
            .map(|m| {
                let bonus = if m.has_gpu { gpu_bonus_rate } else { 1.0 };
                (m.address, m.score * bonus)
            })
            .collect();

        let total_score: f64 = effective_scores.iter().map(|(_, s)| s).sum();
        if total_score == 0.0 {
            return rewards;
        }

        // SECURITY: Use fixed-point integer arithmetic for precision
        const PRECISION: u128 = 1_000_000_000_000;
        for (address, effective_score) in effective_scores {
            let scaled_share = if total_score > 0.0 {
                ((effective_score / total_score) * PRECISION as f64).min(PRECISION as f64) as u128
            } else {
                0
            };
            let reward = pool.checked_mul(scaled_share).map(|x| x / PRECISION).unwrap_or(0);
            if reward > 0 {
                rewards.insert(address, reward);
            }
        }

        rewards
    }

    /// Distribute by epoch stats with task-based GPU verification (RECOMMENDED)
    /// GPU bonus is proportional to actual GPU task completion ratio
    /// This prevents miners from claiming GPU bonus without actually completing GPU tasks
    ///
    /// # Security
    /// - No self-declaration: bonus based on verified task completion
    /// - Allows hardware changes: miners can change machines freely
    /// - Validators verify GPU task outputs for quality + timing
    pub fn distribute_by_epoch_stats(
        &self,
        pool: u128,
        miners: &[MinerEpochStats],
        gpu_bonus_rate: f64, // max bonus rate (e.g., 1.4 = 40% max)
    ) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        // Calculate effective scores with dynamic GPU bonus
        let effective_scores: Vec<([u8; 20], f64)> = miners
            .iter()
            .map(|m| {
                // GPU bonus scales with task completion ratio
                let bonus = m.effective_gpu_bonus(gpu_bonus_rate);
                (m.address, m.base_score * bonus)
            })
            .collect();

        let total_score: f64 = effective_scores.iter().map(|(_, s)| s).sum();
        if total_score == 0.0 {
            return rewards;
        }

        for (address, effective_score) in effective_scores {
            // SECURITY: Use fixed-point integer arithmetic for precision
            const PRECISION: u128 = 1_000_000_000_000;
            let scaled_share = if total_score > 0.0 {
                ((effective_score / total_score) * PRECISION as f64).min(PRECISION as f64) as u128
            } else {
                0
            };
            let reward = pool.checked_mul(scaled_share).map(|x| x / PRECISION).unwrap_or(0);
            if reward > 0 {
                rewards.insert(address, reward);
            }
        }

        rewards
    }

    /// Distribute by stake with logarithmic curve for whale protection
    fn distribute_by_stake(
        &self,
        pool: u128,
        validators: &[ValidatorInfo],
    ) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        // Calculate effective stake using logarithmic curve
        let effective_stakes: Vec<([u8; 20], u128)> =
            validators.iter().map(|v| (v.address, logarithmic_stake(v.stake))).collect();

        let total_effective: u128 = effective_stakes.iter().map(|(_, e)| e).sum();
        if total_effective == 0 {
            return rewards;
        }

        for (address, effective) in effective_stakes {
            // Use u128 arithmetic to avoid overflow
            let share =
                (pool as u128).checked_mul(effective).map(|x| x / total_effective).unwrap_or(0);
            if share > 0 {
                rewards.insert(address, share);
            }
        }

        rewards
    }

    /// Distribute to delegators with lock bonus and logarithmic stake curve
    fn distribute_to_delegators(
        &self,
        pool: u128,
        delegators: &[DelegatorInfo],
    ) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        // Calculate weighted stake: logarithmic_stake * (1 + lock_bonus_bps/10000)
        let weighted_stakes: Vec<(_, u128)> = delegators
            .iter()
            .map(|d| {
                let bonus_bps = self.lock_bonus.get_bonus_bps(d.lock_days);
                let effective = logarithmic_stake(d.stake);
                // Integer-safe: effective * (10_000 + bonus_bps) / 10_000
                let weight = effective * (10_000 + bonus_bps as u128) / 10_000;
                (d.address, weight)
            })
            .collect();

        let total_weighted: u128 = weighted_stakes.iter().map(|(_, w)| w).sum();
        if total_weighted == 0 {
            return rewards;
        }

        for (address, weight) in weighted_stakes {
            let share = (pool as u128).checked_mul(weight).map(|x| x / total_weighted).unwrap_or(0);
            if share > 0 {
                rewards.insert(address, share);
            }
        }

        rewards
    }

    /// Distribute to infrastructure (full-node) operators by uptime score
    ///
    /// This fixes the 2% "infrastructure gap" where the pool was allocated but
    /// never distributed. Nodes with higher uptime receive proportionally more.
    fn distribute_to_infrastructure(
        &self,
        pool: u128,
        nodes: &[InfrastructureNodeInfo],
    ) -> HashMap<[u8; 20], u128> {
        let mut rewards = HashMap::new();

        if nodes.is_empty() {
            // No infrastructure nodes registered — undistributed infra pool
            // is implicitly held (not burned, can be retroactively distributed)
            return rewards;
        }

        let total_score: f64 = nodes.iter().map(|n| n.uptime_score).sum();
        if total_score == 0.0 {
            return rewards;
        }

        let mut _distributed: u128 = 0;
        for node in nodes {
            // SECURITY: Use fixed-point integer arithmetic instead of f64
            const PRECISION: u128 = 1_000_000_000_000;
            let scaled_share = if total_score > 0.0 {
                ((node.uptime_score / total_score) * PRECISION as f64).min(PRECISION as f64) as u128
            } else {
                0
            };
            let reward = pool
                .checked_mul(scaled_share)
                .map(|x| x / PRECISION)
                .unwrap_or(0)
                .min(pool.saturating_sub(_distributed));
            if reward > 0 {
                rewards.insert(node.address, reward);
                _distributed += reward;
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

    /// Get reference to distribution config
    pub fn config(&self) -> &DistributionConfig {
        &self.config
    }

    /// Get reference to lock bonus config
    pub fn lock_bonus_config(&self) -> &LockBonusConfig {
        &self.lock_bonus
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
        assert_eq!(config.miner_share_bps, 3500);
        assert_eq!(config.validator_share_bps, 2800);
        assert_eq!(config.delegator_share_bps, 1200);
        assert_eq!(config.subnet_owner_share_bps, 800);
        assert_eq!(config.dao_share_bps, 500);
        assert_eq!(config.community_ecosystem_share_bps, 1000);
    }

    #[test]
    fn test_lock_bonus() {
        let config = LockBonusConfig::default();
        assert_eq!(config.get_bonus_bps(0), 0);
        assert_eq!(config.get_bonus_bps(29), 0);
        assert_eq!(config.get_bonus_bps(30), 1000);
        assert_eq!(config.get_bonus_bps(90), 2500);
        assert_eq!(config.get_bonus_bps(180), 5000);
        assert_eq!(config.get_bonus_bps(365), 10000);
        assert_eq!(config.get_bonus_bps(1000), 10000);
    }

    #[test]
    fn test_full_distribution() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let total_emission: u128 = 1_000_000_000_000_000_000; // 1 token

        let miners = vec![
            MinerInfo { address: test_address(1), score: 0.6, has_gpu: false },
            MinerInfo { address: test_address(2), score: 0.4, has_gpu: false },
        ];

        let validators = vec![
            ValidatorInfo { address: test_address(10), stake: 100 },
            ValidatorInfo { address: test_address(11), stake: 100 },
        ];

        let delegators = vec![
            DelegatorInfo { address: test_address(20), stake: 100, lock_days: 0 },
            DelegatorInfo { address: test_address(21), stake: 100, lock_days: 365 },
        ];

        let subnets = vec![SubnetInfo { owner: test_address(30), emission_weight: 50 }];

        let result =
            distributor.distribute(1, total_emission, &miners, &validators, &delegators, &subnets);

        // Check total distributed
        let total_rewards: u128 = result.miner_rewards.values().sum::<u128>()
            + result.validator_rewards.values().sum::<u128>()
            + result.delegator_rewards.values().sum::<u128>()
            + result.subnet_owner_rewards.values().sum::<u128>()
            + result.infrastructure_rewards.values().sum::<u128>()
            + result.dao_allocation
            + result.community_ecosystem_allocation;

        // Should be close to total_emission (may have rounding)
        assert!(total_rewards > 0);
        assert!(total_rewards <= total_emission);

        // Check DAO got 5% (500 BPS)
        let expected_dao = total_emission * 500 / 10_000;
        assert_eq!(result.dao_allocation, expected_dao);

        // Check community ecosystem got 10% (1000 BPS)
        let expected_community = total_emission * 1000 / 10_000;
        assert_eq!(result.community_ecosystem_allocation, expected_community);

        // Check delegator with lock gets more than one without
        let d20_reward = *result.delegator_rewards.get(&test_address(20)).unwrap_or(&0);
        let d21_reward = *result.delegator_rewards.get(&test_address(21)).unwrap_or(&0);
        assert!(
            d21_reward > d20_reward,
            "Locked delegator should get more: {} vs {}",
            d21_reward,
            d20_reward
        );
    }

    #[test]
    fn test_gpu_bonus_distribution() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let pool: u128 = 1_000_000; // 1M units

        // Two miners with same score, one has GPU
        let miners = vec![
            MinerInfo::new(test_address(1), 0.5),      // CPU only
            MinerInfo::with_gpu(test_address(2), 0.5), // Has GPU
        ];

        // Without GPU bonus (rate = 1.0)
        let rewards_no_bonus = distributor.distribute_by_score_with_gpu(pool, &miners, 1.0);
        let cpu_reward = *rewards_no_bonus.get(&test_address(1)).unwrap();
        let gpu_reward = *rewards_no_bonus.get(&test_address(2)).unwrap();
        assert_eq!(cpu_reward, gpu_reward, "Without bonus, rewards should be equal");

        // With 20% GPU bonus (rate = 1.2)
        let rewards_with_bonus = distributor.distribute_by_score_with_gpu(pool, &miners, 1.2);
        let cpu_reward = *rewards_with_bonus.get(&test_address(1)).unwrap();
        let gpu_reward = *rewards_with_bonus.get(&test_address(2)).unwrap();
        assert!(
            gpu_reward > cpu_reward,
            "GPU miner should get more: {} vs {}",
            gpu_reward,
            cpu_reward
        );

        // GPU should get ~54.5% (0.5*1.2 / (0.5 + 0.5*1.2) = 0.6/1.1)
        // CPU should get ~45.5% (0.5 / 1.1)
        let gpu_share = gpu_reward as f64 / pool as f64;
        assert!(
            gpu_share > 0.54 && gpu_share < 0.56,
            "GPU share should be ~54.5%, got {}",
            gpu_share
        );
    }

    #[test]
    fn test_miner_epoch_stats_gpu_ratio() {
        // No GPU tasks assigned → 0 ratio
        let stats1 = MinerEpochStats::new(test_address(1), 0.5);
        assert_eq!(stats1.gpu_ratio(), 0.0);

        // 8 out of 10 GPU tasks completed → 0.8 ratio
        let stats2 = MinerEpochStats::with_tasks(test_address(2), 0.5, 5, 8, 10);
        assert!((stats2.gpu_ratio() - 0.8).abs() < 0.001);

        // All GPU tasks completed → 1.0 ratio
        let stats3 = MinerEpochStats::with_tasks(test_address(3), 0.5, 5, 10, 10);
        assert!((stats3.gpu_ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_effective_gpu_bonus_calculation() {
        // 80% completion @ 1.4 max → 1.0 + 0.4*0.8 = 1.32x
        let stats = MinerEpochStats::with_tasks(test_address(1), 1.0, 0, 8, 10);
        let bonus = stats.effective_gpu_bonus(1.4);
        assert!((bonus - 1.32).abs() < 0.001, "Expected 1.32, got {}", bonus);

        // 0% completion → 1.0x (no bonus)
        let stats_no_gpu = MinerEpochStats::with_tasks(test_address(2), 1.0, 10, 0, 10);
        let bonus_none = stats_no_gpu.effective_gpu_bonus(1.4);
        assert!((bonus_none - 1.0).abs() < 0.001, "Expected 1.0, got {}", bonus_none);

        // 100% completion → full 1.4x bonus
        let stats_full = MinerEpochStats::with_tasks(test_address(3), 1.0, 0, 10, 10);
        let bonus_full = stats_full.effective_gpu_bonus(1.4);
        assert!((bonus_full - 1.4).abs() < 0.001, "Expected 1.4, got {}", bonus_full);
    }

    #[test]
    fn test_distribute_by_epoch_stats() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let pool: u128 = 1_000_000;

        // Two miners with same base score
        // Miner 1: No GPU tasks completed (0/10)
        // Miner 2: All GPU tasks completed (10/10)
        let miners = vec![
            MinerEpochStats::with_tasks(test_address(1), 0.5, 10, 0, 10), // CPU only
            MinerEpochStats::with_tasks(test_address(2), 0.5, 0, 10, 10), // Full GPU
        ];

        // With 40% max GPU bonus (rate = 1.4)
        let rewards = distributor.distribute_by_epoch_stats(pool, &miners, 1.4);
        let cpu_reward = *rewards.get(&test_address(1)).unwrap();
        let gpu_reward = *rewards.get(&test_address(2)).unwrap();

        assert!(gpu_reward > cpu_reward, "GPU miner should get more");

        // Miner 1: 0.5 * 1.0 = 0.5 effective
        // Miner 2: 0.5 * 1.4 = 0.7 effective
        // Total: 1.2
        // GPU share: 0.7/1.2 = 58.33%
        let gpu_share = gpu_reward as f64 / pool as f64;
        assert!(
            gpu_share > 0.58 && gpu_share < 0.59,
            "GPU share should be ~58.3%, got {}",
            gpu_share
        );
    }

    #[test]
    fn test_partial_gpu_completion_scaling() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let pool: u128 = 1_000_000;

        // 50% GPU completion should get 50% of max bonus
        let miners = vec![
            MinerEpochStats::with_tasks(test_address(1), 0.5, 10, 0, 0), // No GPU assigned
            MinerEpochStats::with_tasks(test_address(2), 0.5, 0, 5, 10), // 50% GPU completion
        ];

        let rewards = distributor.distribute_by_epoch_stats(pool, &miners, 1.4);
        let base_reward = *rewards.get(&test_address(1)).unwrap();
        let partial_reward = *rewards.get(&test_address(2)).unwrap();

        // Miner 1: 0.5 * 1.0 = 0.5 (no GPU assigned = 1.0x)
        // Miner 2: 0.5 * 1.2 = 0.6 (50% completion = 1.0 + 0.4*0.5 = 1.2x)
        // Partial should get more, but not as much as full completion
        assert!(partial_reward > base_reward, "Partial completion should earn more");

        let ratio = partial_reward as f64 / base_reward as f64;
        // Ratio should be ~1.2 (0.6/0.5)
        assert!(ratio > 1.15 && ratio < 1.25, "Bonus ratio should be ~1.2, got {}", ratio);
    }

    #[test]
    fn test_infrastructure_distribution() {
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let total_emission: u128 = 1_000_000_000_000_000_000; // 1 token

        let miners = vec![MinerInfo::new(test_address(1), 0.5)];
        let validators = vec![ValidatorInfo { address: test_address(10), stake: 100 }];
        let delegators =
            vec![DelegatorInfo { address: test_address(20), stake: 100, lock_days: 0 }];
        let subnets = vec![SubnetInfo { owner: test_address(30), emission_weight: 50 }];
        let infra_nodes = vec![
            InfrastructureNodeInfo { address: test_address(40), uptime_score: 0.99 },
            InfrastructureNodeInfo { address: test_address(41), uptime_score: 0.50 },
        ];

        let result = distributor.distribute_with_infra(
            1,
            total_emission,
            &miners,
            &validators,
            &delegators,
            &subnets,
            &infra_nodes,
        );

        // Infrastructure pool = 200 BPS = 2%
        let expected_infra_pool = total_emission * 200 / 10_000;

        // Infra rewards should exist and total close to the pool
        let infra_total: u128 = result.infrastructure_rewards.values().sum();
        assert!(infra_total > 0, "Infrastructure rewards should be non-zero");
        // Allow fixed-point rounding tolerance: with PRECISION=10^12,
        // max error per node ≈ pool / PRECISION, so total ≈ N * pool / 10^12
        assert!(infra_total <= expected_infra_pool, "Should not exceed infra pool");
        let max_rounding_error =
            (infra_nodes.len() as u128) * expected_infra_pool / 1_000_000_000_000 + 1;
        assert!(
            expected_infra_pool - infra_total <= max_rounding_error,
            "Infra rewards rounding too large: diff={}, max_allowed={}, total={} vs pool={}",
            expected_infra_pool - infra_total,
            max_rounding_error,
            infra_total,
            expected_infra_pool,
        );

        // Higher uptime → more reward
        let r40 = *result.infrastructure_rewards.get(&test_address(40)).unwrap_or(&0);
        let r41 = *result.infrastructure_rewards.get(&test_address(41)).unwrap_or(&0);
        assert!(r40 > r41, "Higher uptime should get more: {} vs {}", r40, r41);
    }

    #[test]
    fn test_distribute_backward_compatible() {
        // The original distribute() (without infra nodes) should still work
        // and produce the same results as before (infra pool is empty)
        let dao_addr = test_address(100);
        let distributor = RewardDistributor::default_with_dao(dao_addr);

        let total_emission: u128 = 1_000_000;
        let miners = vec![MinerInfo::new(test_address(1), 1.0)];
        let validators = vec![ValidatorInfo { address: test_address(10), stake: 100 }];
        let delegators = vec![];
        let subnets = vec![];

        let result =
            distributor.distribute(1, total_emission, &miners, &validators, &delegators, &subnets);

        // Infrastructure rewards should be empty (no infra nodes passed)
        assert!(result.infrastructure_rewards.is_empty());

        // Total distributed should include the infra pool allocation even though
        // it wasn't distributed — this represents the full 100% accounting
        let expected_total = total_emission; // All 10,000 BPS
        assert_eq!(result.total_distributed, expected_total);
    }
}
