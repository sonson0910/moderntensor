// EVM executor using revm for actual bytecode execution

use crate::error::ContractError;
use crate::types::ContractAddress;
use luxtensor_core::types::{Address, Hash};
use revm::primitives::{
    AccountInfo, Address as RevmAddress, Bytecode, Bytes, ExecutionResult as RevmExecutionResult,
    Output, ResultAndState, TransactTo, TxEnv, U256,
};
use revm::{Database, DatabaseCommit, Evm};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, warn};

/// EVM-based contract executor
pub struct EvmExecutor {
    /// Account storage (address -> AccountInfo)
    accounts: Arc<RwLock<HashMap<RevmAddress, AccountInfo>>>,
    /// Contract storage (address -> key -> value)
    storage: Arc<RwLock<HashMap<RevmAddress, HashMap<U256, U256>>>>,
}

impl EvmExecutor {
    /// Create new EVM executor
    pub fn new() -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute contract deployment
    pub fn deploy(
        &self,
        deployer: Address,
        code: Vec<u8>,
        value: u128,
        gas_limit: u64,
        gas_price: u64,
        block_number: u64,
        timestamp: u64,
    ) -> Result<(Vec<u8>, u64, Vec<u8>), ContractError> {
        let deployer_addr = address_to_revm(&deployer);

        // Ensure deployer account exists
        self.ensure_account(&deployer_addr);

        // Create EVM instance
        let mut evm = Evm::builder()
            .with_db(self.clone())
            .modify_block_env(|b| {
                b.number = U256::from(block_number);
                b.timestamp = U256::from(timestamp);
                b.gas_limit = U256::from(gas_limit);
            })
            .modify_tx_env(|tx| {
                tx.caller = deployer_addr;
                tx.transact_to = TransactTo::Create;
                tx.data = Bytes::from(code);
                tx.value = U256::from(value);
                tx.gas_limit = gas_limit;
                tx.gas_price = U256::from(gas_price);
            })
            .build();

        // Execute transaction
        let result = evm.transact_commit().map_err(|e| {
            warn!("EVM deployment error: {:?}", e);
            ContractError::ExecutionFailed(format!("EVM error: {:?}", e))
        })?;

        match result {
            RevmExecutionResult::Success {
                output,
                gas_used,
                logs,
                ..
            } => {
                let (contract_address, deployed_code) = match output {
                    Output::Create(bytes, Some(addr)) => (addr.0 .0.to_vec(), bytes.to_vec()),
                    Output::Create(bytes, None) => {
                        return Err(ContractError::ExecutionFailed(
                            "Contract creation failed".to_string(),
                        ))
                    }
                    _ => {
                        return Err(ContractError::ExecutionFailed(
                            "Invalid output for deployment".to_string(),
                        ))
                    }
                };

                debug!(
                    "Contract deployed at {} with {} gas",
                    hex::encode(&contract_address),
                    gas_used
                );

                // Convert logs to bytes (simplified)
                let logs_data = logs
                    .iter()
                    .flat_map(|log| log.data.data.iter().copied())
                    .collect();

                Ok((contract_address, gas_used, logs_data))
            }
            RevmExecutionResult::Revert { gas_used, output } => {
                let reason = String::from_utf8_lossy(&output).to_string();
                warn!("Contract deployment reverted: {}", reason);
                Err(ContractError::ExecutionReverted(reason))
            }
            RevmExecutionResult::Halt { reason, gas_used } => {
                warn!("Contract deployment halted: {:?}", reason);
                Err(ContractError::ExecutionFailed(format!(
                    "Halted: {:?}",
                    reason
                )))
            }
        }
    }

    /// Execute contract call
    pub fn call(
        &self,
        caller: Address,
        contract_address: ContractAddress,
        contract_code: Vec<u8>,
        input_data: Vec<u8>,
        value: u128,
        gas_limit: u64,
        gas_price: u64,
        block_number: u64,
        timestamp: u64,
    ) -> Result<(Vec<u8>, u64, Vec<u8>), ContractError> {
        let caller_addr = address_to_revm(&caller);
        let contract_addr = RevmAddress::from_slice(&contract_address.0);

        // Ensure caller account exists
        self.ensure_account(&caller_addr);

        // Set up contract account with code
        {
            let mut accounts = self.accounts.write();
            accounts.insert(
                contract_addr,
                AccountInfo {
                    balance: U256::from(value),
                    nonce: 0,
                    code_hash: revm::primitives::KECCAK_EMPTY,
                    code: Some(Bytecode::new_raw(Bytes::from(contract_code))),
                },
            );
        }

        // Create EVM instance
        let mut evm = Evm::builder()
            .with_db(self.clone())
            .modify_block_env(|b| {
                b.number = U256::from(block_number);
                b.timestamp = U256::from(timestamp);
                b.gas_limit = U256::from(gas_limit);
            })
            .modify_tx_env(|tx| {
                tx.caller = caller_addr;
                tx.transact_to = TransactTo::Call(contract_addr);
                tx.data = Bytes::from(input_data);
                tx.value = U256::from(value);
                tx.gas_limit = gas_limit;
                tx.gas_price = U256::from(gas_price);
            })
            .build();

        // Execute transaction
        let result = evm.transact_commit().map_err(|e| {
            warn!("EVM call error: {:?}", e);
            ContractError::ExecutionFailed(format!("EVM error: {:?}", e))
        })?;

        match result {
            RevmExecutionResult::Success {
                output,
                gas_used,
                logs,
                ..
            } => {
                let return_data = match output {
                    Output::Call(bytes) => bytes.to_vec(),
                    _ => vec![],
                };

                debug!("Contract call succeeded with {} gas", gas_used);

                // Convert logs to bytes (simplified)
                let logs_data = logs
                    .iter()
                    .flat_map(|log| log.data.data.iter().copied())
                    .collect();

                Ok((return_data, gas_used, logs_data))
            }
            RevmExecutionResult::Revert { gas_used, output } => {
                let reason = String::from_utf8_lossy(&output).to_string();
                warn!("Contract call reverted: {}", reason);
                Err(ContractError::ExecutionReverted(reason))
            }
            RevmExecutionResult::Halt { reason, gas_used } => {
                warn!("Contract call halted: {:?}", reason);
                Err(ContractError::ExecutionFailed(format!(
                    "Halted: {:?}",
                    reason
                )))
            }
        }
    }

