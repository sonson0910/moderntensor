// Block Reward Halving Schedule
// Implements Bitcoin-like halving to ensure sustainable token emission
//
// Design:
// - Initial reward: 2 MDT per block
// - Halving interval: 1,051,200 blocks (~3.3 years with 100s avg block time)
// - Total emission from rewards: 45% of 21M = 9.45M MDT
// - After ~10 halvings, reward becomes negligible

use serde::{Deserialize, Serialize};

/// Initial block reward: 0.24 MDT (with 18 decimals)
/// Scaled from 2 MDT for 100s blocks → 0.24 MDT for 12s blocks
/// to preserve the same annual emission (~631K MDT/year)
pub const INITIAL_BLOCK_REWARD: u128 = 240_000_000_000_000_000;

/// Halving interval in blocks
/// With 12-second block times: 8,760,000 blocks ≈ 3.33 years
/// (= 1,051,200 × 100/12, adjusted from original 100s design)
/// This gives us roughly 10 halvings over 33 years
pub const HALVING_INTERVAL: u64 = 8_760_000;

/// Minimum reward threshold (0.001 MDT) - below this, reward is 0
pub const MINIMUM_REWARD: u128 = 1_000_000_000_000_000;

/// Maximum number of halvings (after this, reward is 0)
pub const MAX_HALVINGS: u32 = 10;

/// Halving schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalvingSchedule {
    /// Initial block reward
    pub initial_reward: u128,
    /// Number of blocks between halvings
    pub halving_interval: u64,
    /// Minimum reward threshold
    pub minimum_reward: u128,
    /// Maximum number of halvings
    pub max_halvings: u32,
}

impl Default for HalvingSchedule {
    fn default() -> Self {
        Self {
            initial_reward: INITIAL_BLOCK_REWARD,
            halving_interval: HALVING_INTERVAL,
            minimum_reward: MINIMUM_REWARD,
            max_halvings: MAX_HALVINGS,
        }
    }
}

impl HalvingSchedule {
    /// Create a new halving schedule with custom parameters
    pub fn new(
        initial_reward: u128,
        halving_interval: u64,
        minimum_reward: u128,
        max_halvings: u32,
    ) -> Self {
        Self {
            initial_reward,
            halving_interval,
            minimum_reward,
            max_halvings,
        }
    }

    /// Calculate block reward for a given block height
    ///
    /// Formula: reward = initial_reward / 2^halvings
    /// Where: halvings = min(block_height / halving_interval, max_halvings)
    ///
    /// # Examples
    /// - Block 0:          0.24 MDT
    /// - Block 8,760,000:  0.12 MDT (first halving)
    /// - Block 17,520,000: 0.06 MDT (second halving)
    pub fn calculate_reward(&self, block_height: u64) -> u128 {
        let halvings = (block_height / self.halving_interval) as u32;

        // Cap at max halvings
        let effective_halvings = halvings.min(self.max_halvings);

        // After max halvings, reward is 0 (emission ends)
        // Note: EmissionController handles tail emission via its own min_emission floor
        if halvings > self.max_halvings {
            return 0;
        }

        // Calculate reward: initial_reward >> halvings (divide by 2^halvings)
        let reward = self.initial_reward >> effective_halvings;

        // Return 0 if below minimum threshold
        if reward < self.minimum_reward {
            0
        } else {
            reward
        }
    }

    /// Get the current halving era (0 = before first halving)
    pub fn get_halving_era(&self, block_height: u64) -> u32 {
        ((block_height / self.halving_interval) as u32).min(self.max_halvings)
    }

    /// Calculate remaining blocks until next halving
    pub fn blocks_until_next_halving(&self, block_height: u64) -> u64 {
        let current_era = self.get_halving_era(block_height);
        if current_era >= self.max_halvings {
            return 0; // No more halvings
        }

        let next_halving_block = (current_era as u64 + 1) * self.halving_interval;
        next_halving_block.saturating_sub(block_height)
    }

    /// Calculate total emitted tokens up to a block height
    /// This is the sum of geometric series for each halving era
    pub fn total_emitted(&self, block_height: u64) -> u128 {
        let mut total: u128 = 0;
        let mut current_block: u64 = 0;

        for era in 0..=self.max_halvings {
            let era_start = era as u64 * self.halving_interval;
            let era_end = ((era + 1) as u64 * self.halving_interval).min(block_height);

            if current_block >= block_height {
                break;
            }

            if era_start < block_height {
                let blocks_in_era = era_end.saturating_sub(era_start).min(block_height.saturating_sub(era_start));
                let reward_per_block = self.initial_reward >> era;

                if reward_per_block >= self.minimum_reward {
                    total = total.saturating_add((blocks_in_era as u128).saturating_mul(reward_per_block));
                }
            }

            current_block = era_end;
        }

        total
    }

