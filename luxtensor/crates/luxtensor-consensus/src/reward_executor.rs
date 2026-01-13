// Reward Executor Module
// Processes epoch rewards and credits tokens to participant balances

use std::collections::HashMap;
use parking_lot::RwLock;
use crate::reward_distribution::{
    RewardDistributor, DistributionConfig, LockBonusConfig, DistributionResult,
    MinerInfo, ValidatorInfo, DelegatorInfo, SubnetInfo
};
use crate::emission::{EmissionController, EmissionConfig, UtilityMetrics};
use crate::burn_manager::{BurnManager, BurnConfig};

/// Pending reward for a participant
#[derive(Debug, Clone, Default)]
pub struct PendingReward {
    pub amount: u128,
    pub last_epoch: u64,
    pub accumulated_from_epoch: u64,
}

/// Reward history entry
#[derive(Debug, Clone)]
pub struct RewardHistoryEntry {
    pub epoch: u64,
    pub amount: u128,
    pub reward_type: RewardType,
    pub claimed: bool,
}

/// Type of reward
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewardType {
    Mining,
    Validation,
    Delegation,
    SubnetOwner,
}

/// Account balance with rewards
#[derive(Debug, Clone, Default)]
pub struct AccountBalance {
    pub available: u128,
    pub pending_rewards: u128,
    pub staked: u128,
    pub locked_until: u64,  // block height
}

/// Reward Executor - processes epochs and credits rewards
pub struct RewardExecutor {
    distributor: RewardDistributor,
    emission: RwLock<EmissionController>,
    burn_manager: BurnManager,

    // Balances: address -> AccountBalance
    balances: RwLock<HashMap<[u8; 20], AccountBalance>>,

    // Pending rewards: address -> PendingReward
    pending_rewards: RwLock<HashMap<[u8; 20], PendingReward>>,

    // Reward history: address -> Vec<RewardHistoryEntry>
    history: RwLock<HashMap<[u8; 20], Vec<RewardHistoryEntry>>>,

    // DAO treasury balance
    dao_balance: RwLock<u128>,

    // Current epoch
    current_epoch: RwLock<u64>,

    // DAO address
    dao_address: [u8; 20],
}

impl RewardExecutor {
    /// Create new reward executor
    pub fn new(dao_address: [u8; 20]) -> Self {
        Self {
            distributor: RewardDistributor::default_with_dao(dao_address),
            emission: RwLock::new(EmissionController::new(EmissionConfig::default())),
            burn_manager: BurnManager::new(BurnConfig::default()),
            balances: RwLock::new(HashMap::new()),
            pending_rewards: RwLock::new(HashMap::new()),
            history: RwLock::new(HashMap::new()),
            dao_balance: RwLock::new(0),
            current_epoch: RwLock::new(0),
            dao_address,
        }
    }

    /// Process end of epoch - calculate and distribute rewards
    pub fn process_epoch(
        &self,
        epoch: u64,
        block_height: u64,
        utility: &UtilityMetrics,
        miners: &[MinerInfo],
        validators: &[ValidatorInfo],
        delegators: &[DelegatorInfo],
        subnets: &[SubnetInfo],
    ) -> EpochResult {
        // Calculate emission for this epoch
        let emission_result = self.emission.write().process_block(block_height, utility);
        let total_emission = emission_result.amount;

        // Distribute rewards according to tokenomics v3
        let distribution = self.distributor.distribute(
            epoch,
            total_emission,
            miners,
            validators,
            delegators,
            subnets,
        );

        // Credit rewards to pending balances
        self.credit_rewards(&distribution, epoch);

        // Update current epoch
        *self.current_epoch.write() = epoch;

        EpochResult {
            epoch,
            total_emission,
            miner_rewards: distribution.miner_rewards.values().sum(),
            validator_rewards: distribution.validator_rewards.values().sum(),
            delegator_rewards: distribution.delegator_rewards.values().sum(),
            subnet_rewards: distribution.subnet_owner_rewards.values().sum(),
            dao_allocation: distribution.dao_allocation,
            participants_rewarded:
                distribution.miner_rewards.len() +
                distribution.validator_rewards.len() +
                distribution.delegator_rewards.len() +
                distribution.subnet_owner_rewards.len(),
        }
    }

