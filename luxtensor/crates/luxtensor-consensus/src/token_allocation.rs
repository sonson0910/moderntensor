// Token Allocation and Vesting Module
// Implements: Pre-mint with time-locked vesting for all token categories

use std::collections::HashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Total supply: 21,000,000 MDT (18 decimals)
pub const TOTAL_SUPPLY: u128 = 21_000_000_000_000_000_000_000_000;
pub const DECIMALS: u8 = 18;

/// Token allocation categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AllocationCategory {
    /// 45% - Miners, Validators, Infrastructure (emission over 10+ years)
    EmissionRewards,
    /// 12% - Subnet grants, dApp builders (DAO controlled)
    EcosystemGrants,
    /// 10% - Founders and developers
    TeamCoreDev,
    /// 8% - VCs, Angels, Strategic partners
    PrivateSale,
    /// 5% - Community access via launchpad
    IDO,
    /// 10% - Operations, Marketing
    DaoTreasury,
    /// 5% - DEX/CEX trading pairs
    InitialLiquidity,
    /// 5% - Research, Emergency
    FoundationReserve,
}

impl AllocationCategory {
    /// Get allocation percentage (0-100)
    pub fn percentage(&self) -> u8 {
        match self {
            AllocationCategory::EmissionRewards => 45,
            AllocationCategory::EcosystemGrants => 12,
            AllocationCategory::TeamCoreDev => 10,
            AllocationCategory::PrivateSale => 8,
            AllocationCategory::IDO => 5,
            AllocationCategory::DaoTreasury => 10,
            AllocationCategory::InitialLiquidity => 5,
            AllocationCategory::FoundationReserve => 5,
        }
    }

    /// Get total allocated amount
    pub fn amount(&self) -> u128 {
        TOTAL_SUPPLY * self.percentage() as u128 / 100
    }

    /// Get vesting schedule
    pub fn vesting(&self) -> VestingSchedule {
        match self {
            AllocationCategory::EmissionRewards => VestingSchedule {
                cliff_days: 0,
                linear_days: 3650, // 10 years
                tge_percent: 0,
                description: "Emission over 10+ years".to_string(),
            },
            AllocationCategory::EcosystemGrants => VestingSchedule {
                cliff_days: 0,
                linear_days: 0, // DAO controlled
                tge_percent: 0,
                description: "DAO controlled release".to_string(),
            },
            AllocationCategory::TeamCoreDev => VestingSchedule {
                cliff_days: 365, // 1 year cliff
                linear_days: 1460, // 4 years linear
                tge_percent: 0,
                description: "1yr cliff + 4yr linear".to_string(),
            },
            AllocationCategory::PrivateSale => VestingSchedule {
                cliff_days: 365, // 1 year cliff
                linear_days: 730, // 2 years linear
                tge_percent: 0,
                description: "1yr cliff + 2yr linear".to_string(),
            },
            AllocationCategory::IDO => VestingSchedule {
                cliff_days: 0,
                linear_days: 180, // 6 months
                tge_percent: 25, // 25% at TGE
                description: "25% TGE + 6mo linear".to_string(),
            },
            AllocationCategory::DaoTreasury => VestingSchedule {
                cliff_days: 0,
                linear_days: 0, // Multi-sig controlled
                tge_percent: 0,
                description: "Multi-sig controlled".to_string(),
            },
            AllocationCategory::InitialLiquidity => VestingSchedule {
                cliff_days: 0,
                linear_days: 0,
                tge_percent: 100, // Fully liquid for DEX
                description: "Locked in liquidity pool".to_string(),
            },
            AllocationCategory::FoundationReserve => VestingSchedule {
                cliff_days: 0,
                linear_days: 0, // Multi-sig controlled
                tge_percent: 0,
                description: "Multi-sig controlled".to_string(),
            },
        }
    }
}

/// Vesting schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VestingSchedule {
    /// Cliff period in days (no tokens released)
    pub cliff_days: u32,
    /// Linear vesting period in days (after cliff)
    pub linear_days: u32,
    /// Percentage released at TGE (0-100)
    pub tge_percent: u8,
    /// Human-readable description
    pub description: String,
}

impl VestingSchedule {
    /// Calculate vested amount at a given day since TGE
    pub fn vested_amount(&self, total: u128, days_since_tge: u32) -> u128 {
        // TGE unlock
        let tge_amount = total * self.tge_percent as u128 / 100;
        let vesting_amount = total - tge_amount;

        if days_since_tge == 0 {
            return tge_amount;
        }

        // During cliff
        if days_since_tge < self.cliff_days {
            return tge_amount;
        }

        // After cliff, calculate linear vesting
        let days_after_cliff = days_since_tge - self.cliff_days;

        if self.linear_days == 0 {
            // No linear vesting, all unlocked after cliff
            return total;
        }

        // Linear vesting calculation
        let vested = if days_after_cliff >= self.linear_days {
            vesting_amount // Fully vested
        } else {
            vesting_amount * days_after_cliff as u128 / self.linear_days as u128
        };

        tge_amount + vested
    }
}

