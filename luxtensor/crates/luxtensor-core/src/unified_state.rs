//! Unified State Database
//!
//! This module consolidates all state sources into a single unified StateDB:
//! - Account state (balances, nonces)
//! - EVM state (contracts, storage)
//! - Vector store (HNSW for semantic search)
//!
//! # Architecture
//!
//! The design intentionally separates **pure storage operations** (get/set
//! for balances, nonces, contracts, storage slots) from **business logic
//! operations** (transfers with overflow/underflow checks, contract
//! deployment, state root calculation).
//!
//! Consumers should prefer the [`TransferOps`] trait for balance mutations
//! in production code, as it enforces all invariants in one place.
//!
//! ```text
//! ┌───────────────────────────────────────────────────────────┐
//! │                    UnifiedStateDB                           │
//! ├───────────────────────────────────────────────────────────┤
//! │            « Pure Storage Layer »                             │
//! │  get/set_balance, get/set_nonce, get/set_storage            │
//! ├───────────────────────────────────────────────────────────┤
//! │         « Business Logic Layer (TransferOps) »               │
//! │  credit, debit, transfer, deploy_contract                  │
//! ├───────────────────┬───────────────────┬───────────────────┤
//! │  AccountState      │  ContractState    │  VectorStore      │
//! │  balances, nonces  │  code, storage    │  HNSW embeddings  │
//! └───────────────────┴───────────────────┴───────────────────┘
//!                    │ Persistence Layer (RocksDB) │
//!                    └─────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::{Account, Address, Hash, Result, CoreError};
use crate::constants::chain_id;
use crate::hnsw::HnswVectorStore;

// =================== Transfer Operations Trait ===================
//
// This trait encapsulates all balance-mutation business logic, making
// the invariant enforcement (overflow/underflow checks, nonce
// ordering) testable and swappable independently of the storage
// backing.

/// Business-logic trait for balance/nonce mutations with invariant
/// enforcement.
///
/// Separating this from the raw storage operations in [`UnifiedStateDB`]
/// improves testability and ensures all balance changes go through
/// checked paths.  `UnifiedStateDB` implements this trait directly;
/// consumers should prefer using it for production mutations.
pub trait TransferOps {
    /// Credit `amount` to `address`, returning an error on overflow.
    fn credit(&mut self, address: &Address, amount: u128) -> Result<()>;

    /// Debit `amount` from `address`, returning an error if the balance
    /// is insufficient.
    fn debit(&mut self, address: &Address, amount: u128) -> Result<()>;

    /// Atomically transfer `amount` from `from` to `to`.
    ///
    /// This is the preferred entry point for value transfers.  It debit
    /// first (to fail-fast on insufficient balance) and then credits.
    fn transfer(&mut self, from: &Address, to: &Address, amount: u128) -> Result<()>;

