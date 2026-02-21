// LuxTensor smart contracts module
// Provides infrastructure for smart contract deployment and execution with EVM

pub mod error;
pub mod executor;
pub mod state;
pub mod types;
pub mod evm_executor;
pub mod revm_integration;
pub mod account_abstraction;
pub mod ai_precompiles;
pub mod agent_registry;
pub mod agent_trigger;


pub use error::*;
pub use executor::{
    ContractExecutor, ExecutionContext, ExecutionResult, Log, ContractStats,
    DEFAULT_GAS_LIMIT, MAX_GAS_LIMIT,
};
pub use state::ContractState;
pub use types::{
    ContractAddress, ContractCode, ContractABI, FunctionSignature, EventSignature, ABIType,
};
pub use evm_executor::EvmExecutor;
pub use evm_executor::{EvmLog, PersistentEvmExecutor, EvmAccountRecord, EvmStateStore};
pub use account_abstraction::{
    UserOperation, EntryPoint, UserOperationReceipt, PaymasterInfo,
    SimulationResult, GasEstimate, AccountAbstractionError,
};
pub use ai_precompiles::{
    AIPrecompileState, AIRequestEntry, RequestStatus,
    TrainingJob, TrainingStatus, gas_costs,
};
pub use revm_integration::precompiles;
pub use revm_integration::{execute_ai_precompile, is_luxtensor_precompile};
pub use agent_registry::{AgentRegistry, AgentAccount, AgentTriggerConfig, AgentRegistryConfig, AgentRegistryError};
pub use agent_trigger::{AgentTriggerEngine, TriggerResult, BlockTriggerOutcome};


