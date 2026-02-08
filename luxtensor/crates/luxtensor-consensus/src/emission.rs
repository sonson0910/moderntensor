// Emission controller module for adaptive tokenomics
// Implements halving schedule and utility-based emission adjustments

use serde::{Deserialize, Serialize};

/// Emission configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionConfig {
    /// Total maximum supply
    pub max_supply: u128,
    /// Initial emission per block
    pub initial_emission: u128,
    /// Halving interval in blocks
    pub halving_interval: u64,
    /// Minimum emission per block (floor)
    pub min_emission: u128,
    /// Utility score weight (0-100)
    pub utility_weight: u8,
}

impl Default for EmissionConfig {
    fn default() -> Self {
        Self {
            max_supply: 21_000_000_000_000_000_000_000_000u128, // 21 million tokens
            initial_emission: 2_000_000_000_000_000_000u128,   // 2 tokens/block
            // 🔧 FIX: Aligned with HalvingSchedule (8,760,000 blocks ≈ 3.33 years @ 12s)
            // Corrected from 1,051,200 which was for 100s blocks — 8× faster halving at 12s
            halving_interval: 8_760_000,
            // 🔧 FIX: Aligned with HalvingSchedule MINIMUM_REWARD (0.001 MDT)
            // Previously 0.1 MDT — 100x higher than halving.rs
            min_emission: 1_000_000_000_000_000u128,           // 0.001 tokens minimum
            utility_weight: 30,                                 // 30% adjustment based on utility
        }
    }
}

/// Network utility metrics for emission adjustment
#[derive(Debug, Clone, Default)]
pub struct UtilityMetrics {
    /// Active validators count
    pub active_validators: u64,
    /// Active subnets count
    pub active_subnets: u64,
    /// Transactions in last epoch
    pub epoch_transactions: u64,
    /// AI tasks completed in last epoch
    pub epoch_ai_tasks: u64,
    /// Average block utilization (0-100)
    pub block_utilization: u8,
}

impl UtilityMetrics {
    /// Calculate utility score (0.0 - 2.0)
    /// Score > 1.0 means high utility, < 1.0 means low utility
    pub fn utility_score(&self) -> f64 {
        // Base score from validator participation
        let validator_score = (self.active_validators as f64 / 100.0).min(1.0);

        // Transaction activity score
        let tx_score = (self.epoch_transactions as f64 / 10000.0).min(1.0);

        // AI task score (unique to ModernTensor)
        let ai_score = (self.epoch_ai_tasks as f64 / 1000.0).min(1.0);

        // Block utilization
        let util_score = self.block_utilization as f64 / 100.0;

        // Weighted average
        let base_score = validator_score * 0.3 + tx_score * 0.2 + ai_score * 0.3 + util_score * 0.2;

        // Normalize to 0.5 - 1.5 range
        0.5 + base_score
    }
}

/// Emission controller
#[derive(Debug, Clone)]
pub struct EmissionController {
    config: EmissionConfig,
    /// Current total supply (minted so far)
    current_supply: u128,
    /// Current halving epoch
    halving_epoch: u32,
}

impl EmissionController {
    /// Create new emission controller
    pub fn new(config: EmissionConfig) -> Self {
        Self {
            config,
            current_supply: 0,
            halving_epoch: 0,
        }
    }

    /// Create with existing supply (for resuming)
    pub fn with_supply(config: EmissionConfig, current_supply: u128) -> Self {
        Self {
            config,
            current_supply,
            halving_epoch: 0,
        }
    }

    /// Calculate base emission for a block height (before utility adjustment)
    pub fn base_emission(&self, block_height: u64) -> u128 {
        // Calculate how many halvings have occurred
        let halvings = block_height / self.config.halving_interval;

        // 🔧 FIX: Cap at 10 halvings (aligned with HalvingSchedule::MAX_HALVINGS)
        // Previously looped up to 64, diverging from halving.rs which caps at 10 and returns 0
        // Now: after 10 halvings, emission settles at min_emission (tail emission) instead of 0
        let effective_halvings = halvings.min(10);

        // Calculate halved emission
        let mut emission = self.config.initial_emission;
        for _ in 0..effective_halvings {
            emission = emission / 2;
        }

        // Apply floor — tail emission ensures perpetual validator incentives
        emission.max(self.config.min_emission)
    }

    /// Calculate adjusted emission based on utility
    pub fn adjusted_emission(&self, block_height: u64, utility: &UtilityMetrics) -> u128 {
        let base = self.base_emission(block_height);
        let utility_score = utility.utility_score();

        // Calculate adjustment factor using integer BPS to avoid f64 precision loss on u128
        // utility_score range: 0.5 - 1.5 → utility_bps range: 5000 - 15000
        let utility_bps = (utility_score * 10_000.0).round() as i64;
        let weight = self.config.utility_weight as i64; // 0-100

        // adjustment_bps = 10_000 + (utility_bps - 10_000) * weight / 100
        // Range: ~7000 to ~13000 (0.7x to 1.3x)
        let adjustment_bps = 10_000i64 + (utility_bps - 10_000) * weight / 100;
        let adjustment_bps = adjustment_bps.max(0) as u128;

        // Apply adjustment using integer math
        let adjusted = base * adjustment_bps / 10_000;

        // Ensure we don't exceed remaining supply
        let remaining = self.config.max_supply.saturating_sub(self.current_supply);
        adjusted.min(remaining)
    }

