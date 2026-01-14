//! Root Subnet (Subnet 0) State Management
//!
//! Manages all registered subnets, root validators, and weight voting
//! for emission distribution across the ModernTensor network.
//!
//! Synced with Python SDK: sdk/root_subnet.py

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use luxtensor_core::{
    SubnetInfo, RootConfig, RootValidatorInfo, SubnetWeights,
    EmissionShare, SubnetRegistrationResult
};
use tracing::{info, warn};

/// Root Subnet state
pub struct RootSubnetState {
    /// Registered subnets: netuid -> SubnetInfo
    pub subnets: HashMap<u16, SubnetInfo>,

    /// Root validators (top stakers)
    pub root_validators: Vec<RootValidatorInfo>,

    /// Weight votes: validator address hex -> SubnetWeights
    pub weight_matrix: HashMap<String, SubnetWeights>,

    /// Computed emission shares: netuid -> share (0-10000 bps)
    pub emission_shares: HashMap<u16, u16>,

    /// Configuration
    pub config: RootConfig,

    /// Next available subnet ID
    next_netuid: u16,

    /// Last weight calculation block
    last_weight_update: u64,
}

impl Default for RootSubnetState {
    fn default() -> Self {
        Self::new()
    }
}

impl RootSubnetState {
    pub fn new() -> Self {
        Self {
            subnets: HashMap::new(),
            root_validators: Vec::new(),
            weight_matrix: HashMap::new(),
            emission_shares: HashMap::new(),
            config: RootConfig::default(),
            next_netuid: 1,
            last_weight_update: 0,
        }
    }

    /// Register a new subnet
    pub fn register_subnet(
        &mut self,
        name: String,
        owner: [u8; 20],
        block_number: u64,
    ) -> SubnetRegistrationResult {
        // Check max subnets
        if self.subnets.len() >= self.config.max_subnets as usize {
            return SubnetRegistrationResult::failure(
                format!("Maximum subnets ({}) reached", self.config.max_subnets)
            );
        }

        // Assign netuid
        let netuid = self.next_netuid;
        self.next_netuid += 1;

        // Create subnet
        let subnet = SubnetInfo::new(netuid, owner, name.clone(), block_number);
        self.subnets.insert(netuid, subnet);

        // Initialize emission share
        self.emission_shares.insert(netuid, 0);

        info!("Registered subnet {}: {} (owner: {:?})", netuid, name, owner);

        SubnetRegistrationResult::success(netuid, self.config.subnet_registration_cost)
    }

    /// Deregister a subnet
    pub fn deregister_subnet(&mut self, netuid: u16, caller: [u8; 20]) -> Result<(), String> {
        let subnet = self.subnets.get(&netuid)
            .ok_or_else(|| format!("Subnet {} not found", netuid))?;

        if subnet.owner != caller {
            return Err("Caller is not subnet owner".to_string());
        }

        self.subnets.remove(&netuid);
        self.emission_shares.remove(&netuid);

        // Remove from weight matrix
        for weights in self.weight_matrix.values_mut() {
            weights.weights.remove(&netuid);
        }

        info!("Deregistered subnet {}", netuid);
        Ok(())
    }

    /// Update root validators from top stakers
    pub fn update_root_validators(&mut self, stakers: Vec<([u8; 20], u128)>) {
        self.root_validators.clear();

        for (rank, (address, stake)) in stakers.iter().enumerate() {
            if rank >= self.config.max_root_validators as usize {
                break;
            }
            if *stake >= self.config.min_stake_for_root {
                self.root_validators.push(RootValidatorInfo::new(
                    *address,
                    *stake,
                    (rank + 1) as u16
                ));
            }
        }

        info!("Updated root validators: {} validators", self.root_validators.len());
    }

    /// Check if address is a root validator
    pub fn is_root_validator(&self, address: &[u8; 20]) -> bool {
        self.root_validators.iter().any(|v| &v.address == address)
    }

