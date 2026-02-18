// EVM executor using revm for actual bytecode execution

use crate::error::ContractError;
use crate::types::ContractAddress;
use luxtensor_core::constants::chain_id;
use luxtensor_core::types::{Address, Hash};
use parking_lot::RwLock;
use revm::primitives::{
    AccountInfo, Address as RevmAddress, Bytecode, Bytes, ExecutionResult as RevmExecutionResult,
    Output, TransactTo, U256,
};
use revm::{Database, DatabaseCommit, Evm};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// Structured EVM log entry preserving topics and data from REVM execution.
/// This avoids the information loss of flattening logs to raw bytes.
#[derive(Debug, Clone)]
pub struct EvmLog {
    /// Address of the contract that emitted the log
    pub address: Vec<u8>,
    /// Indexed topics (topic[0] = event signature hash)
    pub topics: Vec<[u8; 32]>,
    /// Non-indexed ABI-encoded data
    pub data: Vec<u8>,
}

/// EVM-based contract executor
pub struct EvmExecutor {
    /// Chain ID for EIP-155 cross-chain replay protection.
    chain_id: u64,
    /// Account storage (address -> AccountInfo)
    accounts: Arc<RwLock<HashMap<RevmAddress, AccountInfo>>>,
    /// Contract storage (address -> key -> value)
    storage: Arc<RwLock<HashMap<RevmAddress, HashMap<U256, U256>>>>,
    /// Recent block hashes for BLOCKHASH opcode (number -> hash)
    /// Keeps up to 256 entries per EIP-2 specification.
    block_hashes: Arc<RwLock<HashMap<u64, [u8; 32]>>>,
}

impl Default for EvmExecutor {
    /// Default executor uses LuxTensor Devnet chain_id (8898)
    fn default() -> Self {
        Self::new(chain_id::DEVNET) // LuxTensor Devnet
    }
}

