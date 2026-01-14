// LuxTensor smart contracts module
// Provides infrastructure for smart contract deployment and execution with EVM

pub mod error;
pub mod executor;
pub mod state;
pub mod types;
pub mod evm_executor;
pub mod revm_integration;
pub mod account_abstraction;

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
pub use account_abstraction::{
    UserOperation, EntryPoint, UserOperationReceipt, PaymasterInfo,
    SimulationResult, GasEstimate, AccountAbstractionError,
};