    /// Credit rewards to pending balances
    fn credit_rewards(&self, distribution: &DistributionResult, epoch: u64) {
        let mut pending = self.pending_rewards.write();
        let mut history = self.history.write();

        // Helper to credit and record
        let mut credit = |address: [u8; 20], amount: u128, reward_type: RewardType| {
            if amount == 0 {
                return;
            }

            // Update pending
            let entry = pending.entry(address).or_insert(PendingReward {
                amount: 0,
                last_epoch: 0,
                accumulated_from_epoch: epoch,
            });
            entry.amount += amount;
            entry.last_epoch = epoch;

            // Record history
            let addr_history = history.entry(address).or_insert_with(Vec::new);
            addr_history.push(RewardHistoryEntry {
                epoch,
                amount,
                reward_type,
                claimed: false,
            });
        };

        // Credit miners
        for (addr, amount) in &distribution.miner_rewards {
            credit(*addr, *amount, RewardType::Mining);
        }

        // Credit validators
        for (addr, amount) in &distribution.validator_rewards {
            credit(*addr, *amount, RewardType::Validation);
        }

        // Credit delegators
        for (addr, amount) in &distribution.delegator_rewards {
            credit(*addr, *amount, RewardType::Delegation);
        }

        // Credit subnet owners
        for (addr, amount) in &distribution.subnet_owner_rewards {
            credit(*addr, *amount, RewardType::SubnetOwner);
        }

        // Credit DAO treasury
        *self.dao_balance.write() += distribution.dao_allocation;
    }

    /// Claim pending rewards - moves from pending to available balance
    pub fn claim_rewards(&self, address: [u8; 20]) -> ClaimResult {
        let mut pending = self.pending_rewards.write();
        let mut balances = self.balances.write();
        let mut history = self.history.write();

        let pending_amount = pending.get(&address).map(|p| p.amount).unwrap_or(0);

        if pending_amount == 0 {
            return ClaimResult {
                success: false,
                amount: 0,
                new_balance: balances.get(&address).map(|b| b.available).unwrap_or(0),
                message: "No pending rewards to claim".to_string(),
            };
        }

        // Move from pending to available
        pending.remove(&address);

        let balance = balances.entry(address).or_insert(AccountBalance::default());
        balance.available += pending_amount;
        balance.pending_rewards = 0;

        // Mark history entries as claimed
        if let Some(entries) = history.get_mut(&address) {
            for entry in entries.iter_mut() {
                if !entry.claimed {
                    entry.claimed = true;
                }
            }
        }

        ClaimResult {
            success: true,
            amount: pending_amount,
            new_balance: balance.available,
            message: format!("Successfully claimed {} tokens", pending_amount),
        }
    }

    /// Get pending rewards for an address
    pub fn get_pending_rewards(&self, address: [u8; 20]) -> u128 {
        self.pending_rewards.read()
            .get(&address)
            .map(|p| p.amount)
            .unwrap_or(0)
    }

    /// Get account balance
    pub fn get_balance(&self, address: [u8; 20]) -> AccountBalance {
        self.balances.read()
            .get(&address)
            .cloned()
            .unwrap_or_default()
    }