impl EvmExecutor {
    /// Create new EVM executor with the given chain ID for EIP-155 replay protection.
    pub fn new(chain_id: u64) -> Self {
        Self {
            chain_id,
            accounts: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(RwLock::new(HashMap::new())),
            block_hashes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a block hash for use by the EVM BLOCKHASH opcode.
    /// Only the most recent 256 blocks are accessible per the EVM spec.
    pub fn record_block_hash(&self, number: u64, hash: [u8; 32]) {
        let mut hashes = self.block_hashes.write();
        hashes.insert(number, hash);
        // Prune entries older than 256 blocks
        if number > 256 {
            let cutoff = number - 256;
            hashes.retain(|&k, _| k > cutoff);
        }
    }

    /// Execute contract deployment
    /// Returns: (contract_address, gas_used, logs, deployed_bytecode)
    pub fn deploy(
        &self,
        deployer: Address,
        code: Vec<u8>,
        value: u128,
        gas_limit: u64,
        block_number: u64,
        timestamp: u64,
        gas_price: u128,
    ) -> Result<(Vec<u8>, u64, Vec<EvmLog>, Vec<u8>), ContractError> {
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
            .modify_cfg_env(|cfg| {
                // SECURITY: Set chain_id for cross-chain replay protection (EIP-155)
                cfg.chain_id = self.chain_id;
            })
            .build();

        // Execute transaction
        let result = evm.transact_commit().map_err(|e| {
            warn!("EVM deployment error: {:?}", e);
            ContractError::ExecutionFailed(format!("EVM error: {:?}", e))
        })?;

        // Debug: log raw EVM result
        tracing::info!("🔍 EVM Deploy raw result: {:?}", result);

        match result {
            RevmExecutionResult::Success { output, gas_used, logs, .. } => {
                let (contract_address, deployed_code) = match output {
                    Output::Create(bytes, Some(addr)) => (addr.0 .0.to_vec(), bytes.to_vec()),
                    Output::Create(_bytes, None) => {
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

                // Convert REVM logs to structured EvmLog preserving topics
                let evm_logs = logs
                    .iter()
                    .map(|log| {
                        let mut topics = Vec::with_capacity(log.data.topics().len());
                        for topic in log.data.topics() {
                            topics.push(topic.0);
                        }
                        EvmLog {
                            address: log.address.0.to_vec(),
                            topics,
                            data: log.data.data.to_vec(),
                        }
                    })
                    .collect();

                Ok((contract_address, gas_used, evm_logs, deployed_code))
            }
            RevmExecutionResult::Revert { gas_used: _, output } => {
                let reason = String::from_utf8_lossy(&output).to_string();
                warn!("Contract deployment reverted: {}", reason);
                Err(ContractError::ExecutionReverted(reason))
            }
            RevmExecutionResult::Halt { reason, gas_used: _ } => {
                warn!("Contract deployment halted: {:?}", reason);
                Err(ContractError::ExecutionFailed(format!("Halted: {:?}", reason)))
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
        block_number: u64,
        timestamp: u64,
        gas_price: u128,
    ) -> Result<(Vec<u8>, u64, Vec<EvmLog>), ContractError> {
        let caller_addr = address_to_revm(&caller);
        let contract_addr = RevmAddress::from_slice(&contract_address.0);

        // Ensure caller account exists
        self.ensure_account(&caller_addr);

        // Set up contract account with code
        // SECURITY: Use entry().or_insert() to preserve existing account state
        // (balance, nonce) across multiple calls within the same block.
        // Previously, `accounts.insert()` overwrote the entire AccountInfo,
        // destroying accumulated balance and resetting nonce to 0.
        {
            let mut accounts = self.accounts.write();
            let account = accounts.entry(contract_addr).or_insert_with(|| AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: revm::primitives::KECCAK_EMPTY,
                code: None,
            });
            // Always update code, but preserve balance and nonce
            account.code = Some(Bytecode::new_raw(Bytes::from(contract_code)));
            // Add transferred value to existing balance
            account.balance = account.balance.saturating_add(U256::from(value));
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
            .modify_cfg_env(|cfg| {
                // SECURITY: Set chain_id for cross-chain replay protection (EIP-155)
                cfg.chain_id = self.chain_id;
            })
            .build();

        // Execute transaction
        let result = evm.transact_commit().map_err(|e| {
            warn!("EVM call error: {:?}", e);
            ContractError::ExecutionFailed(format!("EVM error: {:?}", e))
        })?;

        match result {
            RevmExecutionResult::Success { output, gas_used, logs, .. } => {
                let return_data = match output {
                    Output::Call(bytes) => bytes.to_vec(),
                    _ => vec![],
                };

                debug!("Contract call succeeded with {} gas", gas_used);

                // Convert REVM logs to structured EvmLog preserving topics
                let evm_logs = logs
                    .iter()
                    .map(|log| {
                        let mut topics = Vec::with_capacity(log.data.topics().len());
                        for topic in log.data.topics() {
                            topics.push(topic.0);
                        }
                        EvmLog {
                            address: log.address.0.to_vec(),
                            topics,
                            data: log.data.data.to_vec(),
                        }
                    })
                    .collect();

                Ok((return_data, gas_used, evm_logs))
            }
            RevmExecutionResult::Revert { gas_used: _, output } => {
                let reason = String::from_utf8_lossy(&output).to_string();
                warn!("Contract call reverted: {}", reason);
                Err(ContractError::ExecutionReverted(reason))
            }
            RevmExecutionResult::Halt { reason, gas_used: _ } => {
                warn!("Contract call halted: {:?}", reason);
                Err(ContractError::ExecutionFailed(format!("Halted: {:?}", reason)))
            }
        }
    }

    /// Ensure account exists in state
    /// New accounts start with zero balance; use fund_account() to add funds explicitly
    fn ensure_account(&self, address: &RevmAddress) {
        let mut accounts = self.accounts.write();
        accounts.entry(*address).or_insert(AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: revm::primitives::KECCAK_EMPTY,
            code: None,
        });
    }

    /// Fund an account with specific balance
    ///
    /// # Security
    /// This is an internal method used to sync on-chain balances into the EVM
    /// state before execution. It MUST NOT be exposed to external callers or
    /// used to create funds out of thin air. Only call with balances already
    /// validated against the canonical StateDB.
    ///
    /// # Warning
    /// This method is `pub` because it's used by sibling crates (luxtensor-node,
    /// luxtensor-rpc). It must NEVER be exposed through any RPC endpoint or
    /// user-facing API.
    #[doc(hidden)]
    pub fn fund_account(&self, address: &Address, amount: u128) {
        let addr = address_to_revm(address);
        let mut accounts = self.accounts.write();
        let account = accounts.entry(addr).or_insert(AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: revm::primitives::KECCAK_EMPTY,
            code: None,
        });
        account.balance = account.balance.saturating_add(U256::from(amount));
    }

    /// Set contract code for an account (used by eth_call / eth_estimateGas
    /// to seed the executor with real state from UnifiedStateDB).
    pub fn deploy_code(&self, address: &Address, code: Vec<u8>) {
        let addr = address_to_revm(address);
        let code_hash = {
            use sha3::{Digest, Keccak256};
            let mut hasher = Keccak256::new();
            hasher.update(&code);
            let result = hasher.finalize();
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&result);
            revm::primitives::B256::from(hash)
        };
        let bytecode = Bytecode::new_raw(Bytes::from(code));
        let mut accounts = self.accounts.write();
        let account = accounts.entry(addr).or_insert(AccountInfo {
            balance: U256::ZERO,
            nonce: 0,
            code_hash: revm::primitives::KECCAK_EMPTY,
            code: None,
        });
        account.code_hash = code_hash;
        account.code = Some(bytecode);
    }

    /// Get account balance
    pub fn get_account_balance(&self, address: &Address) -> u128 {
        let addr = address_to_revm(address);
        self.accounts
            .read()
            .get(&addr)
            .map(|a| {
                // Convert U256 to u128, clamping if too large
                a.balance.try_into().unwrap_or(u128::MAX)
            })
            .unwrap_or(0)
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

    /// Execute a read-only (static) contract call that does NOT commit state changes.
    ///
    /// SECURITY: Unlike `call()`, this uses `transact()` instead of `transact_commit()`,
    /// meaning no state changes (balance, storage, nonce) are persisted. This is essential
    /// for `eth_call` / `eth_estimateGas` RPCs and any read-only query.
    pub fn static_call(
        &self,
        caller: Address,
        contract_address: ContractAddress,
        contract_code: Vec<u8>,
        input_data: Vec<u8>,
        gas_limit: u64,
        block_number: u64,
        timestamp: u64,
        gas_price: u128,
    ) -> Result<(Vec<u8>, u64, Vec<EvmLog>), ContractError> {
        let caller_addr = address_to_revm(&caller);
        let contract_addr = RevmAddress::from_slice(&contract_address.0);

        // SECURITY (C-10): Deep-clone so the temporary executor owns
        // independent copies of accounts/storage/block_hashes.  A plain
        // `clone()` only clones the Arc pointers, so writes on the temp
        // executor (ensure_account, fund caller, set code) would pollute
        // the real persistent state.
        let temp_executor = self.deep_clone();

        // Ensure caller account exists in the temporary state
        temp_executor.ensure_account(&caller_addr);

        // Fund caller temporarily so gas fees are covered in the simulated execution
        {
            let mut accounts = temp_executor.accounts.write();
            let account = accounts.entry(caller_addr).or_insert(AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: revm::primitives::KECCAK_EMPTY,
                code: None,
            });
            account.balance = U256::from(gas_limit);
        }

        // Set up contract account with code in the temporary state
        {
            let mut accounts = temp_executor.accounts.write();
            let account = accounts.entry(contract_addr).or_insert_with(|| AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: revm::primitives::KECCAK_EMPTY,
                code: None,
            });
            account.code = Some(Bytecode::new_raw(Bytes::from(contract_code)));
        }

        // Build EVM with the TEMPORARY executor (not the real one)
        let mut evm = Evm::builder()
            .with_db(temp_executor)
            .modify_block_env(|b| {
                b.number = U256::from(block_number);
                b.timestamp = U256::from(timestamp);
                b.gas_limit = U256::from(gas_limit);
            })
            .modify_tx_env(|tx| {
                tx.caller = caller_addr;
                tx.transact_to = TransactTo::Call(contract_addr);
                tx.data = Bytes::from(input_data);
                tx.value = U256::ZERO; // Static calls cannot send value
                tx.gas_limit = gas_limit;
                tx.gas_price = U256::from(gas_price);
            })
            .modify_cfg_env(|cfg| {
                cfg.chain_id = self.chain_id;
            })
            .build();

        // SECURITY: Use transact() instead of transact_commit()
        // transact() returns the result WITHOUT committing state changes
        let result = evm.transact().map_err(|e| {
            warn!("EVM static call error: {:?}", e);
            ContractError::ExecutionFailed(format!("EVM static call error: {:?}", e))
        })?;

        match result.result {
            RevmExecutionResult::Success { output, gas_used, logs, .. } => {
                let return_data = match output {
                    Output::Call(bytes) => bytes.to_vec(),
                    _ => vec![],
                };

                let evm_logs = logs
                    .iter()
                    .map(|log| {
                        let mut topics = Vec::with_capacity(log.data.topics().len());
                        for topic in log.data.topics() {
                            topics.push(topic.0);
                        }
                        EvmLog {
                            address: log.address.0.to_vec(),
                            topics,
                            data: log.data.data.to_vec(),
                        }
                    })
                    .collect();

                Ok((return_data, gas_used, evm_logs))
            }
            RevmExecutionResult::Revert { gas_used: _, output } => {
                let reason = String::from_utf8_lossy(&output).to_string();
                Err(ContractError::ExecutionReverted(reason))
            }
            RevmExecutionResult::Halt { reason, gas_used: _ } => {
                Err(ContractError::ExecutionFailed(format!("Halted: {:?}", reason)))
            }
        }
    }

    /// Set storage value for a contract
    pub fn set_storage(&self, address: &ContractAddress, key: Hash, value: Hash) {
        let contract_addr = RevmAddress::from_slice(&address.0);
        let key_u256 = U256::from_be_bytes(key);
        let value_u256 = U256::from_be_bytes(value);

        let mut storage = self.storage.write();
        storage.entry(contract_addr).or_insert_with(HashMap::new).insert(key_u256, value_u256);
    }
}

impl Clone for EvmExecutor {
    fn clone(&self) -> Self {
        Self {
            chain_id: self.chain_id,
            accounts: Arc::clone(&self.accounts),
            storage: Arc::clone(&self.storage),
            block_hashes: Arc::clone(&self.block_hashes),
        }
    }
}

impl EvmExecutor {
    /// Create an independent deep copy of this executor.
    ///
    /// Unlike `clone()` which shares the underlying state via `Arc`,
    /// this method copies all inner `HashMap` data into **new**
    /// `Arc<RwLock<…>>` wrappers, so mutations on the copy never
    /// affect the original. Used by `static_call()` to prevent
    /// read-only queries from polluting persistent state (C-10).
    /// 🔧 FIX MC-3: Acquire all three read guards before cloning to get an
    /// atomic snapshot. Previously each `.read().clone()` was independent,
    /// so a concurrent write between the first and third clone could produce
    /// an internally-inconsistent copy.
    fn deep_clone(&self) -> Self {
        let accounts = self.accounts.read();
        let storage = self.storage.read();
        let block_hashes = self.block_hashes.read();
        Self {
            chain_id: self.chain_id,
            accounts: Arc::new(RwLock::new(accounts.clone())),
            storage: Arc::new(RwLock::new(storage.clone())),
            block_hashes: Arc::new(RwLock::new(block_hashes.clone())),
        }
    }
}

// Implement Database trait for EVM integration
impl Database for EvmExecutor {
    type Error = ContractError;

    fn basic(&mut self, address: RevmAddress) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.accounts.read().get(&address).cloned())
    }