    /// Increment the sender’s nonce (checked, panics on overflow).
    fn increment_nonce(&mut self, address: &Address) -> Result<()>;
}

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

    // =================== Pure Storage Layer ===================
    //
    // These methods perform raw reads/writes with no business-logic
    // invariant checking.  For checked balance mutations prefer the
    // `TransferOps` trait (implemented below).

    /// Get account balance
    pub fn get_balance(&self, address: &Address) -> u128 {
        *self.balances.get(address).unwrap_or(&0)
    }

    /// Set account balance (raw — no overflow check)
    pub fn set_balance(&mut self, address: Address, balance: u128) {
        self.balances.insert(address, balance);
        self.dirty = true;
    }

    /// Get account nonce
    pub fn get_nonce(&self, address: &Address) -> u64 {
        *self.nonces.get(address).unwrap_or(&0)
    }

    /// Set account nonce directly (raw — used by state sync from block
    /// production)
    pub fn set_nonce(&mut self, address: Address, nonce: u64) {
        self.nonces.insert(address, nonce);
        self.dirty = true;
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
    ///
    /// # Panics
    /// Panics if block number overflows `u64::MAX`.
    pub fn advance_block(&mut self) {
        self.block_number = self.block_number.checked_add(1).expect("block number overflow");
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
        // Pre-allocate the per-leaf data buffer: 20 (addr) + 16 (balance) + 8 (nonce)
        let mut data = Vec::with_capacity(44);
        for addr in addresses {
            let balance = self.get_balance(addr);
            let nonce = self.get_nonce(addr);
            data.clear();
            data.extend_from_slice(addr.as_bytes());
            data.extend_from_slice(&balance.to_le_bytes());
            data.extend_from_slice(&nonce.to_le_bytes());
            // SECURITY: Use hash_leaf (0x00 prefix) to prevent second-preimage attacks
            leaves.push(luxtensor_crypto::MerkleTree::hash_leaf(&data));
        }

        Ok(luxtensor_crypto::MerkleTree::new(leaves).root())
    }

    fn calculate_contract_root(&self) -> Result<Hash> {
        if self.contracts.is_empty() && self.storage.is_empty() {
            return Ok([0u8; 32]);
        }

        let mut items: Vec<_> = self.contracts.keys().collect();
        items.sort();

        let mut leaves = Vec::with_capacity(items.len());
        // Pre-allocate the per-contract data buffer: 20 (addr) + 32 (code_hash)
        let mut data = Vec::with_capacity(52);
        for addr in items {
            if let Some(info) = self.contracts.get(addr) {
                let code_hash = luxtensor_crypto::keccak256(&info.code);
                data.clear();
                data.extend_from_slice(addr.as_bytes());
                data.extend_from_slice(&code_hash);
                // SECURITY: Use hash_leaf (0x00 prefix) to prevent second-preimage attacks
                leaves.push(luxtensor_crypto::MerkleTree::hash_leaf(&data));
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
        Self::new(chain_id::DEVNET) // LuxTensor devnet chain ID (8898)
    }
}

// =================== TransferOps Implementation ===================
//
// Business logic for balance mutations lives here, cleanly separated
// from the raw storage layer above.

impl TransferOps for UnifiedStateDB {
    fn credit(&mut self, address: &Address, amount: u128) -> Result<()> {
        let balance = self.balances.entry(*address).or_insert(0);
        *balance = balance.checked_add(amount).ok_or(CoreError::BalanceOverflow)?;
        self.dirty = true;
        Ok(())
    }

    fn debit(&mut self, address: &Address, amount: u128) -> Result<()> {
        let balance = self.balances.entry(*address).or_insert(0);
        if *balance < amount {
            return Err(CoreError::InsufficientBalance);
        }
        *balance -= amount;
        self.dirty = true;
        Ok(())
    }

    fn transfer(&mut self, from: &Address, to: &Address, amount: u128) -> Result<()> {
        // Debit first to fail-fast on insufficient balance.
        self.debit(from, amount)?;
        // Credit cannot overflow in practice (total supply is bounded),
        // but the checked add guards against bugs higher up.
        self.credit(to, amount)?;
        Ok(())
    }

    fn increment_nonce(&mut self, address: &Address) -> Result<()> {
        let nonce = self.nonces.entry(*address).or_insert(0);
        *nonce = nonce.checked_add(1).ok_or(CoreError::NonceOverflow)?;
        self.dirty = true;
        Ok(())
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

impl UnifiedStateDB {
    /// Synchronize this UnifiedStateDB from a StateDB snapshot.
    ///
    /// Called after each block is produced so that the RPC layer reflects
    /// the latest on-chain state (balances, nonces, contract code).
    /// `new_block_number` should be the height of the just-produced block.
    pub fn sync_from_state_db(&mut self, state: &crate::StateDB, new_block_number: u64) {
        for (address, account) in state.accounts() {
            self.balances.insert(*address, account.balance);
            self.nonces.insert(*address, account.nonce);

            // Sync contract code if present
            if let Some(ref code) = account.code {
                if !code.is_empty() && !self.contracts.contains_key(address) {
                    self.contracts.insert(*address, ContractInfo {
                        code: code.clone(),
                        deployer: Address::zero(), // deployer unknown from StateDB
                        deploy_block: new_block_number,
                    });
                }
            }
        }
        self.block_number = new_block_number;
        self.dirty = true;
    }
}

// =================== FIX-4: HNSW Persistence Bridge ===================
//
// Problem: UnifiedStateDB keeps the HNSW vector store purely in-memory.
// On node restart all indexed vectors are lost, breaking the Semantic Layer.
//
// Solution: expose serialization helpers so the node service can cheaply
// checkpoint the HNSW index into RocksDB-backed StateDB after each block
// and restore it on startup.  This avoids a circular crate dependency
// (luxtensor-core cannot depend on luxtensor-storage) by providing raw bytes
// that the *caller* passes to StateDB::set_hnsw_index / get_hnsw_index.
//
// Usage (in node service, after each block):
//
//   let bytes = unified_state.read().to_hnsw_bytes();
//   state_db.set_hnsw_index(UnifiedStateDB::HNSW_INDEX_NAME, bytes)?;
//
// Usage (on startup):
//
//   if let Some(bytes) = state_db.get_hnsw_index(UnifiedStateDB::HNSW_INDEX_NAME)? {
//       unified_state.write().restore_hnsw_from_bytes(&bytes)?;
//   }

impl UnifiedStateDB {
    /// Canonical RocksDB key used to store the HNSW index.
    ///
    /// Using a single stable name means the node always overwrites the same
    /// key on each checkpoint, keeping storage usage bounded.
    pub const HNSW_INDEX_NAME: &'static str = "unified_state_hnsw_v1";

    /// Serialize the current HNSW vector index to raw bytes.
    ///
    /// The bytes can be stored via `StateDB::set_hnsw_index()` or any other
    /// persistent backend.  The format is the same as `HnswVectorStore::to_bytes()`.
    ///
    /// Returns an empty `Vec` if the index is empty (skip writing to avoid
    /// unnecessary I/O on nodes that have never indexed any vectors).
    pub fn to_hnsw_bytes(&self) -> Vec<u8> {
        if self.vector_store.is_empty() {
            return Vec::new();
        }
        self.vector_store.to_bytes()
    }

    /// Restore the HNSW vector index from raw bytes previously produced by
    /// `to_hnsw_bytes()`.
    ///
    /// If `bytes` is empty this is a no-op (preserving existing state), so
    /// callers can safely pass the result of `StateDB::get_hnsw_index()` even
    /// when it returns `None` (map to `&[]`).
    ///
    /// # Errors
    /// Returns `CoreError::HnswDeserialization` if the bytes are malformed.
    pub fn restore_hnsw_from_bytes(&mut self, bytes: &[u8]) -> crate::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        match crate::hnsw::HnswVectorStore::from_bytes(bytes) {
            Ok(store) => {
                self.vector_store = store;
                self.dirty = true;
                tracing::info!(
                    "🧠 HNSW index restored: {} vectors",
                    self.vector_store.len()
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!("❌ HNSW index restoration failed: {:?}", e);
                Err(crate::CoreError::HnswDeserialization(format!("{e:?}")))
            }
        }
    }

    /// Returns `true` if the HNSW index has been modified since the last
    /// call to `mark_hnsw_clean()`.
    ///
    /// Node services can use this to avoid writing identical bytes to RocksDB
    /// on every block when the vector store has not changed.
    pub fn is_hnsw_dirty(&self) -> bool {
        self.dirty && !self.vector_store.is_empty()
    }

    /// Mark the HNSW index as clean (called after a successful checkpoint).
    ///
    /// Only clears the dirty flag; does not modify the vector data.
    pub fn mark_hnsw_clean(&mut self) {
        // We only clear the flag — the next mutation will re-set it.
        self.dirty = false;
    }
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

    // ── TransferOps trait tests ──────────────────────────────────────

    #[test]
    fn test_transfer_success() {
        let mut state = UnifiedStateDB::new(1);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        state.credit(&alice, 500).unwrap();
        state.transfer(&alice, &bob, 200).unwrap();

        assert_eq!(state.get_balance(&alice), 300);
        assert_eq!(state.get_balance(&bob), 200);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let mut state = UnifiedStateDB::new(1);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        state.credit(&alice, 100).unwrap();

        // Transfer more than balance — should fail and leave state unchanged
        let result = state.transfer(&alice, &bob, 200);
        assert!(result.is_err());
        assert_eq!(state.get_balance(&alice), 100); // unchanged
        assert_eq!(state.get_balance(&bob), 0);     // unchanged
    }

    #[test]
    fn test_transfer_self() {
        let mut state = UnifiedStateDB::new(1);
        let alice = Address::from([1u8; 20]);

        state.credit(&alice, 1000).unwrap();
        // Self-transfer is allowed (debit then credit = no-op on balance)
        state.transfer(&alice, &alice, 100).unwrap();
        assert_eq!(state.get_balance(&alice), 1000);
    }

    #[test]
    fn test_commit_idempotent() {
        let mut state = UnifiedStateDB::new(1);
        let addr = Address::from([1u8; 20]);
        state.credit(&addr, 500).unwrap();

        let root1 = state.commit().unwrap();
        let root2 = state.commit().unwrap();
        assert_eq!(root1, root2, "Committing twice without changes should yield the same root");
    }

    #[test]
    fn test_state_root_deterministic() {
        // Two states with identical content must produce the same root.
        let mut s1 = UnifiedStateDB::new(1);
        let mut s2 = UnifiedStateDB::new(1);
        let addr = Address::from([5u8; 20]);

        s1.credit(&addr, 42).unwrap();
        s2.credit(&addr, 42).unwrap();

        assert_eq!(
            s1.root_hash().unwrap(),
            s2.root_hash().unwrap(),
            "Identical states must produce identical roots"
        );
    }
}
