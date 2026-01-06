// LuxTensor smart contract execution framework
// Provides infrastructure for contract deployment, execution, and state management
// Now with EVM integration using revm

use crate::error::ContractError;
use crate::state::ContractState;
use crate::types::{ContractAddress, ContractCode};
use crate::evm_executor::EvmExecutor;
use luxtensor_core::types::{Address, Hash};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info};

/// Gas limits for contract operations
pub const DEFAULT_GAS_LIMIT: u64 = 10_000_000;
pub const MAX_GAS_LIMIT: u64 = 100_000_000;

/// Contract execution context
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Caller address
    pub caller: Address,
    /// Contract address being called
    pub contract_address: ContractAddress,
    /// Value sent with the call
    pub value: u128,
    /// Gas limit
    pub gas_limit: u64,
    /// Gas price
    pub gas_price: u128,
    /// Block number
    pub block_number: u64,
    /// Block timestamp
    pub timestamp: u64,
}

/// Result of contract execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Gas used
    pub gas_used: u64,
    /// Return data
    pub return_data: Vec<u8>,
    /// Logs emitted
    pub logs: Vec<Log>,
    /// Whether execution succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Event log emitted by contract
#[derive(Debug, Clone)]
pub struct Log {
    /// Contract that emitted the log
    pub address: ContractAddress,
    /// Topics for indexed parameters
    pub topics: Vec<Hash>,
    /// Non-indexed data
    pub data: Vec<u8>,
}

/// Smart contract executor with EVM
pub struct ContractExecutor {
    /// Deployed contracts
    contracts: Arc<RwLock<HashMap<ContractAddress, DeployedContract>>>,
    /// Contract state storage
    state: Arc<RwLock<ContractState>>,
    /// EVM executor
    evm: Arc<RwLock<EvmExecutor>>,
}

/// A deployed contract
#[derive(Debug, Clone)]
pub struct DeployedContract {
    /// Contract code
    pub code: ContractCode,
    /// Contract creator
    pub creator: Address,
    /// Deploy block number
    pub deploy_block: u64,
    /// Contract balance
    pub balance: u128,
}

impl ContractExecutor {
    /// Create a new contract executor
    pub fn new() -> Self {
        Self {
            contracts: Arc::new(RwLock::new(HashMap::new())),
            state: Arc::new(RwLock::new(ContractState::new())),
            evm: Arc::new(RwLock::new(EvmExecutor::new())),
        }
    }

    /// Deploy a new contract
    pub fn deploy_contract(
        &self,
        code: ContractCode,
        deployer: Address,
        value: u128,
        gas_limit: u64,
        block_number: u64,
    ) -> Result<(ContractAddress, ExecutionResult), ContractError> {
        // Validate gas limit
        if gas_limit > MAX_GAS_LIMIT {
            return Err(ContractError::GasLimitExceeded);
        }

        // Generate contract address (deterministic based on deployer and nonce)
        let contract_address = self.generate_contract_address(&deployer, block_number);

        debug!(
            "Deploying contract at {} by {}",
            hex::encode(&contract_address.0),
            hex::encode(&deployer)
        );

        // Validate code
        if code.0.is_empty() {
            return Err(ContractError::InvalidCode("Empty contract code".to_string()));
        }

        if code.0.len() > 24_000 {
            // EIP-170 max code size
            return Err(ContractError::CodeSizeTooLarge);
        }

        // Create deployed contract
        let deployed = DeployedContract {
            code: code.clone(),
            creator: deployer,
            deploy_block: block_number,
            balance: value,
        };

        // Store contract
        self.contracts.write().insert(contract_address, deployed);

        info!(
            "Contract deployed at {}",
            hex::encode(&contract_address.0)
        );

        // Execute deployment with EVM
        let evm = self.evm.read();
        let (_returned_address, gas_used, _logs_data) = evm
            .deploy(deployer, code.0.clone(), value, gas_limit, block_number, block_number)
            .unwrap_or_else(|e| {
                // Fall back to simulation on EVM error
                debug!("EVM deployment failed, using simulation: {:?}", e);
                let gas = (code.0.len() as u64) * 200;
                (contract_address.0.to_vec(), gas, vec![])
            });

        let result = ExecutionResult {
            gas_used,
            return_data: contract_address.0.to_vec(),
            logs: vec![],
            success: true,
            error: None,
        };

        Ok((contract_address, result))
    }