    /// Process block emission and return amount to mint
    pub fn process_block(&mut self, block_height: u64, utility: &UtilityMetrics) -> EmissionResult {
        let emission = self.adjusted_emission(block_height, utility);

        // Update supply
        self.current_supply = self.current_supply.saturating_add(emission);

        // Check for halving
        let new_halving_epoch = (block_height / self.config.halving_interval) as u32;
        let halving_occurred = new_halving_epoch > self.halving_epoch;
        if halving_occurred {
            self.halving_epoch = new_halving_epoch;
        }

        EmissionResult {
            amount: emission,
            block_height,
            current_supply: self.current_supply,
            halving_epoch: self.halving_epoch,
            halving_occurred,
            utility_score: utility.utility_score(),
        }
    }

    /// Get current supply
    pub fn current_supply(&self) -> u128 {
        self.current_supply
    }

    /// Get remaining supply to mint
    pub fn remaining_supply(&self) -> u128 {
        self.config.max_supply.saturating_sub(self.current_supply)
    }

    /// Get current halving epoch
    pub fn halving_epoch(&self) -> u32 {
        self.halving_epoch
    }

    /// Get blocks until next halving
    pub fn blocks_until_halving(&self, current_height: u64) -> u64 {
        let next_halving = (self.halving_epoch as u64 + 1) * self.config.halving_interval;
        next_halving.saturating_sub(current_height)
    }

    /// Get emission statistics
    pub fn stats(&self, current_height: u64) -> EmissionStats {
        EmissionStats {
            current_supply: self.current_supply,
            max_supply: self.config.max_supply,
            remaining_supply: self.remaining_supply(),
            halving_epoch: self.halving_epoch,
            blocks_until_halving: self.blocks_until_halving(current_height),
            current_base_emission: self.base_emission(current_height),
            supply_percentage: (self.current_supply as f64 / self.config.max_supply as f64) * 100.0,
        }
    }
}

/// Result of emission processing
#[derive(Debug, Clone)]
pub struct EmissionResult {
    /// Amount emitted
    pub amount: u128,
    /// Block height
    pub block_height: u64,
    /// Current total supply after emission
    pub current_supply: u128,
    /// Current halving epoch
    pub halving_epoch: u32,
    /// Whether halving occurred at this block
    pub halving_occurred: bool,
    /// Utility score used
    pub utility_score: f64,
}

/// Emission statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionStats {
    pub current_supply: u128,
    pub max_supply: u128,
    pub remaining_supply: u128,
    pub halving_epoch: u32,
    pub blocks_until_halving: u64,
    pub current_base_emission: u128,
    pub supply_percentage: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_emission() {
        let config = EmissionConfig::default();
        let controller = EmissionController::new(config.clone());

        // Initial emission
        assert_eq!(controller.base_emission(0), config.initial_emission);

        // Before first halving
        assert_eq!(controller.base_emission(config.halving_interval - 1), config.initial_emission);

        // After first halving
        assert_eq!(controller.base_emission(config.halving_interval), config.initial_emission / 2);

        // After second halving
        assert_eq!(controller.base_emission(config.halving_interval * 2), config.initial_emission / 4);
    }

    #[test]
    fn test_utility_adjustment() {
        let config = EmissionConfig::default();
        let controller = EmissionController::new(config);

        let low_utility = UtilityMetrics::default();
        let high_utility = UtilityMetrics {
            active_validators: 100,
            active_subnets: 10,
            epoch_transactions: 10000,
            epoch_ai_tasks: 1000,
            block_utilization: 80,
        };

        let low_emission = controller.adjusted_emission(0, &low_utility);
        let high_emission = controller.adjusted_emission(0, &high_utility);

        // High utility should get more emission
        assert!(high_emission > low_emission,
            "high {} should > low {}", high_emission, low_emission);
    }

    #[test]
    fn test_process_block() {
        let config = EmissionConfig::default();
        let mut controller = EmissionController::new(config);

        let utility = UtilityMetrics::default();

        let result = controller.process_block(0, &utility);

        assert!(result.amount > 0);
        assert_eq!(result.current_supply, result.amount);
        assert!(!result.halving_occurred);
    }

    #[test]
    fn test_halving_detection() {
        let config = EmissionConfig {
            halving_interval: 10, // Short for testing
            ..Default::default()
        };
        let mut controller = EmissionController::new(config);

        let utility = UtilityMetrics::default();

        // Process blocks until halving
        for i in 0..9 {
            let result = controller.process_block(i, &utility);
            assert!(!result.halving_occurred, "No halving at block {}", i);
        }

        // Halving should occur at block 10
        let result = controller.process_block(10, &utility);
        assert!(result.halving_occurred);
        assert_eq!(result.halving_epoch, 1);
    }

    #[test]
    fn test_max_supply_cap() {
        let config = EmissionConfig {
            max_supply: 100,
            initial_emission: 50,
            min_emission: 10,
            utility_weight: 0, // Disable utility adjustment for this test
            ..Default::default()
        };
        let mut controller = EmissionController::new(config);

        let utility = UtilityMetrics::default();

        // First block
        controller.process_block(0, &utility);
        assert!(controller.current_supply() > 0);

        // Process more blocks until max reached
        for i in 1..10 {
            controller.process_block(i, &utility);
        }

        // Eventually should cap at max_supply
        assert!(controller.current_supply() <= 100);
    }
}
