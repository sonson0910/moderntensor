//! # Agent Trigger Engine — Block-Level Autonomous Execution
//!
//! Executes AI agent triggers automatically at the start of each block.
//!
//! ## Flow
//! 1. Consensus loop calls `process_block_triggers()` at the start of each block
//! 2. Engine queries `AgentRegistry::get_triggered_agents()` for due agents
//! 3. For each agent, calls their contract via `EvmExecutor::call()`
//! 4. Deducts gas cost from agent's deposit
//! 5. Auto-disables agents that run out of gas
//!
//! ## Safety
//! - Total gas for all triggers capped at `max_trigger_gas_per_block`
//! - Each trigger uses `call()` so failures revert safely
//! - Gas price is block-level, preventing MEV on trigger ordering

use crate::agent_registry::{AgentAccount, AgentRegistry};
use crate::evm_executor::EvmExecutor;
use crate::types::ContractAddress;
use std::sync::Arc;
use tracing::{info, warn, debug, error};

// ──────────────────────────────────────────────────────────────────────────────
// Types
// ──────────────────────────────────────────────────────────────────────────────

/// Result of a single agent trigger execution.
#[derive(Debug, Clone)]
pub struct TriggerResult {
    /// Agent ID that was triggered.
    pub agent_id: [u8; 32],
    /// Whether the trigger executed successfully.
    pub success: bool,
    /// Gas consumed by the trigger.
    pub gas_used: u64,
    /// Return data from the contract call (if any).
    pub return_data: Vec<u8>,
    /// Error message (if the trigger failed).
    pub error: Option<String>,
}

/// Summary of all trigger processing for a block.
#[derive(Debug, Default)]
pub struct BlockTriggerOutcome {
    /// Number of agents triggered successfully.
    pub successful: usize,
    /// Number of agents whose triggers failed/reverted.
    pub failed: usize,
    /// Number of agents skipped due to block gas budget.
    pub skipped: usize,
    /// Total gas consumed by all triggers in this block.
    pub total_gas_used: u64,
    /// Individual trigger results.
    pub results: Vec<TriggerResult>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Engine
// ──────────────────────────────────────────────────────────────────────────────

/// Block Trigger Engine — autonomous AI agent execution per block.
///
/// Integrated into the consensus loop to execute agent triggers
/// before processing user transactions.
pub struct AgentTriggerEngine {
    /// Reference to the agent registry.
    registry: Arc<AgentRegistry>,
    /// Reference to the EVM executor.
    evm: Arc<EvmExecutor>,
}

impl AgentTriggerEngine {
    /// Create a new trigger engine.
    pub fn new(registry: Arc<AgentRegistry>, evm: Arc<EvmExecutor>) -> Self {
        Self { registry, evm }
    }

    /// Process all agent triggers for the given block.
    ///
    /// Called at the START of each block, before user transactions.
    /// Respects the block gas budget — skips remaining agents if exhausted.
    ///
    /// # Arguments
    /// * `block_number` - current block height
    /// * `timestamp` - block timestamp (unix seconds)
    /// * `gas_price` - block-level gas price (wei per gas unit)
    pub fn process_block_triggers(
        &self,
        block_number: u64,
        timestamp: u64,
        gas_price: u128,
    ) -> BlockTriggerOutcome {
        let mut outcome = BlockTriggerOutcome::default();
        let max_gas = self.registry.max_trigger_gas_per_block();

        // Get agents due for trigger execution (sorted by deposit, highest first)
        let triggered_agents = self.registry.get_triggered_agents(block_number);

        if triggered_agents.is_empty() {
            return outcome;
        }

        debug!(
            block = block_number,
            agents_due = triggered_agents.len(),
            gas_budget = max_gas,
            "Processing agent triggers"
        );

        for agent in &triggered_agents {
            // Check remaining gas budget
            let remaining_budget = max_gas.saturating_sub(outcome.total_gas_used);
            if remaining_budget < agent.trigger_config.gas_limit_per_trigger {
                outcome.skipped += 1;
                debug!(
                    agent_id = hex::encode(agent.agent_id),
                    remaining = remaining_budget,
                    required = agent.trigger_config.gas_limit_per_trigger,
                    "Skipping agent — insufficient block gas budget"
                );
                continue;
            }

            // Execute the trigger
            let result = self.execute_trigger(agent, block_number, timestamp, gas_price);

            // Record gas usage in registry
            if let Err(e) = self.registry.record_gas_usage(
                &agent.agent_id,
                result.gas_used,
                block_number,
                gas_price,
            ) {
                error!(
                    agent_id = hex::encode(agent.agent_id),
                    error = %e,
                    "Failed to record gas usage"
                );
            }

            outcome.total_gas_used += result.gas_used;

            if result.success {
                outcome.successful += 1;
            } else {
                outcome.failed += 1;
            }

            outcome.results.push(result);
        }

        // Log remaining skipped agents
        let remaining_agents = triggered_agents.len()
            - outcome.successful
            - outcome.failed
            - outcome.skipped;
        if remaining_agents > 0 {
            outcome.skipped += remaining_agents;
        }

        info!(
            block = block_number,
            successful = outcome.successful,
            failed = outcome.failed,
            skipped = outcome.skipped,
            total_gas = outcome.total_gas_used,
            "Block triggers processed"
        );

        outcome
    }