    /// Set subnet weights for a validator
    pub fn set_weights(
        &mut self,
        validator: [u8; 20],
        weights: HashMap<u16, u16>,
        block_number: u64,
    ) -> Result<(), String> {
        if !self.is_root_validator(&validator) {
            return Err("Not a root validator".to_string());
        }

        // Validate total <= 10000 bps
        let total: u32 = weights.values().map(|&v| v as u32).sum();
        if total > 10000 {
            return Err(format!("Weights sum {} exceeds 10000", total));
        }

        // Validate netuids exist
        for netuid in weights.keys() {
            if !self.subnets.contains_key(netuid) {
                return Err(format!("Subnet {} does not exist", netuid));
            }
        }

        // Store weights
        let addr_hex = hex::encode(validator);
        let mut subnet_weights = SubnetWeights::new(validator);
        subnet_weights.weights = weights;
        subnet_weights.block_updated = block_number;
        self.weight_matrix.insert(addr_hex.clone(), subnet_weights);

        // Recalculate emissions
        self.calculate_emission_shares();

        info!("Updated weights for validator {}", addr_hex);
        Ok(())
    }

    /// Calculate emission shares based on stake-weighted votes
    fn calculate_emission_shares(&mut self) {
        if self.root_validators.is_empty() || self.weight_matrix.is_empty() {
            return;
        }

        let total_stake: u128 = self.root_validators.iter().map(|v| v.stake).sum();
        if total_stake == 0 {
            return;
        }

        // Calculate weighted sum for each subnet
        let mut shares: HashMap<u16, u128> = HashMap::new();

        for validator in &self.root_validators {
            let addr_hex = hex::encode(validator.address);
            if let Some(weights) = self.weight_matrix.get(&addr_hex) {
                for (netuid, weight_bps) in &weights.weights {
                    let weighted_vote = validator.stake * (*weight_bps as u128);
                    *shares.entry(*netuid).or_insert(0) += weighted_vote;
                }
            }
        }

        // Normalize to basis points
        let total_weighted: u128 = shares.values().sum();
        if total_weighted > 0 {
            for netuid in self.subnets.keys() {
                let weighted = shares.get(netuid).copied().unwrap_or(0);
                let share_bps = ((weighted * 10000) / total_weighted) as u16;
                self.emission_shares.insert(*netuid, share_bps);
            }
        }
    }

    /// Get emission distribution for all subnets
    pub fn get_emission_distribution(&self, total_emission: u128) -> Vec<EmissionShare> {
        self.emission_shares
            .iter()
            .map(|(netuid, share_bps)| {
                let share = *share_bps as f64 / 10000.0;
                let amount = (total_emission as f64 * share) as u128;
                EmissionShare::new(*netuid, share, amount)
            })
            .collect()
    }

    /// Get all subnets
    pub fn get_all_subnets(&self) -> Vec<&SubnetInfo> {
        self.subnets.values().collect()
    }

    /// Get subnet by netuid
    pub fn get_subnet(&self, netuid: u16) -> Option<&SubnetInfo> {
        self.subnets.get(&netuid)
    }

    /// Get root validator count
    pub fn get_root_validator_count(&self) -> usize {
        self.root_validators.len()
    }
}

/// Thread-safe Root Subnet manager
pub type RootSubnet = Arc<RwLock<RootSubnetState>>;

/// Create new Root Subnet instance
pub fn new_root_subnet() -> RootSubnet {
    Arc::new(RwLock::new(RootSubnetState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_subnet() {
        let mut state = RootSubnetState::new();
        let owner = [1u8; 20];

        let result = state.register_subnet("Test Subnet".to_string(), owner, 100);
        assert!(result.success);
        assert_eq!(result.netuid, Some(1));

        let subnet = state.get_subnet(1).unwrap();
        assert_eq!(subnet.name, "Test Subnet");
    }

    #[test]
    fn test_weight_voting() {
        let mut state = RootSubnetState::new();
        let owner = [1u8; 20];
        let validator = [2u8; 20];

        // Register subnet
        state.register_subnet("Test".to_string(), owner, 100);

        // Add validator
        state.root_validators.push(RootValidatorInfo::new(validator, 1000, 1));

        // Set weights
        let mut weights = HashMap::new();
        weights.insert(1u16, 5000u16); // 50%

        let result = state.set_weights(validator, weights, 200);
        assert!(result.is_ok());
    }
}