/// Individual vesting entry for a beneficiary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VestingEntry {
    pub beneficiary: [u8; 20],
    pub category: AllocationCategory,
    pub total_amount: u128,
    pub claimed_amount: u128,
    pub tge_timestamp: u64,
    pub revoked: bool,
}

impl VestingEntry {
    /// Calculate claimable amount
    pub fn claimable(&self, current_timestamp: u64) -> u128 {
        if self.revoked {
            return 0;
        }

        let days_since_tge = ((current_timestamp - self.tge_timestamp) / 86400) as u32;
        let schedule = self.category.vesting();
        let vested = schedule.vested_amount(self.total_amount, days_since_tge);

        vested.saturating_sub(self.claimed_amount)
    }
}

/// Token Allocation Manager
/// Handles pre-minting at TGE and vesting release
pub struct TokenAllocation {
    /// TGE timestamp
    tge_timestamp: u64,
    /// Pre-minted amounts per category
    minted: RwLock<HashMap<AllocationCategory, u128>>,
    /// Individual vesting entries
    vesting_entries: RwLock<Vec<VestingEntry>>,
    /// Total supply minted
    total_minted: RwLock<u128>,
    /// Emission pool (for gradual emission)
    emission_pool: RwLock<u128>,
}

impl TokenAllocation {
    /// Create new token allocation manager
    pub fn new(tge_timestamp: u64) -> Self {
        Self {
            tge_timestamp,
            minted: RwLock::new(HashMap::new()),
            vesting_entries: RwLock::new(Vec::new()),
            total_minted: RwLock::new(0),
            emission_pool: RwLock::new(0),
        }
    }

    /// Execute Token Generation Event (TGE)
    /// Pre-mints tokens for all categories except EmissionRewards
    pub fn execute_tge(&self) -> TgeResult {
        let mut minted = self.minted.write();
        let mut total = self.total_minted.write();
        let mut result = TgeResult::default();

        // Pre-mint for each category
        for category in [
            AllocationCategory::EcosystemGrants,
            AllocationCategory::TeamCoreDev,
            AllocationCategory::PrivateSale,
            AllocationCategory::IDO,
            AllocationCategory::DaoTreasury,
            AllocationCategory::InitialLiquidity,
            AllocationCategory::FoundationReserve,
        ] {
            let amount = category.amount();
            minted.insert(category, amount);
            *total += amount;
            result.pre_minted.push((category, amount));
        }

        // EmissionRewards are NOT pre-minted
        // They are minted gradually through mining/staking
        *self.emission_pool.write() = AllocationCategory::EmissionRewards.amount();
        result.emission_reserved = AllocationCategory::EmissionRewards.amount();

        result.total_pre_minted = *total;
        result.tge_timestamp = self.tge_timestamp;
        result
    }

    /// Add vesting entry for a beneficiary
    pub fn add_vesting(
        &self,
        beneficiary: [u8; 20],
        category: AllocationCategory,
        amount: u128,
    ) -> Result<(), &'static str> {
        // Check category has enough balance
        let mut minted = self.minted.write();
        let available = minted.get(&category).copied().unwrap_or(0);

        if amount > available {
            return Err("Insufficient allocation balance");
        }

        // Deduct from category pool
        minted.insert(category, available - amount);

        // Create vesting entry
        let entry = VestingEntry {
            beneficiary,
            category,
            total_amount: amount,
            claimed_amount: 0,
            tge_timestamp: self.tge_timestamp,
            revoked: false,
        };

