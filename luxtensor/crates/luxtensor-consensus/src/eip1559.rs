//! EIP-1559 Dynamic Fee Pricing
//!
//! Implements EIP-1559 fee market mechanism for LuxTensor blockchain.
//! This provides predictable gas pricing with network congestion adjustment.
//!
//! # Algorithm
//! ```text
//! if parent_gas_used > target_gas_used:
//!     base_fee = parent_base_fee * (1 + change_rate)
//! else:
//!     base_fee = parent_base_fee * (1 - change_rate)
//!
//! where change_rate = |parent_gas_used - target_gas_used| / target_gas_used * max_change_rate
//! ```
//!
//! # Constants
//! - Target = 50% of block gas limit
//! - Max change rate = 12.5% per block
//! - Min base fee = 0.1 gwei (spam protection)
//! - Max base fee = 100 gwei (emergency cap)

/// EIP-1559 fee configuration
#[derive(Debug, Clone)]
pub struct Eip1559Config {
    /// Block gas limit
    pub block_gas_limit: u64,
    /// Target gas usage (typically 50% of limit)
    pub target_gas_used: u64,
    /// Base fee in wei (starting value)
    pub initial_base_fee: u128,
    /// Maximum change denominator (8 = 12.5% max change)
    pub base_fee_max_change_denominator: u64,
    /// Minimum base fee (wei)
    pub min_base_fee: u128,
    /// Maximum base fee (wei)
    pub max_base_fee: u128,
}

impl Default for Eip1559Config {
    fn default() -> Self {
        Self {
            block_gas_limit: 30_000_000,
            target_gas_used: 15_000_000, // 50% target
            initial_base_fee: 500_000_000, // 0.5 gwei - balanced for AI+DeFi
            base_fee_max_change_denominator: 8, // 12.5% max change
            min_base_fee: 100_000_000, // 0.1 gwei - spam protection but accessible
            max_base_fee: 100_000_000_000, // 100 gwei emergency cap (was 10000)
        }
    }
}

/// Fee market state for tracking dynamic pricing
#[derive(Debug, Clone)]
pub struct FeeMarket {
    /// Current base fee per gas (wei)
    pub base_fee: u128,
    /// Configuration
    pub config: Eip1559Config,
    /// Last block's gas used
    pub last_gas_used: u64,
    /// Current block number
    pub block_number: u64,
}

impl FeeMarket {
    /// Create new fee market with default config
    pub fn new() -> Self {
        let config = Eip1559Config::default();
        Self {
            base_fee: config.initial_base_fee,
            config,
            last_gas_used: 0,
            block_number: 0,
        }
    }

    /// Create with custom config
    pub fn with_config(config: Eip1559Config) -> Self {
        let base_fee = config.initial_base_fee;
        Self {
            base_fee,
            config,
            last_gas_used: 0,
            block_number: 0,
        }
    }

    /// Calculate next block's base fee based on parent block's gas usage
    ///
    /// Implements EIP-1559 base fee adjustment:
    /// - If block is more than 50% full, increase base fee
    /// - If block is less than 50% full, decrease base fee
    /// - Maximum change is 12.5% per block
    pub fn calculate_next_base_fee(&self, parent_gas_used: u64) -> u128 {
        let target = self.config.target_gas_used;
        let denominator = self.config.base_fee_max_change_denominator as u128;

        if parent_gas_used == target {
            // Exactly at target, no change
            return self.base_fee;
        }

        let new_base_fee = if parent_gas_used > target {
            // Block was more than target, increase base fee
            let gas_used_delta = (parent_gas_used - target) as u128;
            let base_fee_delta = self.base_fee * gas_used_delta / target as u128 / denominator;
            // Ensure minimum increase of 1 wei if there's any overflow
            self.base_fee.saturating_add(base_fee_delta.max(1))
        } else {
            // Block was less than target, decrease base fee
            let gas_used_delta = (target - parent_gas_used) as u128;
            let base_fee_delta = self.base_fee * gas_used_delta / target as u128 / denominator;
            self.base_fee.saturating_sub(base_fee_delta)
        };

        // Clamp to valid range
        new_base_fee
            .max(self.config.min_base_fee)
            .min(self.config.max_base_fee)
    }

    /// Update fee market after block is produced
    pub fn on_block_produced(&mut self, gas_used: u64) {
        let new_base_fee = self.calculate_next_base_fee(gas_used);
        self.base_fee = new_base_fee;
        self.last_gas_used = gas_used;
        self.block_number += 1;
    }

