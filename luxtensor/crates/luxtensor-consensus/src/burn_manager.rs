// Burn Manager Module for Tokenomics v3
// Implements 4 burn mechanisms: tx fees, subnet registration, unmet quota, slashing

use parking_lot::RwLock;

/// Burn configuration
#[derive(Debug, Clone)]
pub struct BurnConfig {
    /// Transaction fee burn rate (50%)
    pub tx_fee_burn_rate: f64,
    /// Subnet registration burn rate (50%, 50% to grants)
    pub subnet_burn_rate: f64,
    /// Unmet quota burn rate (100%)
    pub unmet_quota_burn_rate: f64,
    /// Slashing burn rate (80%)
    pub slashing_burn_rate: f64,
}

impl Default for BurnConfig {
    fn default() -> Self {
        Self {
            tx_fee_burn_rate: 0.50,
            subnet_burn_rate: 0.50,
            unmet_quota_burn_rate: 1.00,
            slashing_burn_rate: 0.80,
        }
    }
}

/// Types of burns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnType {
    TransactionFee,
    SubnetRegistration,
    UnmetQuota,
    Slashing,
}

/// A burn event
#[derive(Debug, Clone)]
pub struct BurnEvent {
    pub burn_type: BurnType,
    pub amount: u128,
    pub block_height: u64,
    pub source: Option<[u8; 20]>,
}

/// Burn statistics
#[derive(Debug, Clone, Default)]
pub struct BurnStats {
    pub total_burned: u128,
    pub tx_fee_burned: u128,
    pub subnet_burned: u128,
    pub quota_burned: u128,
    pub slashing_burned: u128,
    pub recycled_to_grants: u128,
}

/// Internal counters protected by RwLock
struct BurnCounters {
    total_burned: u128,
    tx_fee_burned: u128,
    subnet_burned: u128,
    quota_burned: u128,
    slashing_burned: u128,
    recycled_to_grants: u128,
}

impl Default for BurnCounters {
    fn default() -> Self {
        Self {
            total_burned: 0,
            tx_fee_burned: 0,
            subnet_burned: 0,
            quota_burned: 0,
            slashing_burned: 0,
            recycled_to_grants: 0,
        }
    }
}

/// Burn Manager - tracks and executes burns
pub struct BurnManager {
    config: BurnConfig,
    counters: RwLock<BurnCounters>,
    events: RwLock<Vec<BurnEvent>>,
}

impl BurnManager {
    /// Create new burn manager
    pub fn new(config: BurnConfig) -> Self {
        Self {
            config,
            counters: RwLock::new(BurnCounters::default()),
            events: RwLock::new(Vec::new()),
        }
    }

    /// Process transaction fee - returns (burned, remaining)
    pub fn burn_tx_fee(&self, fee: u128, block_height: u64) -> (u128, u128) {
        let burn_amount = (fee as f64 * self.config.tx_fee_burn_rate) as u128;
        let remaining = fee - burn_amount;

        self.record_burn(BurnType::TransactionFee, burn_amount, block_height, None);

        let mut counters = self.counters.write();
        counters.tx_fee_burned += burn_amount;
        counters.total_burned += burn_amount;

        (burn_amount, remaining)
    }

    /// Process subnet registration - returns (burned, recycled_to_grants)
    pub fn burn_subnet_registration(&self, fee: u128, block_height: u64, owner: [u8; 20]) -> (u128, u128) {
        let burn_amount = (fee as f64 * self.config.subnet_burn_rate) as u128;
        let recycle_amount = fee - burn_amount;

        self.record_burn(BurnType::SubnetRegistration, burn_amount, block_height, Some(owner));

        let mut counters = self.counters.write();
        counters.subnet_burned += burn_amount;
        counters.total_burned += burn_amount;
        counters.recycled_to_grants += recycle_amount;

        (burn_amount, recycle_amount)
    }