        self.vesting_entries.write().push(entry);
        Ok(())
    }

    /// Claim vested tokens
    pub fn claim(&self, beneficiary: [u8; 20], current_timestamp: u64) -> ClaimResult {
        let mut entries = self.vesting_entries.write();
        let mut total_claimed = 0u128;

        for entry in entries.iter_mut() {
            if entry.beneficiary == beneficiary {
                let claimable = entry.claimable(current_timestamp);
                if claimable > 0 {
                    entry.claimed_amount += claimable;
                    total_claimed += claimable;
                }
            }
        }

        ClaimResult {
            beneficiary,
            amount_claimed: total_claimed,
            timestamp: current_timestamp,
        }
    }

    /// Get vested amount for beneficiary
    pub fn get_vested(&self, beneficiary: [u8; 20], current_timestamp: u64) -> u128 {
        let entries = self.vesting_entries.read();
        entries.iter()
            .filter(|e| e.beneficiary == beneficiary)
            .map(|e| {
                let days = ((current_timestamp - e.tge_timestamp) / 86400) as u32;
                e.category.vesting().vested_amount(e.total_amount, days)
            })
            .sum()
    }

    /// Get claimable amount for beneficiary
    pub fn get_claimable(&self, beneficiary: [u8; 20], current_timestamp: u64) -> u128 {
        let entries = self.vesting_entries.read();
        entries.iter()
            .filter(|e| e.beneficiary == beneficiary)
            .map(|e| e.claimable(current_timestamp))
            .sum()
    }

    /// Mint from emission pool (for rewards)
    pub fn mint_emission(&self, amount: u128) -> Result<u128, &'static str> {
        let mut pool = self.emission_pool.write();
        if amount > *pool {
            return Err("Emission pool exhausted");
        }
        *pool -= amount;
        *self.total_minted.write() += amount;
        Ok(amount)
    }

    /// Get remaining emission pool
    pub fn remaining_emission(&self) -> u128 {
        *self.emission_pool.read()
    }

    /// Get allocation stats
    pub fn stats(&self) -> AllocationStats {
        let minted = self.minted.read();
        AllocationStats {
            total_supply: TOTAL_SUPPLY,
            total_pre_minted: *self.total_minted.read(),
            emission_remaining: *self.emission_pool.read(),
            allocations: minted.iter().map(|(k, v)| (*k, *v)).collect(),
        }
    }
}

/// TGE execution result
#[derive(Debug, Default, Serialize)]
pub struct TgeResult {
    pub tge_timestamp: u64,
    pub pre_minted: Vec<(AllocationCategory, u128)>,
    pub total_pre_minted: u128,
    pub emission_reserved: u128,
}

/// Claim result
#[derive(Debug, Serialize)]
pub struct ClaimResult {
    pub beneficiary: [u8; 20],
    pub amount_claimed: u128,
    pub timestamp: u64,
}

/// Allocation statistics
#[derive(Debug, Serialize)]
pub struct AllocationStats {
    pub total_supply: u128,
    pub total_pre_minted: u128,
    pub emission_remaining: u128,
    pub allocations: Vec<(AllocationCategory, u128)>,
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
    fn test_allocation_percentages() {
        let total: u8 = [
            AllocationCategory::EmissionRewards,
            AllocationCategory::EcosystemGrants,
            AllocationCategory::TeamCoreDev,
            AllocationCategory::PrivateSale,
            AllocationCategory::IDO,
            AllocationCategory::DaoTreasury,
            AllocationCategory::InitialLiquidity,
            AllocationCategory::FoundationReserve,
        ].iter().map(|c| c.percentage()).sum();

        assert_eq!(total, 100);
    }

    #[test]
    fn test_tge_execution() {
        let allocation = TokenAllocation::new(1_700_000_000);
        let result = allocation.execute_tge();

        // 55% pre-minted (100% - 45% emission)
        let expected = TOTAL_SUPPLY * 55 / 100;
        assert_eq!(result.total_pre_minted, expected);

        // Emission pool reserved
        assert_eq!(result.emission_reserved, TOTAL_SUPPLY * 45 / 100);
    }

    #[test]
    fn test_vesting_cliff() {
        let schedule = AllocationCategory::PrivateSale.vesting();

        // Day 0: Nothing
        assert_eq!(schedule.vested_amount(1000, 0), 0);

        // Day 364: Still in cliff
        assert_eq!(schedule.vested_amount(1000, 364), 0);

        // Day 365: Cliff ends, linear starts
        assert!(schedule.vested_amount(1000, 365) > 0);

        // Day 365 + 730: Fully vested
        assert_eq!(schedule.vested_amount(1000, 365 + 730), 1000);
    }

    #[test]
    fn test_ido_tge_unlock() {
        let schedule = AllocationCategory::IDO.vesting();

        // 25% at TGE
        assert_eq!(schedule.vested_amount(1000, 0), 250);

        // 100% after 6 months
        assert_eq!(schedule.vested_amount(1000, 180), 1000);
    }

    #[test]
    fn test_claim_flow() {
        let allocation = TokenAllocation::new(1_700_000_000);
        allocation.execute_tge();

        let investor = test_address(1);
        let amount = 100_000_000_000_000_000_000u128; // 100 MDT

        // Add vesting for private sale investor
        allocation.add_vesting(investor, AllocationCategory::PrivateSale, amount).unwrap();

        // Day 0: Nothing claimable (cliff)
        let claimable = allocation.get_claimable(investor, 1_700_000_000);
        assert_eq!(claimable, 0);

        // After 1 year + 1 day: Some claimable
        let one_year = 1_700_000_000 + (366 * 86400);
        let claimable = allocation.get_claimable(investor, one_year);
        assert!(claimable > 0);
    }
}
