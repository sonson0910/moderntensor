//! Unified State Database
//!
//! This module consolidates all state sources into a single unified StateDB:
//! - Account state (balances, nonces)
//! - EVM state (contracts, storage)
//! - Vector store (HNSW for semantic search)
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      UnifiedStateDB                             │
//! ├─────────────────┬─────────────────┬───────────────────────────┬─┤
//! │  AccountState   │   ContractState │  VectorStore (HNSW)       │ │
//! │  - balances     │   - bytecode    │  - embeddings             │ │
//! │  - nonces       │   - storage     │  - ANN search             │ │
//! └─────────────────┴─────────────────┴───────────────────────────┴─┘
//!                    │ Persistence Layer (RocksDB) │
//!                    └─────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::{Account, Address, Hash, Result, CoreError};
use crate::hnsw::HnswVectorStore;

/// Deployed contract information
#[derive(Debug, Clone)]
pub struct ContractInfo {
    /// Contract bytecode
    pub code: Vec<u8>,
    /// Deployer address
    pub deployer: Address,
    /// Deployment block
    pub deploy_block: u64,
}

/// Contract storage slot
pub type StorageSlot = [u8; 32];

/// Unified State Database consolidating all state sources
///
/// This struct replaces the fragmented state across:
/// - `StateDB` (luxtensor-core)
/// - `EvmState` (luxtensor-rpc)
/// - `EvmExecutor` (luxtensor-contracts)
pub struct UnifiedStateDB {
    /// Account balances
    balances: HashMap<Address, u128>,
    /// Account nonces
    nonces: HashMap<Address, u64>,
    /// Contract bytecode registry
    contracts: HashMap<Address, ContractInfo>,
    /// Contract storage: (address, slot) -> value
    storage: HashMap<(Address, StorageSlot), StorageSlot>,
    /// Current block number
    block_number: u64,
    /// Chain ID
    chain_id: u64,
    /// HNSW-backed vector store for AI/Semantic Layer
    pub vector_store: HnswVectorStore,
    /// Dirty flag for persistence
    dirty: bool,
}

impl UnifiedStateDB {
    /// Create a new UnifiedStateDB for production
    pub fn new(chain_id: u64) -> Self {
        Self {
            balances: HashMap::new(),
            nonces: HashMap::new(),
            contracts: HashMap::new(),
            storage: HashMap::new(),
            block_number: 0,
            chain_id,
            vector_store: HnswVectorStore::new(768), // Default dimension
            dirty: true,
        }
    }

    /// Create a new UnifiedStateDB for development with pre-funded accounts
    pub fn new_dev(chain_id: u64) -> Self {
        let mut state = Self::new(chain_id);

        // Pre-fund test accounts (same as EvmState::new_dev)
        let test_accounts: [[u8; 20]; 3] = [
            [0xf3, 0x9F, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xF6, 0xF4, 0xce,
             0x6a, 0xB8, 0x82, 0x72, 0x79, 0xcf, 0xfF, 0xb9, 0x22, 0x66],
            [0x70, 0x99, 0x79, 0x70, 0xC5, 0x18, 0x12, 0xdc, 0x3A, 0x01,
             0x0C, 0x7d, 0x01, 0xb5, 0x0e, 0x0d, 0x17, 0xdc, 0x79, 0xC8],
            [0x3C, 0x44, 0xCd, 0xDd, 0xB6, 0xa9, 0x00, 0xfa, 0x2b, 0x58,
             0x5d, 0xd2, 0x99, 0xe0, 0x3d, 0x12, 0xFA, 0x42, 0x93, 0xBC],
        ];

        for account in test_accounts {
            state.balances.insert(
                Address::from(account),
                1_000_000_000_000_000_000_000u128 // 1000 ETH
            );
        }

        state
    }

    // =================== Account Operations ===================

    /// Get account balance
    pub fn get_balance(&self, address: &Address) -> u128 {
        *self.balances.get(address).unwrap_or(&0)
    }

    /// Set account balance
    pub fn set_balance(&mut self, address: Address, balance: u128) {
        self.balances.insert(address, balance);
        self.dirty = true;
    }

    /// Credit account (add to balance, returns error on overflow)
    pub fn credit(&mut self, address: &Address, amount: u128) -> Result<()> {
        let balance = self.balances.entry(*address).or_insert(0);
        *balance = balance.checked_add(amount).ok_or(CoreError::BalanceOverflow)?;
        self.dirty = true;
        Ok(())
    }

    /// Debit account (subtract from balance)
    pub fn debit(&mut self, address: &Address, amount: u128) -> Result<()> {
        let balance = self.balances.entry(*address).or_insert(0);
        if *balance < amount {
            return Err(CoreError::InsufficientBalance);
        }
        *balance -= amount;
        self.dirty = true;
        Ok(())
    }