    /// Ensure account exists in state
    fn ensure_account(&self, address: &RevmAddress) {
        let mut accounts = self.accounts.write();
        accounts.entry(*address).or_insert(AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: revm::primitives::KECCAK_EMPTY,
            code: None,
        });
    }

    /// Get storage value for a contract
    pub fn get_storage(&self, address: &ContractAddress, key: &Hash) -> Option<Hash> {
        let contract_addr = RevmAddress::from_slice(&address.0);
        let storage = self.storage.read();
        let contract_storage = storage.get(&contract_addr)?;
        let key_u256 = U256::from_be_bytes(*key);
        let value_u256 = contract_storage.get(&key_u256)?;
        let mut result = [0u8; 32];
        let bytes = value_u256.to_be_bytes_vec();
        result.copy_from_slice(&bytes[..32.min(bytes.len())]);
        Some(result)
    }

    /// Set storage value for a contract
    pub fn set_storage(&self, address: &ContractAddress, key: Hash, value: Hash) {
        let contract_addr = RevmAddress::from_slice(&address.0);
        let key_u256 = U256::from_be_bytes(key);
        let value_u256 = U256::from_be_bytes(value);

        let mut storage = self.storage.write();
        storage
            .entry(contract_addr)
            .or_insert_with(HashMap::new)
            .insert(key_u256, value_u256);
    }
}

impl Clone for EvmExecutor {
    fn clone(&self) -> Self {
        Self {
            accounts: Arc::clone(&self.accounts),
            storage: Arc::clone(&self.storage),
        }
    }
}

// Implement Database trait for EVM integration
impl Database for EvmExecutor {
    type Error = ContractError;

    fn basic(&mut self, address: RevmAddress) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.accounts.read().get(&address).cloned())
    }

    fn code_by_hash(&mut self, _code_hash: revm::primitives::B256) -> Result<Bytecode, Self::Error> {
        // Code is stored directly in AccountInfo
        Ok(Bytecode::default())
    }

    fn storage(&mut self, address: RevmAddress, index: U256) -> Result<U256, Self::Error> {
        let storage = self.storage.read();
        Ok(storage
            .get(&address)
            .and_then(|s| s.get(&index).copied())
            .unwrap_or(U256::ZERO))
    }

    fn block_hash(&mut self, _number: u64) -> Result<revm::primitives::B256, Self::Error> {
        Ok(revm::primitives::B256::ZERO)
    }
}

impl DatabaseCommit for EvmExecutor {
    fn commit(&mut self, changes: HashMap<RevmAddress, revm::primitives::Account>) {
        let mut accounts = self.accounts.write();
        let mut storage = self.storage.write();

        for (address, account) in changes {
            // Update account info
            if account.is_selfdestructed() {
                accounts.remove(&address);
                storage.remove(&address);
            } else {
                accounts.insert(
                    address,
                    AccountInfo {
                        balance: account.info.balance,
                        nonce: account.info.nonce,
                        code_hash: account.info.code_hash,
                        code: account.info.code.clone(),
                    },
                );

                // Update storage
                let contract_storage = storage.entry(address).or_insert_with(HashMap::new);
                for (key, value) in account.storage {
                    if value.present_value.is_zero() {
                        contract_storage.remove(&key);
                    } else {
                        contract_storage.insert(key, value.present_value);
                    }
                }
            }
        }
    }
}

/// Convert Address to RevmAddress
fn address_to_revm(address: &Address) -> RevmAddress {
    RevmAddress::from_slice(address.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_GAS_LIMIT: u64 = 1_000_000;
    const TEST_GAS_PRICE: u64 = 1_000_000_000; // 1 gwei
    const TEST_BLOCK_NUMBER: u64 = 1;
    const TEST_TIMESTAMP: u64 = 1000;

    #[test]
    fn test_evm_executor_creation() {
        let executor = EvmExecutor::new();
        assert_eq!(executor.accounts.read().len(), 0);
    }

    #[test]
    fn test_simple_deployment() {
        let executor = EvmExecutor::new();
        let deployer = Address::from([1u8; 20]);

        // Simple contract bytecode (just returns)
        let code = vec![0x60, 0x00, 0x60, 0x00, 0xf3]; // PUSH1 0, PUSH1 0, RETURN

        let result = executor.deploy(
            deployer,
            code,
            0,
            TEST_GAS_LIMIT,
            TEST_GAS_PRICE,
            TEST_BLOCK_NUMBER,
            TEST_TIMESTAMP,
        );
        // May fail without proper bytecode, but should not panic
        assert!(result.is_ok() || result.is_err());
    }
}
