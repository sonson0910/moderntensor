// LuxTensor smart contracts module
// Provides infrastructure for smart contract deployment and execution

pub mod error;
pub mod executor;
pub mod state;
pub mod types;

pub use error::*;
pub use executor::{
    ContractExecutor, ExecutionContext, ExecutionResult, Log, ContractStats,
    DEFAULT_GAS_LIMIT, MAX_GAS_LIMIT,
};
pub use state::ContractState;
pub use types::{
    ContractAddress, ContractCode, ContractABI, FunctionSignature, EventSignature, ABIType,
};