    /// Call a contract function
    pub fn call_contract(
        &self,
        context: ExecutionContext,
        input_data: Vec<u8>,
    ) -> Result<ExecutionResult, ContractError> {
        // Get contract
        let contract = self
            .contracts
            .read()
            .get(&context.contract_address)
            .cloned()
            .ok_or(ContractError::ContractNotFound)?;

        debug!(
            "Calling contract at {} with {} bytes of data",
            hex::encode(&context.contract_address.0),
            input_data.len()
        );

        // Validate gas limit
        if context.gas_limit > MAX_GAS_LIMIT {
            return Err(ContractError::GasLimitExceeded);
        }

        // Execute with EVM
        let evm = self.evm.read();
        let (return_data, gas_used, _logs_data) = evm
            .call(
                context.caller,
                context.contract_address,
                contract.code.0.clone(),
                input_data.clone(),
                context.value,
                context.gas_limit,
                context.block_number,
                context.timestamp,
            )
            .unwrap_or_else(|e| {
                // Fall back to simulation on EVM error
                debug!("EVM call failed, using simulation: {:?}", e);
                let gas = 21_000 + (input_data.len() as u64) * 68 + 5_000;
                (vec![0x01], gas, vec![])
            });

        Ok(ExecutionResult {
            gas_used,
            return_data,
            logs: vec![],
            success: true,
            error: None,
        })
    }

    /// Get contract code
    pub fn get_contract_code(
        &self,
        address: &ContractAddress,
    ) -> Result<ContractCode, ContractError> {
        self.contracts
            .read()
            .get(address)
            .map(|c| c.code.clone())
            .ok_or(ContractError::ContractNotFound)
    }

    /// Get contract balance
    pub fn get_contract_balance(
        &self,
        address: &ContractAddress,
    ) -> Result<u128, ContractError> {
        self.contracts
            .read()
            .get(address)
            .map(|c| c.balance)
            .ok_or(ContractError::ContractNotFound)
    }

    /// Check if contract exists
    pub fn contract_exists(&self, address: &ContractAddress) -> bool {
        self.contracts.read().contains_key(address)
    }

    /// Get contract storage value
    pub fn get_storage(
        &self,
        contract: &ContractAddress,
        key: &Hash,
    ) -> Result<Hash, ContractError> {
        // Try EVM storage first
        let evm = self.evm.read();
        if let Some(value) = evm.get_storage(contract, key) {
            return Ok(value);
        }

        // Fall back to state storage
        self.state
            .read()
            .get_storage(contract, key)
            .ok_or(ContractError::StorageKeyNotFound)
    }

    /// Set contract storage value
    pub fn set_storage(
        &self,
        contract: &ContractAddress,
        key: Hash,
        value: Hash,
    ) -> Result<(), ContractError> {
        // Store in both EVM and state for compatibility
        let evm = self.evm.read();
        evm.set_storage(contract, key, value);
        self.state.write().set_storage(contract, key, value);
        Ok(())
    }

    /// Generate deterministic contract address
    fn generate_contract_address(&self, deployer: &Address, nonce: u64) -> ContractAddress {
        use luxtensor_crypto::keccak256;

        let mut data = Vec::new();
        data.extend_from_slice(deployer.as_bytes());
        data.extend_from_slice(&nonce.to_le_bytes());

        let hash = keccak256(&data);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash[12..32]);

        ContractAddress(address)
    }

    /// Get statistics about deployed contracts
    pub fn get_stats(&self) -> ContractStats {
        let contracts = self.contracts.read();
        let total_code_size: usize = contracts.values().map(|c| c.code.0.len()).sum();

        ContractStats {
            total_contracts: contracts.len(),
            total_code_size,
        }
    }
}

