// Node Tier System for Progressive Staking (Model C)
// 4 Tiers: Light Node, Full Node, Validator, Super Validator
// Enhanced with GPU bonus and logarithmic stake curve for fair economics

use parking_lot::RwLock;
use std::collections::HashMap;

/// Calculate effective stake using logarithmic curve for whale protection
/// This ensures diminishing returns for very large stakes:
/// - 1,000 LUX → 1,000 effective (100%)
/// - 10,000 LUX → ~8,500 effective (85%)
/// - 100,000 LUX → ~50,000 effective (50%)
/// - 1,000,000 LUX → ~100,000 effective (10%)
pub fn logarithmic_stake(stake: u128) -> u128 {
    if stake == 0 {
        return 0;
    }

    // Use f64 for calculation - stake in base units (18 decimals)
    let stake_f = stake as f64;

    // Normalize to "human readable" units for log calculation
    // Stake is in 18 decimals, so divide by 10^18 for logarithm
    let normalized = stake_f / 1e18;

    if normalized <= 1.0 {
        // Small stakes: no reduction
        return stake;
    }

    // Formula: stake * ln(normalized + 1) / (normalized + 1)
    // This naturally reduces effectiveness for larger stakes
    let log_factor = (normalized + 1.0).ln() / (normalized + 1.0);

    // Apply factor, ensuring minimum of 10% effectiveness for very large stakes
    let effective_factor = log_factor.max(0.10);

    // SECURITY: Clamp f64 before casting to u128 to avoid NaN/negative
    let raw = stake_f * effective_factor;
    if raw.is_nan() || raw < 0.0 {
        0u128
    } else if raw > u128::MAX as f64 {
        u128::MAX
    } else {
        raw as u128
    }
}

/// Minimum stake requirements for each tier (in base units)
pub const LIGHT_NODE_STAKE: u128 = 0; // No stake required
pub const FULL_NODE_STAKE: u128 = 10_000_000_000_000_000_000; // 10 MDT
pub const VALIDATOR_STAKE: u128 = 100_000_000_000_000_000_000; // 100 MDT
pub const SUPER_VALIDATOR_STAKE: u128 = 1_000_000_000_000_000_000_000; // 1000 MDT

/// Node tier levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeTier {
    /// Tier 0: Light node - sync and relay, tx fee sharing
    LightNode = 0,
    /// Tier 1: Full node - full validation, 2% infrastructure emission
    FullNode = 1,
    /// Tier 2: Validator - AI quality validation, 28% validator emission
    Validator = 2,
    /// Tier 3: Super validator - block production priority, delegation
    SuperValidator = 3,
}

impl NodeTier {
    /// Get minimum stake for this tier
    pub fn min_stake(&self) -> u128 {
        match self {
            NodeTier::LightNode => LIGHT_NODE_STAKE,
            NodeTier::FullNode => FULL_NODE_STAKE,
            NodeTier::Validator => VALIDATOR_STAKE,
            NodeTier::SuperValidator => SUPER_VALIDATOR_STAKE,
        }
    }

    /// Get tier from stake amount
    pub fn from_stake(stake: u128) -> Self {
        if stake >= SUPER_VALIDATOR_STAKE {
            NodeTier::SuperValidator
        } else if stake >= VALIDATOR_STAKE {
            NodeTier::Validator
        } else if stake >= FULL_NODE_STAKE {
            NodeTier::FullNode
        } else {
            NodeTier::LightNode
        }
    }

    /// Get reward share for this tier
    pub fn emission_share(&self) -> f64 {
        match self {
            NodeTier::LightNode => 0.0,       // No emission, only tx fees
            NodeTier::FullNode => 0.02,       // 2% infrastructure
            NodeTier::Validator => 0.28,      // 28% validation
            NodeTier::SuperValidator => 0.28, // Same as validator + priority fees
        }
    }