    /// Calculate effective gas price for transaction
    ///
    /// # Parameters
    /// - `max_fee_per_gas`: Maximum fee user is willing to pay (total)
    /// - `max_priority_fee_per_gas`: Maximum tip user is willing to pay to validator
    ///
    /// # Returns
    /// - `Some((effective_price, tip))` if transaction is valid
    /// - `None` if max_fee_per_gas < base_fee
    pub fn calculate_effective_gas_price(
        &self,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Option<(u128, u128)> {
        if max_fee_per_gas < self.base_fee {
            // User can't afford current base fee
            return None;
        }

        // Priority fee = min(max_priority_fee, max_fee - base_fee)
        let priority_fee = max_priority_fee_per_gas.min(max_fee_per_gas - self.base_fee);

        // Effective price = base_fee + priority_fee
        let effective_price = self.base_fee.saturating_add(priority_fee);

        Some((effective_price, priority_fee))
    }

    /// Get current base fee
    pub fn current_base_fee(&self) -> u128 {
        self.base_fee
    }

    /// Estimate max_fee_per_gas for fast inclusion (2x current base fee)
    pub fn estimate_fast_max_fee(&self) -> u128 {
        self.base_fee.saturating_mul(2)
    }

    /// Estimate max_fee_per_gas for normal inclusion (1.5x current base fee)
    pub fn estimate_normal_max_fee(&self) -> u128 {
        self.base_fee + self.base_fee / 2
    }

    /// Estimate max_fee_per_gas for slow inclusion (1.1x current base fee)
    pub fn estimate_slow_max_fee(&self) -> u128 {
        self.base_fee + self.base_fee / 10
    }

    /// Get suggested priority fee based on recent block congestion
    pub fn suggested_priority_fee(&self) -> u128 {
        // Simple heuristic: 1-5 gwei based on last block's usage
        let usage_ratio = self.last_gas_used as f64 / self.config.target_gas_used as f64;

        let priority_gwei = if usage_ratio > 1.5 {
            5 // Very congested
        } else if usage_ratio > 1.0 {
            3 // Above target
        } else if usage_ratio > 0.5 {
            2 // Normal
        } else {
            1 // Light traffic
        };

        priority_gwei * 1_000_000_000 // Convert to wei
    }
}

impl Default for FeeMarket {
    fn default() -> Self {
        Self::new()
    }
}

/// Fee estimation response (for RPC eth_feeHistory)
#[derive(Debug, Clone)]
pub struct FeeHistory {
    /// Base fee per gas for each block
    pub base_fees: Vec<u128>,
    /// Gas used ratio for each block (0.0 to 1.0+)
    pub gas_used_ratios: Vec<f64>,
    /// Oldest block number
    pub oldest_block: u64,
    /// Reward percentiles if requested
    pub reward: Option<Vec<Vec<u128>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Eip1559Config::default();
        assert_eq!(config.target_gas_used, 15_000_000);
        assert_eq!(config.initial_base_fee, 500_000_000); // 0.5 gwei
    }

    #[test]
    fn test_base_fee_at_target() {
        let market = FeeMarket::new();
        let target = market.config.target_gas_used;

        let next_fee = market.calculate_next_base_fee(target);
        assert_eq!(next_fee, market.base_fee);
    }

    #[test]
    fn test_base_fee_increase() {
        let market = FeeMarket::new();

        // Use 100% of gas limit (double target)
        let full_block = market.config.block_gas_limit;
        let next_fee = market.calculate_next_base_fee(full_block);

        // Should increase by ~12.5%
        assert!(next_fee > market.base_fee);
        assert!(next_fee <= market.base_fee + market.base_fee / 8);
    }

    #[test]
    fn test_base_fee_decrease() {
        let market = FeeMarket::new();

        // Empty block
        let next_fee = market.calculate_next_base_fee(0);

        // Should decrease by ~12.5%
        assert!(next_fee < market.base_fee);
        assert!(next_fee >= market.base_fee - market.base_fee / 8);
    }

    #[test]
    fn test_min_base_fee() {
        let config = Eip1559Config {
            initial_base_fee: 1_000_000_000,
            min_base_fee: 1_000_000_000,
            ..Default::default()
        };
        let market = FeeMarket::with_config(config);

        // Many empty blocks should not go below min
        let next = market.calculate_next_base_fee(0);
        assert!(next >= market.config.min_base_fee);
    }

    #[test]
    fn test_effective_gas_price() {
        let market = FeeMarket::new();

        // User willing to pay 2 gwei max, 0.5 gwei priority
        let max_fee = 2_000_000_000u128;
        let max_priority = 500_000_000u128;

        let result = market.calculate_effective_gas_price(max_fee, max_priority);
        assert!(result.is_some());

        let (effective, tip) = result.unwrap();
        assert_eq!(effective, market.base_fee + tip);
        assert!(tip <= max_priority);
    }

    #[test]
    fn test_insufficient_max_fee() {
        let market = FeeMarket::new();

        // User can't afford base fee
        let max_fee = market.base_fee / 2;
        let result = market.calculate_effective_gas_price(max_fee, 0);

        assert!(result.is_none());
    }

    #[test]
    fn test_on_block_produced() {
        let mut market = FeeMarket::new();
        let initial_fee = market.base_fee;

        // Produce a full block
        market.on_block_produced(market.config.block_gas_limit);

        assert!(market.base_fee > initial_fee);
        assert_eq!(market.block_number, 1);
    }

    #[test]
    fn test_fee_estimates() {
        let market = FeeMarket::new();

        let fast = market.estimate_fast_max_fee();
        let normal = market.estimate_normal_max_fee();
        let slow = market.estimate_slow_max_fee();

        assert!(fast > normal);
        assert!(normal > slow);
        assert!(slow > market.base_fee);
    }
}