    /// Get account nonce
    pub fn get_nonce(&self, address: &Address) -> u64 {
        *self.nonces.get(address).unwrap_or(&0)
    }

    /// Increment account nonce (checked to prevent overflow)
    pub fn increment_nonce(&mut self, address: &Address) -> Result<()> {
        let nonce = self.nonces.entry(*address).or_insert(0);
        *nonce = nonce.checked_add(1).ok_or(CoreError::NonceOverflow)?;
        self.dirty = true;
        Ok(())
    }

    /// Get full account info
    pub fn get_account(&self, address: &Address) -> Option<Account> {
        let balance = self.get_balance(address);
        let nonce = self.get_nonce(address);

        // Check if account exists (has balance or nonce)
        if balance == 0 && nonce == 0 && !self.has_code(address) {
            return None;
        }

        // Build Account with all fields
        let mut account = Account::with_balance(balance);
        account.nonce = nonce;

        // If this is a contract, add code info
        if let Some(info) = self.contracts.get(address) {
            account.code = Some(info.code.clone());
            account.code_hash = luxtensor_crypto::keccak256(&info.code);
        }

        Some(account)
    }

    // =================== Contract Operations ===================

    /// Deploy a contract
    pub fn deploy_contract(
        &mut self,
        address: Address,
        code: Vec<u8>,
        deployer: Address,
    ) {
        self.contracts.insert(address, ContractInfo {
            code,
            deployer,
            deploy_block: self.block_number,
        });
        self.dirty = true;
    }

    /// Get contract bytecode
    pub fn get_code(&self, address: &Address) -> Option<Vec<u8>> {
        self.contracts.get(address).map(|c| c.code.clone())
    }

    /// Check if address has contract code
    pub fn has_code(&self, address: &Address) -> bool {
        self.contracts.contains_key(address)
    }

    /// Get contract storage
    pub fn get_storage(&self, address: &Address, slot: &StorageSlot) -> StorageSlot {
        self.storage.get(&(*address, *slot)).copied().unwrap_or([0u8; 32])
    }

    /// Set contract storage
    pub fn set_storage(&mut self, address: Address, slot: StorageSlot, value: StorageSlot) {
        if value == [0u8; 32] {
            self.storage.remove(&(address, slot));
        } else {
            self.storage.insert((address, slot), value);
        }
        self.dirty = true;
    }

    // =================== Block Operations ===================

    /// Get current block number
    pub fn block_number(&self) -> u64 {
        self.block_number
    }

    /// Advance to next block
    pub fn advance_block(&mut self) {
        self.block_number += 1;
        self.dirty = true;
    }

    /// Set block number (for sync)
    pub fn set_block_number(&mut self, number: u64) {
        self.block_number = number;
        self.dirty = true;
    }

    /// Get chain ID
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    // =================== State Root ===================

    /// Calculate state root hash (hybrid: account + contract + vector)
    pub fn root_hash(&self) -> Result<Hash> {
        // 1. Account state root
        let account_root = self.calculate_account_root()?;

        // 2. Contract state root
        let contract_root = self.calculate_contract_root()?;

        // 3. Vector state root
        let vector_root = self.vector_store.root_hash();

        // 4. Combine all roots
        let mut combined = Vec::with_capacity(96);
        combined.extend_from_slice(&account_root);
        combined.extend_from_slice(&contract_root);
        combined.extend_from_slice(&vector_root);

        Ok(luxtensor_crypto::keccak256(&combined))
    }

    fn calculate_account_root(&self) -> Result<Hash> {
        if self.balances.is_empty() && self.nonces.is_empty() {
            return Ok([0u8; 32]);
        }

        // Get all unique addresses
        let mut addresses: Vec<_> = self.balances.keys()
            .chain(self.nonces.keys())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        addresses.sort();

        let mut leaves = Vec::with_capacity(addresses.len());
        for addr in addresses {
            let balance = self.get_balance(addr);
            let nonce = self.get_nonce(addr);
            let mut data = Vec::new();
            data.extend_from_slice(addr.as_bytes());
            data.extend_from_slice(&balance.to_le_bytes());
            data.extend_from_slice(&nonce.to_le_bytes());
            leaves.push(luxtensor_crypto::keccak256(&data));
        }

        Ok(luxtensor_crypto::MerkleTree::new(leaves).root())
    }