    /// Get tier name
    pub fn name(&self) -> &'static str {
        match self {
            NodeTier::LightNode => "Light Node",
            NodeTier::FullNode => "Full Node",
            NodeTier::Validator => "Validator",
            NodeTier::SuperValidator => "Super Validator",
        }
    }

    /// Check if tier can produce blocks
    pub fn can_produce_blocks(&self) -> bool {
        matches!(self, NodeTier::Validator | NodeTier::SuperValidator)
    }

    /// Check if tier receives infrastructure rewards
    pub fn receives_infrastructure_rewards(&self) -> bool {
        matches!(self, NodeTier::FullNode | NodeTier::Validator | NodeTier::SuperValidator)
    }

    /// Check if tier receives validator rewards
    pub fn receives_validator_rewards(&self) -> bool {
        matches!(self, NodeTier::Validator | NodeTier::SuperValidator)
    }
}

/// GPU capability for AI nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GpuCapability {
    #[default]
    None,
    /// Basic GPU (RTX 3060 class) - +20% bonus
    Basic,
    /// Advanced GPU (RTX 4080 class) - +30% bonus
    Advanced,
    /// Professional GPU (A100 class) - +40% bonus (capped)
    Professional,
}

impl GpuCapability {
    /// Get GPU bonus multiplier (capped at 40%)
    pub fn bonus_multiplier(&self) -> f64 {
        match self {
            GpuCapability::None => 1.0,          // No bonus
            GpuCapability::Basic => 1.20,        // +20%
            GpuCapability::Advanced => 1.30,     // +30%
            GpuCapability::Professional => 1.40, // +40% (capped)
        }
    }

    /// Check if node has GPU
    pub fn has_gpu(&self) -> bool {
        !matches!(self, GpuCapability::None)
    }
}

/// Registered node info
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub address: [u8; 20],
    pub tier: NodeTier,
    pub stake: u128,
    pub registered_at: u64, // block height
    pub last_active: u64,   // block height
    pub uptime_score: f64,  // 0.0 - 1.0
    pub blocks_produced: u64,
    pub tx_relayed: u64,
    /// GPU capability for AI tasks
    pub gpu: GpuCapability,
    /// AI tasks completed
    pub ai_tasks_completed: u64,
}

impl NodeInfo {
    pub fn new(address: [u8; 20], stake: u128, block_height: u64) -> Self {
        Self {
            address,
            tier: NodeTier::from_stake(stake),
            stake,
            registered_at: block_height,
            last_active: block_height,
            uptime_score: 1.0,
            blocks_produced: 0,
            tx_relayed: 0,
            gpu: GpuCapability::None,
            ai_tasks_completed: 0,
        }
    }

    /// Create node with GPU capability
    pub fn new_with_gpu(
        address: [u8; 20],
        stake: u128,
        block_height: u64,
        gpu: GpuCapability,
    ) -> Self {
        Self { gpu, ..Self::new(address, stake, block_height) }
    }

    /// Calculate effective stake using logarithmic curve for whale protection
    /// Formula: stake * ln(stake + 1) / (stake + 1)
    /// This gives diminishing returns for very large stakes
    pub fn effective_stake(&self) -> u128 {
        logarithmic_stake(self.stake)
    }

    /// Calculate effective stake with GPU bonus
    pub fn effective_stake_with_gpu(&self) -> u128 {
        let base = self.effective_stake();
        // SECURITY: Clamp f64 before casting to u128
        let raw = base as f64 * self.gpu.bonus_multiplier();
        if raw.is_nan() || raw < 0.0 {
            0u128
        } else if raw > u128::MAX as f64 {
            u128::MAX
        } else {
            raw as u128
        }
    }

    /// Set GPU capability
    pub fn set_gpu(&mut self, gpu: GpuCapability) {
        self.gpu = gpu;
    }

    /// Record AI task completion
    pub fn record_ai_task(&mut self) {
        self.ai_tasks_completed += 1;
    }

    /// Update stake and recalculate tier
    pub fn update_stake(&mut self, new_stake: u128) {
        self.stake = new_stake;
        self.tier = NodeTier::from_stake(new_stake);
    }