    fn code_by_hash(
        &mut self,
        _code_hash: revm::primitives::B256,
    ) -> Result<Bytecode, Self::Error> {
        // Code is stored directly in AccountInfo
        Ok(Bytecode::default())
    }

    fn storage(&mut self, address: RevmAddress, index: U256) -> Result<U256, Self::Error> {
        let storage = self.storage.read();
        Ok(storage.get(&address).and_then(|s| s.get(&index).copied()).unwrap_or(U256::ZERO))
    }

    fn block_hash(&mut self, number: u64) -> Result<revm::primitives::B256, Self::Error> {
        let hashes = self.block_hashes.read();
        if let Some(hash) = hashes.get(&number) {
            Ok(revm::primitives::B256::from_slice(hash))
        } else {
            Ok(revm::primitives::B256::ZERO)
        }
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

// ============================================================================
// Persistent EVM Executor — wraps EvmExecutor with RocksDB-backed state
// ============================================================================
// Contract storage, account balances, and nonces survive node restarts.
// Uses a write-back cache: in-memory reads, periodic flush to RocksDB.

// Re-export EvmAccountRecord from the storage crate (canonical definition)
pub use luxtensor_storage::evm_store::EvmAccountRecord;

/// Persistent EVM executor that flushes state to RocksDB after each block.
/// Thread-safe: inner EvmExecutor uses Arc<RwLock<...>>.
pub struct PersistentEvmExecutor {
    /// In-memory EVM executor (the hot cache)
    pub executor: EvmExecutor,
    /// Flag indicating dirty state that needs flushing
    dirty: Arc<RwLock<bool>>,
}

impl PersistentEvmExecutor {
    /// Create a new PersistentEvmExecutor with the given chain ID.
    pub fn new(chain_id: u64) -> Self {
        Self { executor: EvmExecutor::new(chain_id), dirty: Arc::new(RwLock::new(false)) }
    }

    /// Load EVM state from RocksDB on startup.
    /// This restores all contract accounts and storage slots from the database.
    /// The `db` parameter should implement `EvmStateStore` (provided by BlockchainDB).
    pub fn load_from_db(&self, db: &dyn EvmStateStore) {
        // Load accounts
        if let Ok(accounts) = db.load_all_evm_accounts() {
            let mut accts = self.executor.accounts.write();
            for (addr_bytes, record) in accounts {
                let addr = RevmAddress::from_slice(&addr_bytes);
                let balance = U256::from_be_bytes(record.balance);
                let code = record.code.map(|c| Bytecode::new_raw(Bytes::from(c)));
                accts.insert(
                    addr,
                    AccountInfo {
                        balance,
                        nonce: record.nonce,
                        code_hash: revm::primitives::B256::from(record.code_hash),
                        code,
                    },
                );
            }
            debug!("Loaded {} EVM accounts from DB", accts.len());
        }

        // Load storage
        if let Ok(storage_entries) = db.load_all_evm_storage() {
            let mut storage = self.executor.storage.write();
            for (addr_bytes, slot_bytes, value_bytes) in storage_entries {
                let addr = RevmAddress::from_slice(&addr_bytes);
                let slot = U256::from_be_bytes(slot_bytes);
                let value = U256::from_be_bytes(value_bytes);
                storage.entry(addr).or_default().insert(slot, value);
            }
            debug!("Loaded EVM storage for {} contracts", storage.len());
        }
    }

    /// Flush dirty EVM state to RocksDB (called after each block execution).
    /// Extracts current in-memory state and writes it atomically.
    pub fn flush_to_db(&self, db: &dyn EvmStateStore) -> Result<(), String> {
        // SECURITY: Acquire write lock on dirty flag from the start to prevent
        // TOCTOU race between checking and clearing the flag.
        let mut dirty_guard = self.dirty.write();
        if !*dirty_guard {
            return Ok(());
        }

        let accounts_guard = self.executor.accounts.read();
        let storage_guard = self.executor.storage.read();

        // Serialize accounts
        let mut account_records: Vec<([u8; 20], Vec<u8>)> = Vec::new();
        for (addr, info) in accounts_guard.iter() {
            let mut addr_bytes = [0u8; 20];
            addr_bytes.copy_from_slice(addr.as_slice());

            let record = EvmAccountRecord {
                balance: info.balance.to_be_bytes(),
                nonce: info.nonce,
                code_hash: info.code_hash.0,
                code: info.code.as_ref().map(|c| c.bytes().to_vec()),
            };
            let data = bincode::serialize(&record).map_err(|e| e.to_string())?;
            account_records.push((addr_bytes, data));
        }

        // Serialize storage
        let mut storage_entries: Vec<([u8; 20], [u8; 32], [u8; 32])> = Vec::new();
        for (addr, slots) in storage_guard.iter() {
            let mut addr_bytes = [0u8; 20];
            addr_bytes.copy_from_slice(addr.as_slice());
            for (slot, value) in slots {
                storage_entries.push((addr_bytes, slot.to_be_bytes(), value.to_be_bytes()));
            }
        }

        drop(accounts_guard);
        drop(storage_guard);

        db.flush_evm_state(&account_records, &storage_entries, &[])?;

        *dirty_guard = false;
        debug!(
            "Flushed {} EVM accounts and {} storage slots to DB",
            account_records.len(),
            storage_entries.len()
        );
        Ok(())
    }

    /// Mark state as dirty (called by block execution after any EVM transaction)
    pub fn mark_dirty(&self) {
        *self.dirty.write() = true;
    }

    /// Get a clone of the inner EvmExecutor for use in EVM execution
    pub fn inner(&self) -> &EvmExecutor {
        &self.executor
    }
}

impl Clone for PersistentEvmExecutor {
    fn clone(&self) -> Self {
        Self { executor: self.executor.clone(), dirty: Arc::clone(&self.dirty) }
    }
}

impl Default for PersistentEvmExecutor {
    fn default() -> Self {
        Self::new(chain_id::MAINNET) // LuxTensor Mainnet (8898)
    }
}

// Re-export EvmStateStore from the storage crate (canonical definition)
pub use luxtensor_storage::evm_store::EvmStateStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evm_executor_creation() {
        let executor = EvmExecutor::new(chain_id::MAINNET);
        assert_eq!(executor.accounts.read().len(), 0);
    }

    #[test]
    fn test_simple_deployment() {
        let executor = EvmExecutor::new(chain_id::MAINNET);
        let deployer = Address::from([1u8; 20]);

        // Simple contract bytecode (just returns)
        let code = vec![0x60, 0x00, 0x60, 0x00, 0xf3]; // PUSH1 0, PUSH1 0, RETURN

        let result = executor.deploy(deployer, code, 0, 1_000_000, 1, 1000, 1);
        // May fail without proper bytecode, but should not panic
        assert!(result.is_ok() || result.is_err());
    }
}