    fn calculate_contract_root(&self) -> Result<Hash> {
        if self.contracts.is_empty() && self.storage.is_empty() {
            return Ok([0u8; 32]);
        }

        let mut items: Vec<_> = self.contracts.keys().collect();
        items.sort();

        let mut leaves = Vec::new();
        for addr in items {
            if let Some(info) = self.contracts.get(addr) {
                let code_hash = luxtensor_crypto::keccak256(&info.code);
                let mut data = Vec::new();
                data.extend_from_slice(addr.as_bytes());
                data.extend_from_slice(&code_hash);
                leaves.push(luxtensor_crypto::keccak256(&data));
            }
        }

        if leaves.is_empty() {
            return Ok([0u8; 32]);
        }

        Ok(luxtensor_crypto::MerkleTree::new(leaves).root())
    }

    /// Commit state and return root hash
    pub fn commit(&mut self) -> Result<Hash> {
        let root = self.root_hash()?;
        self.dirty = false;
        Ok(root)
    }

    /// Check if state has uncommitted changes
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // =================== Statistics ===================

    /// Get state statistics
    pub fn stats(&self) -> UnifiedStateStats {
        UnifiedStateStats {
            account_count: self.balances.len().max(self.nonces.len()),
            contract_count: self.contracts.len(),
            storage_slot_count: self.storage.len(),
            vector_count: self.vector_store.len(),
            block_number: self.block_number,
        }
    }
}

impl Default for UnifiedStateDB {
    fn default() -> Self {
        Self::new(1) // Default chain ID
    }
}

/// State statistics
#[derive(Debug, Clone)]
pub struct UnifiedStateStats {
    pub account_count: usize,
    pub contract_count: usize,
    pub storage_slot_count: usize,
    pub vector_count: usize,
    pub block_number: u64,
}

/// Thread-safe wrapper for UnifiedStateDB
pub type SharedUnifiedState = Arc<RwLock<UnifiedStateDB>>;

/// Create a new shared unified state
pub fn new_shared_state(chain_id: u64) -> SharedUnifiedState {
    Arc::new(RwLock::new(UnifiedStateDB::new(chain_id)))
}

/// Create a new shared unified state for development
pub fn new_shared_state_dev(chain_id: u64) -> SharedUnifiedState {
    Arc::new(RwLock::new(UnifiedStateDB::new_dev(chain_id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_state_creation() {
        let state = UnifiedStateDB::new(1);
        assert_eq!(state.block_number(), 0);
        assert_eq!(state.chain_id(), 1);
    }

    #[test]
    fn test_balance_operations() {
        let mut state = UnifiedStateDB::new(1);
        let addr = Address::from([1u8; 20]);

        assert_eq!(state.get_balance(&addr), 0);

        state.credit(&addr, 1000).unwrap();
        assert_eq!(state.get_balance(&addr), 1000);

        state.debit(&addr, 300).unwrap();
        assert_eq!(state.get_balance(&addr), 700);

        // Insufficient balance
        assert!(state.debit(&addr, 800).is_err());
    }

    #[test]
    fn test_nonce_operations() {
        let mut state = UnifiedStateDB::new(1);
        let addr = Address::from([2u8; 20]);

        assert_eq!(state.get_nonce(&addr), 0);

        state.increment_nonce(&addr).unwrap();
        assert_eq!(state.get_nonce(&addr), 1);

        state.increment_nonce(&addr).unwrap();
        assert_eq!(state.get_nonce(&addr), 2);
    }

    #[test]
    fn test_contract_operations() {
        let mut state = UnifiedStateDB::new(1);
        let deployer = Address::from([1u8; 20]);
        let contract = Address::from([2u8; 20]);
        let code = vec![0x60, 0x00, 0xfd]; // PUSH1 0, REVERT

        state.deploy_contract(contract, code.clone(), deployer);

        assert!(state.has_code(&contract));
        assert_eq!(state.get_code(&contract), Some(code));
        assert!(!state.has_code(&deployer));
    }

    #[test]
    fn test_storage_operations() {
        let mut state = UnifiedStateDB::new(1);
        let addr = Address::from([1u8; 20]);
        let slot = [1u8; 32];
        let value = [2u8; 32];

        assert_eq!(state.get_storage(&addr, &slot), [0u8; 32]);

        state.set_storage(addr, slot, value);
        assert_eq!(state.get_storage(&addr, &slot), value);

        // Setting to zero removes the slot
        state.set_storage(addr, slot, [0u8; 32]);
        assert_eq!(state.get_storage(&addr, &slot), [0u8; 32]);
    }

    #[test]
    fn test_dev_prefunded() {
        let state = UnifiedStateDB::new_dev(1);
        let stats = state.stats();
        assert_eq!(stats.account_count, 3); // 3 pre-funded accounts
    }

    #[test]
    fn test_state_root() {
        let mut state = UnifiedStateDB::new(1);
        let addr = Address::from([1u8; 20]);

        let root1 = state.root_hash().unwrap();

        state.credit(&addr, 1000).unwrap();
        let root2 = state.root_hash().unwrap();

        assert_ne!(root1, root2); // State changed, root should change
    }
}