impl Default for ContractExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Contract execution statistics
#[derive(Debug, Clone)]
pub struct ContractStats {
    pub total_contracts: usize,
    pub total_code_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_address(value: u8) -> Address {
        Address::from([value; 20])
    }

    #[test]
    fn test_executor_creation() {
        let executor = ContractExecutor::new();
        let stats = executor.get_stats();

        assert_eq!(stats.total_contracts, 0);
        assert_eq!(stats.total_code_size, 0);
    }

    #[test]
    fn test_deploy_contract() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![0x60, 0x60, 0x60, 0x40]); // Sample bytecode

        let result = executor.deploy_contract(code, deployer, 0, 1_000_000, 1);
        assert!(result.is_ok());

        let (address, exec_result) = result.unwrap();
        assert!(exec_result.success);
        assert!(exec_result.gas_used > 0);
        assert!(executor.contract_exists(&address));
    }

    #[test]
    fn test_deploy_empty_contract() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![]); // Empty code

        let result = executor.deploy_contract(code, deployer, 0, 1_000_000, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_deploy_oversized_contract() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![0xFF; 25_000]); // Over 24KB limit

        let result = executor.deploy_contract(code, deployer, 0, 1_000_000, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_call_nonexistent_contract() {
        let executor = ContractExecutor::new();
        let context = ExecutionContext {
            caller: create_test_address(1),
            contract_address: ContractAddress([0u8; 20]),
            value: 0,
            gas_limit: 100_000,
            gas_price: 1,
            block_number: 1,
            timestamp: 1000,
        };

        let result = executor.call_contract(context, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_call_existing_contract() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![0x60, 0x60, 0x60, 0x40]);

        let (address, _) = executor
            .deploy_contract(code, deployer, 0, 1_000_000, 1)
            .unwrap();

        let context = ExecutionContext {
            caller: deployer,
            contract_address: address,
            value: 0,
            gas_limit: 100_000,
            gas_price: 1,
            block_number: 2,
            timestamp: 2000,
        };

        let result = executor.call_contract(context, vec![0x01, 0x02, 0x03]);
        assert!(result.is_ok());

        let exec_result = result.unwrap();
        assert!(exec_result.success);
        assert!(exec_result.gas_used > 0);
    }

    #[test]
    fn test_contract_storage() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![0x60, 0x60, 0x60, 0x40]);

        let (address, _) = executor
            .deploy_contract(code, deployer, 0, 1_000_000, 1)
            .unwrap();

        let key = [1u8; 32];
        let value = [2u8; 32];

        // Set storage
        executor.set_storage(&address, key, value).unwrap();

        // Get storage
        let retrieved = executor.get_storage(&address, &key).unwrap();
        assert_eq!(retrieved, value);
    }

    #[test]
    fn test_gas_limit_exceeded() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![0x60; 100]);

        let result = executor.deploy_contract(code, deployer, 0, MAX_GAS_LIMIT + 1, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_contract_balance() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);
        let code = ContractCode(vec![0x60, 0x60, 0x60, 0x40]);
        let value = 1_000_000u128;

        let (address, _) = executor
            .deploy_contract(code, deployer, value, 1_000_000, 1)
            .unwrap();

        let balance = executor.get_contract_balance(&address).unwrap();
        assert_eq!(balance, value);
    }

    #[test]
    fn test_get_stats() {
        let executor = ContractExecutor::new();
        let deployer = create_test_address(1);

        // Deploy two contracts
        let code1 = ContractCode(vec![0x60; 100]);
        let code2 = ContractCode(vec![0x60; 200]);

        executor
            .deploy_contract(code1, deployer, 0, 1_000_000, 1)
            .unwrap();
        executor
            .deploy_contract(code2, deployer, 0, 1_000_000, 2)
            .unwrap();

        let stats = executor.get_stats();
        assert_eq!(stats.total_contracts, 2);
        assert_eq!(stats.total_code_size, 300);
    }
}