    /// Get reward history for an address
    pub fn get_reward_history(&self, address: [u8; 20], limit: usize) -> Vec<RewardHistoryEntry> {
        self.history.read()
            .get(&address)
            .map(|h| h.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    /// Get DAO treasury balance
    pub fn get_dao_balance(&self) -> u128 {
        *self.dao_balance.read()
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        *self.current_epoch.read()
    }

    /// Get executor statistics
    pub fn stats(&self) -> ExecutorStats {
        let pending = self.pending_rewards.read();
        let balances = self.balances.read();

        ExecutorStats {
            current_epoch: *self.current_epoch.read(),
            total_pending: pending.values().map(|p| p.amount).sum(),
            total_available: balances.values().map(|b| b.available).sum(),
            dao_balance: *self.dao_balance.read(),
            accounts_with_pending: pending.len(),
            total_accounts: balances.len(),
        }
    }

    /// Process transaction fee (burns 50%)
    pub fn process_tx_fee(&self, fee: u128, block_height: u64) -> (u128, u128) {
        self.burn_manager.burn_tx_fee(fee, block_height)
    }

    /// Get burn manager reference
    pub fn burn_manager(&self) -> &BurnManager {
        &self.burn_manager
    }
}

/// Result of epoch processing
#[derive(Debug, Clone)]
pub struct EpochResult {
    pub epoch: u64,
    pub total_emission: u128,
    pub miner_rewards: u128,
    pub validator_rewards: u128,
    pub delegator_rewards: u128,
    pub subnet_rewards: u128,
    pub dao_allocation: u128,
    pub participants_rewarded: usize,
}

/// Result of claiming rewards
#[derive(Debug, Clone)]
pub struct ClaimResult {
    pub success: bool,
    pub amount: u128,
    pub new_balance: u128,
    pub message: String,
}

/// Executor statistics
#[derive(Debug, Clone)]
pub struct ExecutorStats {
    pub current_epoch: u64,
    pub total_pending: u128,
    pub total_available: u128,
    pub dao_balance: u128,
    pub accounts_with_pending: usize,
    pub total_accounts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(id: u8) -> [u8; 20] {
        let mut addr = [0u8; 20];
        addr[0] = id;
        addr
    }

    fn test_utility() -> UtilityMetrics {
        UtilityMetrics {
            transaction_count: 100,
            ai_tasks_completed: 50,
            network_utilization: 0.5,
        }
    }

    #[test]
    fn test_process_epoch() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miners = vec![
            MinerInfo { address: test_address(1), score: 0.8 },
        ];
        let validators = vec![
            ValidatorInfo { address: test_address(10), stake: 1000 },
        ];
        let delegators = vec![];
        let subnets = vec![];

        let result = executor.process_epoch(
            1,
            100,
            &test_utility(),
            &miners,
            &validators,
            &delegators,
            &subnets,
        );

        assert_eq!(result.epoch, 1);
        assert!(result.total_emission > 0);
        assert!(result.miner_rewards > 0);
        assert!(result.validator_rewards > 0);
    }

    #[test]
    fn test_claim_rewards() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miner_addr = test_address(1);
        let miners = vec![
            MinerInfo { address: miner_addr, score: 1.0 },
        ];

        // Process an epoch
        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);

        // Check pending rewards
        let pending = executor.get_pending_rewards(miner_addr);
        assert!(pending > 0, "Miner should have pending rewards");

        // Claim rewards
        let claim_result = executor.claim_rewards(miner_addr);
        assert!(claim_result.success);
        assert_eq!(claim_result.amount, pending);

        // Check balance
        let balance = executor.get_balance(miner_addr);
        assert_eq!(balance.available, pending);

        // Pending should be zero now
        assert_eq!(executor.get_pending_rewards(miner_addr), 0);
    }

    #[test]
    fn test_dao_allocation() {
        let dao_addr = test_address(100);
        let executor = RewardExecutor::new(dao_addr);

        let miners = vec![
            MinerInfo { address: test_address(1), score: 1.0 },
        ];

        executor.process_epoch(1, 100, &test_utility(), &miners, &[], &[], &[]);

        // DAO should have received 13%
        let dao_balance = executor.get_dao_balance();
        assert!(dao_balance > 0, "DAO should have balance");
    }
}
