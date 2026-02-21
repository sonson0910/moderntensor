//! # Agent Registry — ERC-6551 Inspired Token Bound Accounts for AI Agents
//!
//! Manages registration and lifecycle of AI agents that can act autonomously
//! on-chain through smart contract wallets with configurable triggers.
//!
//! ## Architecture
//! - Each agent has a `wallet_address` (Token Bound Account) that holds funds
//! - Agents register with a `trigger_config` specifying execution intervals
//! - Gas deposits fund automatic trigger execution
//! - Bounded registrations prevent DoS (default: 1000 max agents)
//!
//! ## Flow
//! 1. Owner deploys agent contract → registers via `register_agent()`
//! 2. Owner deposits gas via `deposit_gas()`
//! 3. `AgentTriggerEngine` calls `get_triggered_agents()` each block
//! 4. Triggered agents execute their contract logic autonomously

use luxtensor_core::types::Address;
use parking_lot::RwLock;
use std::collections::HashMap;
use tracing::{info, warn, debug};

// ──────────────────────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────────────────────

/// Trigger configuration for an AI Agent.
///
/// Defines when and how an agent should be automatically executed.
#[derive(Debug, Clone)]
pub struct AgentTriggerConfig {
    /// Execute trigger every N blocks (0 = disabled).
    pub block_interval: u64,
    /// Maximum gas allowed per trigger execution.
    pub gas_limit_per_trigger: u64,
    /// Calldata (ABI-encoded function call) to invoke on trigger.
    pub trigger_calldata: Vec<u8>,
    /// Whether the trigger is currently enabled.
    pub enabled: bool,
}

impl Default for AgentTriggerConfig {
    fn default() -> Self {
        Self {
            block_interval: 0,
            gas_limit_per_trigger: 500_000, // 500k gas default
            trigger_calldata: Vec::new(),
            enabled: false,
        }
    }
}

/// AI Agent record — an autonomous on-chain entity.
///
/// Inspired by ERC-6551 Token Bound Accounts: each agent controls
/// its own smart contract wallet and can execute transactions autonomously.
#[derive(Debug, Clone)]
pub struct AgentAccount {
    /// Unique 32-byte agent ID (keccak of owner + nonce).
    pub agent_id: [u8; 32],
    /// Owner of this agent (human or contract that created it).
    pub owner: Address,
    /// Agent's smart contract wallet address (Token Bound Account).
    /// Holds the agent's funds and receives trigger gas refunds.
    pub wallet_address: Address,
    /// Address of the agent's logic contract (EVM bytecode).
    pub contract_address: Address,
    /// Trigger configuration.
    pub trigger_config: AgentTriggerConfig,
    /// Block at which this agent was registered.
    pub registered_at: u64,
    /// Total gas consumed by all trigger executions.
    pub total_gas_used: u64,
    /// Remaining gas deposit for trigger execution (in wei).
    pub gas_deposit: u128,
    /// Last block at which this agent was triggered.
    pub last_triggered_block: u64,
}

// ──────────────────────────────────────────────────────────────────────────────
// Registry
// ──────────────────────────────────────────────────────────────────────────────

/// Global configuration for the agent registry.
#[derive(Debug, Clone)]
pub struct AgentRegistryConfig {
    /// Maximum gas budget for ALL agent triggers in a single block.
    /// Prevents excessive block execution time.
    pub max_trigger_gas_per_block: u64,
    /// Maximum number of agents allowed in the registry.
    pub max_agents: usize,
    /// Minimum gas deposit required to register an agent.
    pub min_gas_deposit: u128,
}

impl Default for AgentRegistryConfig {
    fn default() -> Self {
        Self {
            max_trigger_gas_per_block: 5_000_000, // 5M gas per block for triggers
            max_agents: 1_000,
            min_gas_deposit: 1_000_000_000_000_000, // 0.001 token
        }
    }
}

/// Registry managing all AI Agents.
///
/// Thread-safe via `parking_lot::RwLock` for fast, non-async access
/// in the hot consensus path.
pub struct AgentRegistry {
    /// agent_id → AgentAccount
    agents: RwLock<HashMap<[u8; 32], AgentAccount>>,
    /// Configuration
    config: AgentRegistryConfig,
}

/// Error type for registry operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentRegistryError {
    #[error("Agent registry is full (max {0} agents)")]
    RegistryFull(usize),

    #[error("Agent {0} already exists")]
    AlreadyExists(String),

    #[error("Agent {0} not found")]
    NotFound(String),

    #[error("Insufficient gas deposit: required {required}, provided {provided}")]
    InsufficientDeposit { required: u128, provided: u128 },

    #[error("Only the agent owner can perform this action")]
    NotOwner,

    #[error("Invalid trigger configuration: {0}")]
    InvalidConfig(String),
}