    /// Execute a single agent's trigger by calling its contract.
    fn execute_trigger(
        &self,
        agent: &AgentAccount,
        block_number: u64,
        timestamp: u64,
        gas_price: u128,
    ) -> TriggerResult {
        // The agent's wallet is the caller (msg.sender in the contract)
        let caller = agent.wallet_address;
        let contract_addr = ContractAddress::from(*agent.contract_address.as_bytes());

        // Get the agent's contract code from the EVM state
        // In a real scenario, the code would already be deployed and stored
        // For now, we attempt the call — the EVM will handle missing code
        let contract_code = Vec::new(); // EVM resolves from its own state

        let result = self.evm.call(
            caller,
            contract_addr,
            contract_code,
            agent.trigger_config.trigger_calldata.clone(),
            0,       // no value transfer for triggers
            agent.trigger_config.gas_limit_per_trigger,
            block_number,
            timestamp,
            gas_price,
        );

        match result {
            Ok((return_data, gas_used, _logs)) => {
                debug!(
                    agent_id = hex::encode(agent.agent_id),
                    gas_used = gas_used,
                    "Agent trigger executed successfully"
                );
                TriggerResult {
                    agent_id: agent.agent_id,
                    success: true,
                    gas_used,
                    return_data,
                    error: None,
                }
            }
            Err(e) => {
                warn!(
                    agent_id = hex::encode(agent.agent_id),
                    error = %e,
                    "Agent trigger execution failed"
                );
                TriggerResult {
                    agent_id: agent.agent_id,
                    success: false,
                    gas_used: agent.trigger_config.gas_limit_per_trigger, // charge full limit on failure
                    return_data: Vec::new(),
                    error: Some(e.to_string()),
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_registry::{AgentRegistryConfig, AgentTriggerConfig};
    use luxtensor_core::types::Address;

    fn setup_engine() -> (Arc<AgentRegistry>, Arc<EvmExecutor>, AgentTriggerEngine) {
        let config = AgentRegistryConfig {
            max_trigger_gas_per_block: 2_000_000,
            max_agents: 100,
            min_gas_deposit: 1_000_000_000_000_000,
        };
        let registry = Arc::new(AgentRegistry::new(config));
        let evm = Arc::new(EvmExecutor::default());
        let engine = AgentTriggerEngine::new(registry.clone(), evm.clone());
        (registry, evm, engine)
    }

    fn register_agent(registry: &AgentRegistry, id_byte: u8, interval: u64) {
        let mut agent_id = [0u8; 32];
        agent_id[0] = id_byte;

        registry
            .register_agent(
                agent_id,
                Address::from([0xAA; 20]),
                Address::from([0xBB; 20]),
                Address::from([0xCC; 20]),
                AgentTriggerConfig {
                    block_interval: interval,
                    gas_limit_per_trigger: 500_000,
                    trigger_calldata: vec![0xDE, 0xAD],
                    enabled: true,
                },
                10_000_000_000_000_000_000, // 10 tokens — enough for many triggers
                0,
            )
            .unwrap();
    }

    #[test]
    fn test_no_agents_no_triggers() {
        let (_registry, _evm, engine) = setup_engine();
        let outcome = engine.process_block_triggers(100, 1700000000, 1_000_000_000);
        assert_eq!(outcome.successful, 0);
        assert_eq!(outcome.failed, 0);
        assert_eq!(outcome.total_gas_used, 0);
    }

    #[test]
    fn test_trigger_fires_at_interval() {
        let (registry, _evm, engine) = setup_engine();

        // Register agent that triggers every 10 blocks
        register_agent(&registry, 1, 10);

        // Block 10 — should trigger (10 - 0 >= 10)
        let outcome = engine.process_block_triggers(10, 1700000000, 1_000_000_000);
        // Note: call will likely fail since no actual contract deployed,
        // but the mechanism WORKS — it attempts the call.
        assert_eq!(outcome.results.len(), 1);
    }

    #[test]
    fn test_gas_budget_limits_triggers() {
        let config = AgentRegistryConfig {
            max_trigger_gas_per_block: 600_000, // Only enough for 1 agent (500k gas each)
            max_agents: 100,
            min_gas_deposit: 1_000_000_000_000_000,
        };
        let registry = Arc::new(AgentRegistry::new(config));
        let evm = Arc::new(EvmExecutor::default());
        let engine = AgentTriggerEngine::new(registry.clone(), evm);

        // Register 3 agents
        register_agent(&registry, 1, 5);
        register_agent(&registry, 2, 5);
        register_agent(&registry, 3, 5);

        // Process — should only execute 1, skip 2
        let outcome = engine.process_block_triggers(10, 1700000000, 1_000_000_000);
        assert_eq!(outcome.results.len(), 1);
        assert!(outcome.skipped >= 2);
    }
}