    /// Record block production
    pub fn record_block(&mut self, block_height: u64) {
        self.blocks_produced += 1;
        self.last_active = block_height;
    }

    /// Record transaction relay
    pub fn record_tx_relay(&mut self, count: u64) {
        self.tx_relayed += count;
    }

    /// Update uptime score
    pub fn update_uptime(&mut self, score: f64) {
        // Exponential moving average
        self.uptime_score = self.uptime_score * 0.95 + score * 0.05;
    }
}

/// Node Registry - tracks all registered nodes
pub struct NodeRegistry {
    nodes: RwLock<HashMap<[u8; 20], NodeInfo>>,
    nodes_by_tier: RwLock<HashMap<NodeTier, Vec<[u8; 20]>>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self { nodes: RwLock::new(HashMap::new()), nodes_by_tier: RwLock::new(HashMap::new()) }
    }

    /// Register a new node
    /// SECURITY: Hold nodes.write() across both map inserts to prevent ghost entries
    /// from concurrent register/unregister races.
    pub fn register(
        &self,
        address: [u8; 20],
        stake: u128,
        block_height: u64,
    ) -> Result<NodeTier, &'static str> {
        let node = NodeInfo::new(address, stake, block_height);
        let tier = node.tier;

        // Add to nodes map — hold lock across both inserts
        let mut nodes = self.nodes.write();
        nodes.insert(address, node);

        // Add to tier index while still holding nodes lock
        self.nodes_by_tier.write().entry(tier).or_insert_with(Vec::new).push(address);

        drop(nodes); // explicit drop for clarity

        Ok(tier)
    }

    /// Update node stake
    pub fn update_stake(&self, address: [u8; 20], new_stake: u128) -> Option<NodeTier> {
        let mut nodes = self.nodes.write();
        if let Some(node) = nodes.get_mut(&address) {
            let old_tier = node.tier;
            node.update_stake(new_stake);
            let new_tier = node.tier;

            // Update tier index if changed
            if old_tier != new_tier {
                let mut by_tier = self.nodes_by_tier.write();
                if let Some(list) = by_tier.get_mut(&old_tier) {
                    list.retain(|a| a != &address);
                }
                by_tier.entry(new_tier).or_insert_with(Vec::new).push(address);
            }

            Some(new_tier)
        } else {
            None
        }
    }

    /// Unregister a node
    /// SECURITY: Hold nodes.write() across both map removals to prevent
    /// ghost entries from concurrent register/unregister races.
    pub fn unregister(&self, address: [u8; 20]) -> Option<NodeInfo> {
        let mut nodes = self.nodes.write();
        let node = nodes.remove(&address);
        if let Some(ref n) = node {
            self.nodes_by_tier.write().get_mut(&n.tier).map(|list| list.retain(|a| a != &address));
        }
        drop(nodes); // explicit drop after both maps updated
        node
    }

    /// Get node info
    pub fn get(&self, address: [u8; 20]) -> Option<NodeInfo> {
        self.nodes.read().get(&address).cloned()
    }

    /// Get node tier
    pub fn get_tier(&self, address: [u8; 20]) -> Option<NodeTier> {
        self.nodes.read().get(&address).map(|n| n.tier)
    }

    /// Get all nodes in a tier
    /// LOCK ORDER: nodes → nodes_by_tier (consistent with update_stake/register)
    pub fn get_by_tier(&self, tier: NodeTier) -> Vec<NodeInfo> {
        let nodes = self.nodes.read();
        let by_tier = self.nodes_by_tier.read();

        by_tier
            .get(&tier)
            .map(|addrs| addrs.iter().filter_map(|a| nodes.get(a).cloned()).collect())
            .unwrap_or_default()
    }

    /// Get all full nodes (infrastructure providers)
    pub fn get_infrastructure_nodes(&self) -> Vec<NodeInfo> {
        let mut nodes = self.get_by_tier(NodeTier::FullNode);
        nodes.extend(self.get_by_tier(NodeTier::Validator));
        nodes.extend(self.get_by_tier(NodeTier::SuperValidator));
        nodes
    }

    /// Get all validators
    pub fn get_validators(&self) -> Vec<NodeInfo> {
        let mut nodes = self.get_by_tier(NodeTier::Validator);
        nodes.extend(self.get_by_tier(NodeTier::SuperValidator));
        nodes
    }

    /// Get super validators only
    pub fn get_super_validators(&self) -> Vec<NodeInfo> {
        self.get_by_tier(NodeTier::SuperValidator)
    }

    /// Count nodes by tier
    pub fn count_by_tier(&self) -> HashMap<NodeTier, usize> {
        let by_tier = self.nodes_by_tier.read();
        let mut counts = HashMap::new();
        for tier in
            [NodeTier::LightNode, NodeTier::FullNode, NodeTier::Validator, NodeTier::SuperValidator]
        {
            counts.insert(tier, by_tier.get(&tier).map(|v| v.len()).unwrap_or(0));
        }
        counts
    }

    /// Total registered nodes
    pub fn total_nodes(&self) -> usize {
        self.nodes.read().len()
    }

    /// Total stake across all nodes
    pub fn total_stake(&self) -> u128 {
        self.nodes.read().values().map(|n| n.stake).sum()
    }

    /// Record block production for a node
    pub fn record_block_production(&self, address: [u8; 20], block_height: u64) {
        if let Some(node) = self.nodes.write().get_mut(&address) {
            node.record_block(block_height);
        }
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
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
    fn test_tier_from_stake() {
        assert_eq!(NodeTier::from_stake(0), NodeTier::LightNode);
        assert_eq!(NodeTier::from_stake(FULL_NODE_STAKE - 1), NodeTier::LightNode);
        assert_eq!(NodeTier::from_stake(FULL_NODE_STAKE), NodeTier::FullNode);
        assert_eq!(NodeTier::from_stake(VALIDATOR_STAKE), NodeTier::Validator);
        assert_eq!(NodeTier::from_stake(SUPER_VALIDATOR_STAKE), NodeTier::SuperValidator);
    }

    #[test]
    fn test_registry() {
        let registry = NodeRegistry::new();

        // Register nodes
        let tier1 = registry.register(test_address(1), 0, 1).unwrap();
        let tier2 = registry.register(test_address(2), FULL_NODE_STAKE, 1).unwrap();
        let tier3 = registry.register(test_address(3), VALIDATOR_STAKE, 1).unwrap();

        assert_eq!(tier1, NodeTier::LightNode);
        assert_eq!(tier2, NodeTier::FullNode);
        assert_eq!(tier3, NodeTier::Validator);

        // Check counts
        let counts = registry.count_by_tier();
        assert_eq!(counts[&NodeTier::LightNode], 1);
        assert_eq!(counts[&NodeTier::FullNode], 1);
        assert_eq!(counts[&NodeTier::Validator], 1);
    }

    #[test]
    fn test_stake_upgrade() {
        let registry = NodeRegistry::new();

        // Start as light node
        registry.register(test_address(1), 0, 1).unwrap();
        assert_eq!(registry.get_tier(test_address(1)), Some(NodeTier::LightNode));

        // Upgrade to full node
        registry.update_stake(test_address(1), FULL_NODE_STAKE);
        assert_eq!(registry.get_tier(test_address(1)), Some(NodeTier::FullNode));

        // Upgrade to validator
        registry.update_stake(test_address(1), VALIDATOR_STAKE);
        assert_eq!(registry.get_tier(test_address(1)), Some(NodeTier::Validator));
    }

    #[test]
    fn test_infrastructure_nodes() {
        let registry = NodeRegistry::new();

        registry.register(test_address(1), 0, 1).unwrap(); // Light
        registry.register(test_address(2), FULL_NODE_STAKE, 1).unwrap(); // Full
        registry.register(test_address(3), VALIDATOR_STAKE, 1).unwrap(); // Validator

        let infra = registry.get_infrastructure_nodes();
        assert_eq!(infra.len(), 2); // Full + Validator, not Light
    }
}