    /// Burn for unmet quota (100% burned)
    pub fn burn_unmet_quota(&self, amount: u128, block_height: u64, participant: [u8; 20]) -> u128 {
        let burn_amount = (amount as f64 * self.config.unmet_quota_burn_rate) as u128;

        self.record_burn(BurnType::UnmetQuota, burn_amount, block_height, Some(participant));

        let mut counters = self.counters.write();
        counters.quota_burned += burn_amount;
        counters.total_burned += burn_amount;

        burn_amount
    }

    /// Process slashing - returns (burned, remaining)
    pub fn burn_slashing(&self, slashed_amount: u128, block_height: u64, validator: [u8; 20]) -> (u128, u128) {
        let burn_amount = (slashed_amount as f64 * self.config.slashing_burn_rate) as u128;
        let remaining = slashed_amount - burn_amount;

        self.record_burn(BurnType::Slashing, burn_amount, block_height, Some(validator));

        let mut counters = self.counters.write();
        counters.slashing_burned += burn_amount;
        counters.total_burned += burn_amount;

        (burn_amount, remaining)
    }

    /// Record a burn event
    fn record_burn(&self, burn_type: BurnType, amount: u128, block_height: u64, source: Option<[u8; 20]>) {
        if amount > 0 {
            self.events.write().push(BurnEvent {
                burn_type,
                amount,
                block_height,
                source,
            });
        }
    }

    /// Get burn statistics
    pub fn stats(&self) -> BurnStats {
        let counters = self.counters.read();
        BurnStats {
            total_burned: counters.total_burned,
            tx_fee_burned: counters.tx_fee_burned,
            subnet_burned: counters.subnet_burned,
            quota_burned: counters.quota_burned,
            slashing_burned: counters.slashing_burned,
            recycled_to_grants: counters.recycled_to_grants,
        }
    }

    /// Get recent burn events
    pub fn recent_events(&self, count: usize) -> Vec<BurnEvent> {
        let events = self.events.read();
        events.iter().rev().take(count).cloned().collect()
    }

    /// Get total burned
    pub fn total_burned(&self) -> u128 {
        self.counters.read().total_burned
    }

    /// Get recycled to grants
    pub fn recycled_to_grants(&self) -> u128 {
        self.counters.read().recycled_to_grants
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
    fn test_tx_fee_burn() {
        let manager = BurnManager::new(BurnConfig::default());
        let fee = 1000u128;

        let (burned, remaining) = manager.burn_tx_fee(fee, 1);

        assert_eq!(burned, 500); // 50%
        assert_eq!(remaining, 500);
        assert_eq!(manager.total_burned(), 500);
    }

    #[test]
    fn test_subnet_registration_burn() {
        let manager = BurnManager::new(BurnConfig::default());
        let fee = 1000u128;

        let (burned, recycled) = manager.burn_subnet_registration(fee, 1, test_address(1));

        assert_eq!(burned, 500); // 50% burned
        assert_eq!(recycled, 500); // 50% to grants
        assert_eq!(manager.recycled_to_grants(), 500);
    }

    #[test]
    fn test_slashing_burn() {
        let manager = BurnManager::new(BurnConfig::default());
        let slashed = 1000u128;

        let (burned, remaining) = manager.burn_slashing(slashed, 1, test_address(1));

        assert_eq!(burned, 800); // 80% burned
        assert_eq!(remaining, 200); // 20% to treasury
    }

    #[test]
    fn test_stats() {
        let manager = BurnManager::new(BurnConfig::default());

        manager.burn_tx_fee(100, 1);
        manager.burn_subnet_registration(200, 2, test_address(1));
        manager.burn_slashing(100, 3, test_address(2));

        let stats = manager.stats();
        assert_eq!(stats.tx_fee_burned, 50);
        assert_eq!(stats.subnet_burned, 100);
        assert_eq!(stats.slashing_burned, 80);
        assert_eq!(stats.total_burned, 230);
        assert_eq!(stats.recycled_to_grants, 100);
    }
}
