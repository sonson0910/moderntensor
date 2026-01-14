//! Subnet 0 (Root Subnet) Types
//!
//! This module defines core types for Subnet 0 functionality:
//! - SubnetInfo: Metadata for registered subnets
//! - RootConfig: Configuration for Root Subnet
//! - RootValidatorInfo: Information about root validators
//! - SubnetWeights: Weight votes from validators
//!
//! Synced with Python SDK: sdk/models/subnet.py

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Information about a registered subnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetInfo {
    /// Unique subnet ID (1, 2, 3, ...)
    pub netuid: u16,
    /// Owner address
    pub owner: [u8; 20],
    /// Human-readable name
    pub name: String,
    /// Block number when registered
    pub registered_at: u64,
    /// Total stake in subnet (wei)
    pub total_stake: u128,
    /// Number of active validators
    pub active_validators: u32,
    /// Number of active miners
    pub active_miners: u32,
    /// Current emission share (0.0 - 1.0, stored as basis points 0-10000)
    pub emission_share_bps: u16,
    /// Whether subnet is active
    pub active: bool,
    /// Metadata (JSON string)
    pub metadata: String,
}

impl SubnetInfo {
    pub fn new(netuid: u16, owner: [u8; 20], name: String, registered_at: u64) -> Self {
        Self {
            netuid,
            owner,
            name,
            registered_at,
            total_stake: 0,
            active_validators: 0,
            active_miners: 0,
            emission_share_bps: 0,
            active: true,
            metadata: String::new(),
        }
    }

    /// Get emission share as float
    pub fn emission_share(&self) -> f64 {
        self.emission_share_bps as f64 / 10000.0
    }

    /// Set emission share from float
    pub fn set_emission_share(&mut self, share: f64) {
        self.emission_share_bps = (share * 10000.0).min(10000.0).max(0.0) as u16;
    }
}

/// Configuration for Root Subnet (Subnet 0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    /// Maximum number of subnets
    pub max_subnets: u16,
    /// Top N stakers become root validators
    pub max_root_validators: u16,
    /// Minimum stake to be root validator (wei)
    pub min_stake_for_root: u128,
    /// Cost to register subnet (burned)
    pub subnet_registration_cost: u128,
    /// Blocks between weight updates
    pub weight_update_interval: u64,
    /// Blocks per emission cycle
    pub emission_tempo: u64,
}

impl Default for RootConfig {
    fn default() -> Self {
        Self {
            max_subnets: 32,
            max_root_validators: 64,
            min_stake_for_root: 1_000_000_000_000_000_000_000, // 1000 tokens
            subnet_registration_cost: 100_000_000_000_000_000_000, // 100 tokens
            weight_update_interval: 100,
            emission_tempo: 360,
        }
    }
}

/// Information about a root validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootValidatorInfo {
    /// Validator address
    pub address: [u8; 20],
    /// Total stake amount (wei)
    pub stake: u128,
    /// Rank among root validators (1-64)
    pub rank: u16,
    /// Whether validator is active
    pub is_active: bool,
    /// Block of last weight update
    pub last_weight_update: u64,
}

impl RootValidatorInfo {
    pub fn new(address: [u8; 20], stake: u128, rank: u16) -> Self {
        Self {
            address,
            stake,
            rank,
            is_active: true,
            last_weight_update: 0,
        }
    }
}

/// Weight votes from a root validator for subnets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetWeights {
    /// Validator address
    pub validator: [u8; 20],
    /// Weights: netuid -> weight in basis points (0-10000)
    pub weights: HashMap<u16, u16>,
    /// Block when last updated
    pub block_updated: u64,
}

impl SubnetWeights {
    pub fn new(validator: [u8; 20]) -> Self {
        Self {
            validator,
            weights: HashMap::new(),
            block_updated: 0,
        }
    }

    /// Set weight for a subnet (as float 0.0-1.0)
    pub fn set_weight(&mut self, netuid: u16, weight: f64) {
        let bps = (weight * 10000.0).min(10000.0).max(0.0) as u16;
        self.weights.insert(netuid, bps);
    }

    /// Get weight for a subnet (as float 0.0-1.0)
    pub fn get_weight(&self, netuid: u16) -> f64 {
        self.weights.get(&netuid).copied().unwrap_or(0) as f64 / 10000.0
    }

    /// Normalize weights to sum to 1.0 (10000 bps)
    pub fn normalize(&mut self) {
        let total: u32 = self.weights.values().map(|&v| v as u32).sum();
        if total > 0 {
            for v in self.weights.values_mut() {
                *v = ((*v as u32 * 10000) / total) as u16;
            }
        }
    }
}

/// Emission share for a subnet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmissionShare {
    /// Subnet ID
    pub netuid: u16,
    /// Share in basis points (0-10000)
    pub share_bps: u16,
    /// Actual token amount for epoch (wei)
    pub amount: u128,
}

impl EmissionShare {
    pub fn new(netuid: u16, share: f64, amount: u128) -> Self {
        Self {
            netuid,
            share_bps: (share * 10000.0).min(10000.0).max(0.0) as u16,
            amount,
        }
    }

    /// Get share as float
    pub fn share(&self) -> f64 {
        self.share_bps as f64 / 10000.0
    }
}

/// Result of subnet registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetRegistrationResult {
    /// Whether registration succeeded
    pub success: bool,
    /// Assigned subnet ID (if successful)
    pub netuid: Option<u16>,
    /// Transaction hash
    pub tx_hash: Option<[u8; 32]>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Amount burned for registration
    pub cost_burned: u128,
}

impl SubnetRegistrationResult {
    pub fn success(netuid: u16, cost: u128) -> Self {
        Self {
            success: true,
            netuid: Some(netuid),
            tx_hash: None,
            error: None,
            cost_burned: cost,
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            netuid: None,
            tx_hash: None,
            error: Some(error),
            cost_burned: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subnet_info() {
        let mut subnet = SubnetInfo::new(1, [0u8; 20], "Test".to_string(), 100);
        assert_eq!(subnet.netuid, 1);
        assert!(subnet.active);

        subnet.set_emission_share(0.25);
        assert_eq!(subnet.emission_share_bps, 2500);
        assert!((subnet.emission_share() - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_subnet_weights() {
        let mut weights = SubnetWeights::new([0u8; 20]);
        weights.set_weight(1, 0.5);
        weights.set_weight(2, 0.3);

        assert!((weights.get_weight(1) - 0.5).abs() < 0.001);
        assert!((weights.get_weight(2) - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_root_config_default() {
        let config = RootConfig::default();
        assert_eq!(config.max_subnets, 32);
        assert_eq!(config.max_root_validators, 64);
    }
}