impl AgentRegistry {
    /// Create a new registry with the given configuration.
    pub fn new(config: AgentRegistryConfig) -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Create a registry with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(AgentRegistryConfig::default())
    }

    // ── Registration ────────────────────────────────────────────────────

    /// Register a new AI Agent.
    ///
    /// # Arguments
    /// * `agent_id` - Unique 32-byte identifier
    /// * `owner` - Address of the owner
    /// * `wallet_address` - Agent's Token Bound Account address
    /// * `contract_address` - Address of the agent's logic contract
    /// * `trigger_config` - Trigger execution config
    /// * `gas_deposit` - Initial gas deposit (wei)
    /// * `current_block` - Current block number
    pub fn register_agent(
        &self,
        agent_id: [u8; 32],
        owner: Address,
        wallet_address: Address,
        contract_address: Address,
        trigger_config: AgentTriggerConfig,
        gas_deposit: u128,
        current_block: u64,
    ) -> Result<(), AgentRegistryError> {
        // Validate
        if gas_deposit < self.config.min_gas_deposit {
            return Err(AgentRegistryError::InsufficientDeposit {
                required: self.config.min_gas_deposit,
                provided: gas_deposit,
            });
        }

        if trigger_config.block_interval == 0 && trigger_config.enabled {
            return Err(AgentRegistryError::InvalidConfig(
                "block_interval must be > 0 when trigger is enabled".to_string(),
            ));
        }

        let mut agents = self.agents.write();

        if agents.len() >= self.config.max_agents {
            return Err(AgentRegistryError::RegistryFull(self.config.max_agents));
        }

        if agents.contains_key(&agent_id) {
            return Err(AgentRegistryError::AlreadyExists(hex::encode(agent_id)));
        }

        let account = AgentAccount {
            agent_id,
            owner,
            wallet_address,
            contract_address,
            trigger_config,
            registered_at: current_block,
            total_gas_used: 0,
            gas_deposit,
            last_triggered_block: 0,
        };

        agents.insert(agent_id, account);

        info!(
            agent_id = hex::encode(agent_id),
            owner = ?owner,
            "AI Agent registered"
        );

        Ok(())
    }

    /// Deregister (remove) an AI Agent. Only the owner can do this.
    pub fn deregister_agent(
        &self,
        agent_id: &[u8; 32],
        caller: &Address,
    ) -> Result<AgentAccount, AgentRegistryError> {
        let mut agents = self.agents.write();

        let agent = agents
            .get(agent_id)
            .ok_or_else(|| AgentRegistryError::NotFound(hex::encode(agent_id)))?;

        if &agent.owner != caller {
            return Err(AgentRegistryError::NotOwner);
        }

        let removed = agents.remove(agent_id).unwrap();

        info!(
            agent_id = hex::encode(agent_id),
            remaining_deposit = removed.gas_deposit,
            "AI Agent deregistered"
        );

        Ok(removed)
    }

    // ── Gas Deposit ─────────────────────────────────────────────────────

    /// Deposit additional gas for an agent.
    pub fn deposit_gas(
        &self,
        agent_id: &[u8; 32],
        amount: u128,
    ) -> Result<u128, AgentRegistryError> {
        let mut agents = self.agents.write();
        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentRegistryError::NotFound(hex::encode(agent_id)))?;

        agent.gas_deposit = agent.gas_deposit.saturating_add(amount);

        debug!(
            agent_id = hex::encode(agent_id),
            new_balance = agent.gas_deposit,
            "Gas deposited for agent"
        );

        Ok(agent.gas_deposit)
    }

    /// Withdraw gas from an agent. Only the owner can withdraw.
    pub fn withdraw_gas(
        &self,
        agent_id: &[u8; 32],
        amount: u128,
        caller: &Address,
    ) -> Result<u128, AgentRegistryError> {
        let mut agents = self.agents.write();
        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentRegistryError::NotFound(hex::encode(agent_id)))?;

        if &agent.owner != caller {
            return Err(AgentRegistryError::NotOwner);
        }

        if amount > agent.gas_deposit {
            return Err(AgentRegistryError::InsufficientDeposit {
                required: amount,
                provided: agent.gas_deposit,
            });
        }

        agent.gas_deposit -= amount;
        Ok(agent.gas_deposit)
    }

    // ── Trigger Config ──────────────────────────────────────────────────

    /// Update an agent's trigger configuration. Only the owner can update.
    pub fn update_trigger_config(
        &self,
        agent_id: &[u8; 32],
        new_config: AgentTriggerConfig,
        caller: &Address,
    ) -> Result<(), AgentRegistryError> {
        let mut agents = self.agents.write();
        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentRegistryError::NotFound(hex::encode(agent_id)))?;

        if &agent.owner != caller {
            return Err(AgentRegistryError::NotOwner);
        }

        if new_config.block_interval == 0 && new_config.enabled {
            return Err(AgentRegistryError::InvalidConfig(
                "block_interval must be > 0 when enabled".to_string(),
            ));
        }

        agent.trigger_config = new_config;
        Ok(())
    }

    // ── Query & Trigger Selection ───────────────────────────────────────

    /// Get agents whose triggers should fire at the given block number.
    ///
    /// Returns agents sorted by gas deposit (highest first) to prioritize
    /// well-funded agents when block gas budget is limited.
    pub fn get_triggered_agents(&self, block_number: u64) -> Vec<AgentAccount> {
        let agents = self.agents.read();

        let mut triggered: Vec<AgentAccount> = agents
            .values()
            .filter(|a| {
                a.trigger_config.enabled
                    && a.trigger_config.block_interval > 0
                    && a.gas_deposit > 0
                    && (block_number - a.last_triggered_block) >= a.trigger_config.block_interval
            })
            .cloned()
            .collect();

        // Sort by gas deposit descending — prioritize well-funded agents
        triggered.sort_by(|a, b| b.gas_deposit.cmp(&a.gas_deposit));

        triggered
    }

    /// Record gas usage after a trigger execution.
    pub fn record_gas_usage(
        &self,
        agent_id: &[u8; 32],
        gas_used: u64,
        block_number: u64,
        gas_price: u128,
    ) -> Result<(), AgentRegistryError> {
        let mut agents = self.agents.write();
        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| AgentRegistryError::NotFound(hex::encode(agent_id)))?;

        let cost = (gas_used as u128).saturating_mul(gas_price);
        agent.gas_deposit = agent.gas_deposit.saturating_sub(cost);
        agent.total_gas_used = agent.total_gas_used.saturating_add(gas_used);
        agent.last_triggered_block = block_number;

        // Auto-disable if out of gas
        if agent.gas_deposit == 0 {
            agent.trigger_config.enabled = false;
            warn!(
                agent_id = hex::encode(agent_id),
                "Agent auto-disabled: gas deposit exhausted"
            );
        }

        Ok(())
    }

    /// Get an agent by ID.
    pub fn get_agent(&self, agent_id: &[u8; 32]) -> Option<AgentAccount> {
        self.agents.read().get(agent_id).cloned()
    }

    /// Total number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.agents.read().len()
    }

    /// List all registered agents.
    pub fn list_agents(&self) -> Vec<AgentAccount> {
        self.agents.read().values().cloned().collect()
    }

    /// Get the block gas budget for triggers.
    pub fn max_trigger_gas_per_block(&self) -> u64 {
        self.config.max_trigger_gas_per_block
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_owner() -> Address {
        Address::from([0xAA; 20])
    }

    fn test_agent_id() -> [u8; 32] {
        [0x01; 32]
    }

    fn make_trigger_config(interval: u64) -> AgentTriggerConfig {
        AgentTriggerConfig {
            block_interval: interval,
            gas_limit_per_trigger: 500_000,
            trigger_calldata: vec![0xDE, 0xAD],
            enabled: true,
        }
    }

    fn register_test_agent(
        registry: &AgentRegistry,
        agent_id: [u8; 32],
        interval: u64,
        deposit: u128,
        block: u64,
    ) {
        registry
            .register_agent(
                agent_id,
                test_owner(),
                Address::from([0xBB; 20]),
                Address::from([0xCC; 20]),
                make_trigger_config(interval),
                deposit,
                block,
            )
            .unwrap();
    }

    #[test]
    fn test_register_and_get() {
        let registry = AgentRegistry::with_defaults();
        register_test_agent(&registry, test_agent_id(), 10, 1_000_000_000_000_000, 100);

        assert_eq!(registry.agent_count(), 1);

        let agent = registry.get_agent(&test_agent_id()).unwrap();
        assert_eq!(agent.owner, test_owner());
        assert_eq!(agent.registered_at, 100);
        assert_eq!(agent.trigger_config.block_interval, 10);
    }

    #[test]
    fn test_duplicate_registration_fails() {
        let registry = AgentRegistry::with_defaults();
        register_test_agent(&registry, test_agent_id(), 10, 1_000_000_000_000_000, 100);

        let result = registry.register_agent(
            test_agent_id(),
            test_owner(),
            Address::from([0xBB; 20]),
            Address::from([0xCC; 20]),
            make_trigger_config(10),
            1_000_000_000_000_000,
            200,
        );

        assert!(matches!(result, Err(AgentRegistryError::AlreadyExists(_))));
    }

    #[test]
    fn test_insufficient_deposit() {
        let registry = AgentRegistry::with_defaults();

        let result = registry.register_agent(
            test_agent_id(),
            test_owner(),
            Address::from([0xBB; 20]),
            Address::from([0xCC; 20]),
            make_trigger_config(10),
            100, // too low
            100,
        );

        assert!(matches!(
            result,
            Err(AgentRegistryError::InsufficientDeposit { .. })
        ));
    }

    #[test]
    fn test_max_agents_limit() {
        let config = AgentRegistryConfig {
            max_agents: 2,
            ..Default::default()
        };
        let registry = AgentRegistry::new(config);

        for i in 0..2u8 {
            let mut id = [0u8; 32];
            id[0] = i;
            register_test_agent(&registry, id, 10, 1_000_000_000_000_000, 100);
        }

        let mut id3 = [0u8; 32];
        id3[0] = 3;
        let result = registry.register_agent(
            id3,
            test_owner(),
            Address::from([0xBB; 20]),
            Address::from([0xCC; 20]),
            make_trigger_config(10),
            1_000_000_000_000_000,
            100,
        );

        assert!(matches!(result, Err(AgentRegistryError::RegistryFull(2))));
    }

    #[test]
    fn test_deregister_by_owner() {
        let registry = AgentRegistry::with_defaults();
        register_test_agent(&registry, test_agent_id(), 10, 1_000_000_000_000_000, 100);

        let removed = registry
            .deregister_agent(&test_agent_id(), &test_owner())
            .unwrap();
        assert_eq!(removed.agent_id, test_agent_id());
        assert_eq!(registry.agent_count(), 0);
    }

    #[test]
    fn test_deregister_by_non_owner_fails() {
        let registry = AgentRegistry::with_defaults();
        register_test_agent(&registry, test_agent_id(), 10, 1_000_000_000_000_000, 100);

        let non_owner = Address::from([0xFF; 20]);
        let result = registry.deregister_agent(&test_agent_id(), &non_owner);
        assert!(matches!(result, Err(AgentRegistryError::NotOwner)));
    }

    #[test]
    fn test_gas_deposit_and_withdraw() {
        let registry = AgentRegistry::with_defaults();
        register_test_agent(&registry, test_agent_id(), 10, 1_000_000_000_000_000, 100);

        // Deposit more gas
        let balance = registry
            .deposit_gas(&test_agent_id(), 500_000_000_000_000)
            .unwrap();
        assert_eq!(balance, 1_500_000_000_000_000);

        // Withdraw gas
        let balance = registry
            .withdraw_gas(&test_agent_id(), 200_000_000_000_000, &test_owner())
            .unwrap();
        assert_eq!(balance, 1_300_000_000_000_000);
    }

    #[test]
    fn test_get_triggered_agents() {
        let registry = AgentRegistry::with_defaults();

        // Agent triggers every 10 blocks
        register_test_agent(&registry, test_agent_id(), 10, 1_000_000_000_000_000, 100);

        // Block 105 — not enough blocks since registration (last was 0, need 10)
        // Actually: last_triggered_block = 0, so 105 - 0 = 105 >= 10 → trigger!
        let triggered = registry.get_triggered_agents(105);
        assert_eq!(triggered.len(), 1);

        // Record execution at block 105
        registry
            .record_gas_usage(&test_agent_id(), 200_000, 105, 1_000_000_000)
            .unwrap();

        // Block 110 — only 5 blocks since last trigger → no trigger
        let triggered = registry.get_triggered_agents(110);
        assert_eq!(triggered.len(), 0);

        // Block 115 — 10 blocks since last trigger → trigger!
        let triggered = registry.get_triggered_agents(115);
        assert_eq!(triggered.len(), 1);
    }

    #[test]
    fn test_auto_disable_on_gas_exhaustion() {
        let registry = AgentRegistry::with_defaults();

        // Agent with minimal deposit
        register_test_agent(
            &registry,
            test_agent_id(),
            10,
            1_000_000_000_000_000, // 0.001 token
            100,
        );

        // Simulate gas consumption that exhausts the deposit
        // 1M gas at 1 gwei = 1e6 * 1e9 = 1e15 = exactly 0.001 token
        registry
            .record_gas_usage(&test_agent_id(), 1_000_000, 110, 1_000_000_000)
            .unwrap();

        // Should be auto-disabled
        let agent = registry.get_agent(&test_agent_id()).unwrap();
        assert!(!agent.trigger_config.enabled);
        assert_eq!(agent.gas_deposit, 0);

        // No longer appears in triggered agents
        let triggered = registry.get_triggered_agents(120);
        assert_eq!(triggered.len(), 0);
    }
}