    /// Estimate total supply from block rewards (after all halvings)
    pub fn estimate_total_emission(&self) -> u128 {
        // Geometric series sum: a * (1 - r^n) / (1 - r) where r = 0.5
        // ≈ 2 * initial_reward * halving_interval (for r = 0.5)
        let blocks_per_halving = self.halving_interval as u128;
        let mut total: u128 = 0;

        for era in 0..=self.max_halvings {
            let reward = self.initial_reward >> era;
            if reward >= self.minimum_reward {
                total = total.saturating_add(blocks_per_halving.saturating_mul(reward));
            }
        }

        total
    }

    /// Get halving schedule info as a human-readable summary
    pub fn summary(&self) -> HalvingInfo {
        HalvingInfo {
            initial_reward_mdt: self.initial_reward as f64 / 1e18,
            halving_interval_blocks: self.halving_interval,
            // 🔧 FIX: Use 12s block time (not 100s from original design)
            // At 12s blocks: 8,760,000 blocks × 12s = 105,120,000s ≈ 3.33 years
            halving_interval_years: (self.halving_interval as f64 * 12.0) / (365.25 * 24.0 * 3600.0),
            max_halvings: self.max_halvings,
            estimated_total_emission_mdt: self.estimate_total_emission() as f64 / 1e18,
        }
    }
}

/// Human-readable halving info
#[derive(Debug, Clone, Serialize)]
pub struct HalvingInfo {
    pub initial_reward_mdt: f64,
    pub halving_interval_blocks: u64,
    pub halving_interval_years: f64,
    pub max_halvings: u32,
    pub estimated_total_emission_mdt: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_reward() {
        let schedule = HalvingSchedule::default();

        // Block 0 should get full reward
        assert_eq!(schedule.calculate_reward(0), INITIAL_BLOCK_REWARD);

        // Block before first halving should get full reward
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL - 1), INITIAL_BLOCK_REWARD);
    }

    #[test]
    fn test_first_halving() {
        let schedule = HalvingSchedule::default();

        // First halving - reward should be half
        let expected = INITIAL_BLOCK_REWARD / 2;
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL), expected);
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL + 1), expected);
    }

    #[test]
    fn test_multiple_halvings() {
        let schedule = HalvingSchedule::default();

        // Test several halvings
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL * 0), INITIAL_BLOCK_REWARD);      // 2 MDT
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL * 1), INITIAL_BLOCK_REWARD / 2);  // 1 MDT
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL * 2), INITIAL_BLOCK_REWARD / 4);  // 0.5 MDT
        assert_eq!(schedule.calculate_reward(HALVING_INTERVAL * 3), INITIAL_BLOCK_REWARD / 8);  // 0.25 MDT
    }

    #[test]
    fn test_max_halvings() {
        let schedule = HalvingSchedule::default();

        // After max halvings, reward should be 0
        let after_max = (MAX_HALVINGS + 1) as u64 * HALVING_INTERVAL;
        assert_eq!(schedule.calculate_reward(after_max), 0);
    }

    #[test]
    fn test_halving_era() {
        let schedule = HalvingSchedule::default();

        assert_eq!(schedule.get_halving_era(0), 0);
        assert_eq!(schedule.get_halving_era(HALVING_INTERVAL - 1), 0);
        assert_eq!(schedule.get_halving_era(HALVING_INTERVAL), 1);
        assert_eq!(schedule.get_halving_era(HALVING_INTERVAL * 5), 5);
    }

    #[test]
    fn test_blocks_until_next_halving() {
        let schedule = HalvingSchedule::default();

        // At block 0, should be full interval until first halving
        assert_eq!(schedule.blocks_until_next_halving(0), HALVING_INTERVAL);

        // At block HALVING_INTERVAL - 1, should be 1 block
        assert_eq!(schedule.blocks_until_next_halving(HALVING_INTERVAL - 1), 1);

        // At first halving, should be full interval until next
        assert_eq!(schedule.blocks_until_next_halving(HALVING_INTERVAL), HALVING_INTERVAL);
    }

    #[test]
    fn test_total_emission_estimate() {
        let schedule = HalvingSchedule::default();
        let info = schedule.summary();

        // Total emission should be approximately:
        // 2 * halving_interval * (1 + 0.5 + 0.25 + ... + 0.5^10) ≈ 4 * halving_interval
        // For 1,051,200 blocks: ~4.2M MDT from block rewards
        // This is within the 45% emission allocation of 9.45M MDT
        println!("Estimated total emission: {} MDT", info.estimated_total_emission_mdt);
        assert!(info.estimated_total_emission_mdt > 0.0);
        assert!(info.estimated_total_emission_mdt < 10_000_000.0); // Should be less than 10M
    }
}
