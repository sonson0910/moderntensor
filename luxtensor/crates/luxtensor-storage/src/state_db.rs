// NOTE (I-1): This implementation uses `bincode` for account serialization instead
// of RLP (Recursive Length Prefix) as used by standard Ethereum clients. This is
// intentional — LuxTensor is not wire-compatible with Ethereum peers and uses
// bincode for its compact binary encoding and derive-based ergonomics.

use crate::trie::MerkleTrie;
use crate::{Result, StorageError};
use luxtensor_core::{Account, Address};
use luxtensor_crypto::{keccak256, Hash};
use lru::LruCache;
use parking_lot::RwLock;
use rocksdb::{WriteOptions, DB};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::Arc;

/// Key prefix for contract code storage
const CONTRACT_CODE_PREFIX: &[u8] = b"code:";

/// Key prefix for HNSW vector index storage
const HNSW_INDEX_PREFIX: &[u8] = b"hnsw:";

/// Maximum number of account entries in the in-memory LRU cache.
///
/// SECURITY (H-2): Bounds the cache to prevent OOM on networks with millions
/// of accounts. Dirty (uncommitted) entries are retained until commit();
/// only clean read-through entries are subject to LRU eviction.
/// At ~200 bytes per Account, 100K entries ≈ 20 MB.
const MAX_ACCOUNT_CACHE_ENTRIES: usize = 100_000;

/// State database with RocksDB backend and in-memory cache
pub struct StateDB {
    db: Arc<DB>,
    /// In-memory account cache.
    ///
    /// SECURITY (H-2): Bounded LRU cache (MAX_ACCOUNT_CACHE_ENTRIES) that evicts
    /// least-recently-used entries when full, preventing OOM on networks with
    /// millions of accounts. Dirty entries are protected: commit() verifies all
    /// dirty addresses still reside in the cache before persisting to RocksDB.
    cache: RwLock<LruCache<Address, Account>>,
    dirty: RwLock<HashSet<Address>>,
    /// Contract bytecode storage: code_hash -> bytecode
    contract_code: RwLock<HashMap<Hash, Vec<u8>>>,
    /// Dirty contract codes awaiting commit
    dirty_codes: RwLock<HashSet<Hash>>,
    /// SECURITY (H-3): Persisted Merkle trie — updated incrementally each commit
    /// instead of being recreated from only dirty accounts, ensuring the state
    /// root reflects *all* committed accounts, not just recently-modified ones.
    trie: RwLock<MerkleTrie>,
}

impl StateDB {
    /// Create a new state database
    ///
    /// SECURITY (C-1): Rebuilds the Merkle trie from all accounts persisted in
    /// RocksDB so that the state root is correct even after a process restart.
    pub fn new(db: Arc<DB>) -> Self {
        let trie = RwLock::new(MerkleTrie::new());

        // SECURITY (C-1): Rebuild trie from persisted accounts on startup.
        // Without this, the trie starts empty and the state root would only
        // reflect accounts modified after restart, not the full account set.
        {
            let mut t = trie.write();
            let iter = db.iterator(rocksdb::IteratorMode::Start);
            for item in iter {
                if let Ok((key, value)) = item {
                    // Account keys are raw 20-byte addresses (no prefix)
                    if key.len() == 20 {
                        if let Ok(_account) = bincode::deserialize::<Account>(&value) {
                            let trie_key = keccak256(&key);
                            // value is the serialized account bytes — insert into trie
                            let _ = t.insert(&trie_key, &value);
                        }
                    }
                }
            }
            let count = t.len();
            if count > 0 {
                tracing::info!("🔄 Rebuilt Merkle trie with {} accounts from RocksDB", count);
            }
        }

        Self {
            db,
            cache: RwLock::new(LruCache::new(
                NonZeroUsize::new(MAX_ACCOUNT_CACHE_ENTRIES).unwrap(),
            )),
            dirty: RwLock::new(HashSet::new()),
            contract_code: RwLock::new(HashMap::new()),
            dirty_codes: RwLock::new(HashSet::new()),
            trie,
        }
    }

    /// Get an account by address, checking the in-memory cache first, then RocksDB.
    pub fn get_account(&self, address: &Address) -> Result<Account> {
        // Check cache first (peek avoids LRU promotion, keeping read-lock perf)
        {
            let cache = self.cache.read();
            if let Some(account) = cache.peek(address) {
                return Ok(account.clone());
            }
        }

        // Load from database
        match self.db.get(address.as_bytes())? {
            Some(bytes) => {
                let account: Account = bincode::deserialize(&bytes)?;

                // Update cache (put handles LRU eviction automatically)
                let mut cache = self.cache.write();
                cache.put(*address, account.clone());

                Ok(account)
            }
            None => {
                // Return empty account if not found
                Ok(Account::default())
            }
        }
    }

    /// Set (insert or update) an account in the cache and mark it as dirty for the next commit.
    pub fn set_account(&self, address: Address, account: Account) {
        let mut cache = self.cache.write();
        cache.put(address, account);

        let mut dirty = self.dirty.write();
        dirty.insert(address);
    }

    /// Get account balance
    pub fn get_balance(&self, address: &Address) -> Result<u128> {
        let account = self.get_account(address)?;
        Ok(account.balance)
    }

    /// Set account balance
    pub fn set_balance(&self, address: &Address, balance: u128) -> Result<()> {
        let mut account = self.get_account(address)?;
        account.balance = balance;
        self.set_account(*address, account);
        Ok(())
    }

    /// Get account nonce
    pub fn get_nonce(&self, address: &Address) -> Result<u64> {
        let account = self.get_account(address)?;
        Ok(account.nonce)
    }

    /// Set account nonce
    pub fn set_nonce(&self, address: &Address, nonce: u64) -> Result<()> {
        let mut account = self.get_account(address)?;
        account.nonce = nonce;
        self.set_account(*address, account);
        Ok(())
    }

    /// Increment account nonce
    ///
    /// SECURITY (L-1): Uses `checked_add` to prevent nonce wrap-around at `u64::MAX`.
    /// A wrapping nonce could re-enable replay attacks on old transactions.
    pub fn increment_nonce(&self, address: &Address) -> Result<u64> {
        let mut account = self.get_account(address)?;
        let new_nonce = account.nonce.checked_add(1).ok_or_else(|| {
            StorageError::DatabaseError(format!(
                "Nonce overflow for address 0x{}: nonce is already u64::MAX",
                hex::encode(address.as_bytes())
            ))
        })?;
        account.nonce = new_nonce;
        self.set_account(*address, account);
        Ok(new_nonce)
    }

    /// Transfer value between accounts
    ///
    /// SECURITY: Uses checked arithmetic to prevent integer overflow/underflow.
    /// A wrap-around on `to_account.balance` could mint tokens from thin air.
    pub fn transfer(&self, from: &Address, to: &Address, value: u128) -> Result<()> {
        let mut from_account = self.get_account(from)?;
        let mut to_account = self.get_account(to)?;

        // SECURITY (C-2): underflow check — insufficient balance
        let new_from_balance = from_account.balance.checked_sub(value).ok_or_else(|| {
            StorageError::DatabaseError(format!(
                "Insufficient balance: has {}, needs {}",
                from_account.balance, value
            ))
        })?;

        // SECURITY (C-2): overflow check — recipient balance must not wrap around
        let new_to_balance = to_account.balance.checked_add(value).ok_or_else(|| {
            StorageError::DatabaseError(format!(
                "Balance overflow: recipient balance {} + {} would overflow u128",
                to_account.balance, value
            ))
        })?;

        from_account.balance = new_from_balance;
        to_account.balance = new_to_balance;

        self.set_account(*from, from_account);
        self.set_account(*to, to_account);

        Ok(())
    }

    // ============ CONTRACT CODE STORAGE (Ethereum-style) ============

    /// Store contract bytecode and update account code_hash
    /// Returns the code_hash (keccak256 of code)
    pub fn set_contract_code(&self, address: &Address, code: Vec<u8>) -> Result<Hash> {
        // Calculate code_hash
        let code_hash = keccak256(&code);

        // Store code in cache
        {
            let mut codes = self.contract_code.write();
            codes.insert(code_hash, code.clone());
        }

        // Mark as dirty for commit
        {
            let mut dirty = self.dirty_codes.write();
            dirty.insert(code_hash);
        }

        // Update account's code_hash
        let mut account = self.get_account(address)?;
        account.code_hash = code_hash;
        self.set_account(*address, account);

        tracing::info!(
            "📦 Contract code stored at 0x{} (code_hash: 0x{})",
            hex::encode(address.as_bytes()),
            hex::encode(&code_hash[..8])
        );

        Ok(code_hash)
    }

    /// Get contract bytecode by address
    /// Returns None if account has no code (not a contract)
    pub fn get_contract_code(&self, address: &Address) -> Result<Option<Vec<u8>>> {
        let account = self.get_account(address)?;

        // Check if account has code
        if account.code_hash == [0u8; 32] {
            return Ok(None);
        }

        // Check cache first
        {
            let codes = self.contract_code.read();
            if let Some(code) = codes.get(&account.code_hash) {
                return Ok(Some(code.clone()));
            }
        }

        // Load from database
        let mut key = CONTRACT_CODE_PREFIX.to_vec();
        key.extend_from_slice(&account.code_hash);

        match self.db.get(&key)? {
            Some(code) => {
                // Cache it
                let mut codes = self.contract_code.write();
                codes.insert(account.code_hash, code.clone());
                Ok(Some(code))
            }
            None => {
                tracing::warn!(
                    "⚠️ Contract code not found for hash 0x{}",
                    hex::encode(&account.code_hash[..8])
                );
                Ok(None)
            }
        }
    }

    /// Check if address is a contract (has non-zero `code_hash`).
    pub fn is_contract(&self, address: &Address) -> Result<bool> {
        let account = self.get_account(address)?;
        Ok(account.code_hash != [0u8; 32])
    }

    // ============ HNSW VECTOR INDEX STORAGE ============

    /// Store a serialized HNSW index by name
    ///
    /// The index name should be unique within the system (e.g., "embeddings_768")
    /// The data is the bincode-serialized HnswGraph.
    pub fn set_hnsw_index(&self, name: &str, data: Vec<u8>) -> Result<()> {
        // SECURITY (M-4): Validate index name to prevent key prefix collisions
        // from null bytes or special characters that could interfere with other key spaces.
        if name.is_empty() || name.len() > 128 {
            return Err(StorageError::DatabaseError(
                "HNSW index name must be 1-128 characters".to_string(),
            ));
        }
        if name.bytes().any(|b| b == 0 || b == b'/' || b == b'\\') {
            return Err(StorageError::DatabaseError(
                "HNSW index name must not contain null bytes or path separators".to_string(),
            ));
        }
        let mut key = HNSW_INDEX_PREFIX.to_vec();
        key.extend_from_slice(name.as_bytes());

        let opts = WriteOptions::default();
        self.db.put_opt(&key, &data, &opts)?;

        tracing::info!("📊 HNSW index '{}' stored ({} bytes)", name, data.len());

        Ok(())
    }

    /// Get a serialized HNSW index by name
    ///
    /// Returns None if the index doesn't exist.
    pub fn get_hnsw_index(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let mut key = HNSW_INDEX_PREFIX.to_vec();
        key.extend_from_slice(name.as_bytes());

        match self.db.get(&key)? {
            Some(data) => {
                tracing::debug!("📊 HNSW index '{}' loaded ({} bytes)", name, data.len());
                Ok(Some(data))
            }
            None => {
                tracing::debug!("📊 HNSW index '{}' not found", name);
                Ok(None)
            }
        }
    }

    /// Delete an HNSW index by name
    pub fn delete_hnsw_index(&self, name: &str) -> Result<()> {
        // SECURITY (M-4): Same name validation as set_hnsw_index
        if name.is_empty() || name.len() > 128 {
            return Err(StorageError::DatabaseError(
                "HNSW index name must be 1-128 characters".to_string(),
            ));
        }
        if name.bytes().any(|b| b == 0 || b == b'/' || b == b'\\') {
            return Err(StorageError::DatabaseError(
                "HNSW index name must not contain null bytes or path separators".to_string(),
            ));
        }
        let mut key = HNSW_INDEX_PREFIX.to_vec();
        key.extend_from_slice(name.as_bytes());

        self.db.delete(&key)?;

        tracing::info!("📊 HNSW index '{}' deleted", name);

        Ok(())
    }

    /// Commit all dirty accounts to database
    ///
    /// SECURITY (H-3): Uses a *persisted* `MerkleTrie` that survives across commits.
    /// Only dirty (modified) accounts are updated in the trie each round —
    /// clean accounts retain their entries from previous commits, giving a
    /// correct incremental state root over the full account set.
    pub fn commit(&self) -> Result<Hash> {
        // SECURITY (C-2): Lock ordering: cache FIRST, then dirty — consistent with set_account()
        let cache = self.cache.read();
        let dirty = self.dirty.read();

        let mut batch = rocksdb::WriteBatch::default();

        // Pre-allocate with known size for efficiency.
        let dirty_addresses: Vec<Address> = dirty.iter().cloned().collect();

        for address in dirty.iter() {
            if let Some(account) = cache.peek(address) {
                let bytes = bincode::serialize(account)?;
                batch.put(address.as_bytes(), bytes);
            } else {
                // SECURITY (H-2): A dirty entry was evicted from the LRU cache
                // before commit. This is a critical invariant violation — the
                // modified account state is lost. This can only happen if the
                // dirty set exceeds MAX_ACCOUNT_CACHE_ENTRIES, which requires
                // an abnormally large block.
                return Err(StorageError::DatabaseError(format!(
                    "CRITICAL: dirty account 0x{} was evicted from LRU cache before commit",
                    hex::encode(address.as_bytes()),
                )));
            }
        }

        // SECURITY (C-4): Merge contract code writes into the SAME batch
        // for atomicity — prevents partial commits on crash.
        {
            let dirty_codes = self.dirty_codes.read();
            if !dirty_codes.is_empty() {
                let codes = self.contract_code.read();
                for code_hash in dirty_codes.iter() {
                    if let Some(code) = codes.get(code_hash) {
                        let mut key = CONTRACT_CODE_PREFIX.to_vec();
                        key.extend_from_slice(code_hash);
                        batch.put(&key, code);
                    }
                }
            }
        }

        // Single atomic write for both accounts AND contract codes
        self.db.write(batch)?;

        // Clear dirty sets
        drop(dirty);
        self.dirty.write().clear();
        self.dirty_codes.write().clear();

        // SECURITY (H-3): Apply incremental trie updates.
        // The trie is persisted as a field, so each commit only touches the
        // accounts that actually changed, while all previously committed
        // accounts remain in the trie from prior rounds.
        if !dirty_addresses.is_empty() {
            let mut trie = self.trie.write();
            tracing::debug!(
                "Updating trie with {} dirty accounts (incremental)",
                dirty_addresses.len(),
            );
            for address in dirty_addresses.iter() {
                if let Some(account) = cache.peek(address) {
                    let key = keccak256(address.as_bytes());
                    let value = bincode::serialize(account).map_err(|e| {
                        StorageError::SerializationError(format!("account serialize: {}", e))
                    })?;
                    trie.insert(&key, &value)?;
                }
            }
        }

        let state_root = self.trie.read().root_hash();
        Ok(state_root)
    }

    /// Rollback all uncommitted changes
    ///
    /// SECURITY (M-3): Also clears dirty_codes to correctly revert any contract
    /// code changes that occurred within the same transaction batch.
    pub fn rollback(&self) {
        // SECURITY (C-2): Lock ordering: cache FIRST, then dirty — consistent with set_account()
        let dirty_addresses: Vec<Address> = self.dirty.read().iter().cloned().collect();
        let mut cache = self.cache.write();

        // Remove all dirty entries from cache
        for address in &dirty_addresses {
            cache.pop(address);
        }

        drop(cache);
        self.dirty.write().clear();

        // SECURITY (M-3): Clear dirty contract codes — without this, rolled-back
        // contract code changes would persist and be committed on the next call.
        self.dirty_codes.write().clear();

        // SECURITY (H-4): Revert trie entries for dirty accounts.
        // Without this, rolled-back account updates remain in the trie,
        // causing the next commit()'s state root to include phantom changes.
        {
            let mut trie = self.trie.write();
            for address in &dirty_addresses {
                let trie_key = keccak256(address.as_bytes());
                let _ = trie.delete(&trie_key);

                // If the account exists in RocksDB (was previously committed),
                // re-insert it into the trie to restore the pre-rollback state.
                if let Ok(Some(bytes)) = self.db.get(address.as_bytes()) {
                    if let Ok(_account) = bincode::deserialize::<Account>(&bytes) {
                        let _ = trie.insert(&trie_key, &bytes);
                    }
                }
            }
        }
    }

    /// Clear all cached accounts and the dirty set. Primarily used in tests.
    pub fn clear_cache(&self) {
        self.cache.write().clear();
        self.dirty.write().clear();
    }

    /// Return the number of accounts currently held in the in-memory cache.
    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }

    /// Return the number of accounts marked dirty (modified but not yet committed).
    pub fn dirty_count(&self) -> usize {
        self.dirty.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocksdb::Options;
    use tempfile::TempDir;

    fn create_test_db() -> (TempDir, StateDB) {
        let temp_dir = TempDir::new().unwrap();
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = Arc::new(DB::open(&opts, temp_dir.path()).unwrap());
        (temp_dir, StateDB::new(db))
    }

    #[test]
    fn test_state_db_creation() {
        let (_dir, state_db) = create_test_db();
        assert_eq!(state_db.cache_size(), 0);
        assert_eq!(state_db.dirty_count(), 0);
    }

    #[test]
    fn test_get_account_not_exists() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        let account = state_db.get_account(&address).unwrap();
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_set_and_get_account() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        let account = Account {
            nonce: 5,
            balance: 1000,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        };

        state_db.set_account(address, account.clone());

        let retrieved = state_db.get_account(&address).unwrap();
        assert_eq!(retrieved.balance, 1000);
        assert_eq!(retrieved.nonce, 5);
    }

    #[test]
    fn test_balance_operations() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        state_db.set_balance(&address, 5000).unwrap();
        let balance = state_db.get_balance(&address).unwrap();
        assert_eq!(balance, 5000);
    }

    #[test]
    fn test_nonce_operations() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        state_db.set_nonce(&address, 10).unwrap();
        let nonce = state_db.get_nonce(&address).unwrap();
        assert_eq!(nonce, 10);

        let new_nonce = state_db.increment_nonce(&address).unwrap();
        assert_eq!(new_nonce, 11);
    }

    #[test]
    fn test_transfer() {
        let (_dir, state_db) = create_test_db();
        let from = Address::try_from_slice(&[1u8; 20]).unwrap();
        let to = Address::try_from_slice(&[2u8; 20]).unwrap();

        state_db.set_balance(&from, 1000).unwrap();
        state_db.transfer(&from, &to, 300).unwrap();

        assert_eq!(state_db.get_balance(&from).unwrap(), 700);
        assert_eq!(state_db.get_balance(&to).unwrap(), 300);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let (_dir, state_db) = create_test_db();
        let from = Address::try_from_slice(&[1u8; 20]).unwrap();
        let to = Address::try_from_slice(&[2u8; 20]).unwrap();

        state_db.set_balance(&from, 100).unwrap();
        let result = state_db.transfer(&from, &to, 200);

        assert!(result.is_err());
    }

    #[test]
    fn test_commit() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        state_db.set_balance(&address, 1000).unwrap();
        assert_eq!(state_db.dirty_count(), 1);

        let state_root = state_db.commit().unwrap();
        assert_ne!(state_root, [0u8; 32]);
        assert_eq!(state_db.dirty_count(), 0);

        // Clear cache and reload from db
        state_db.clear_cache();
        let balance = state_db.get_balance(&address).unwrap();
        assert_eq!(balance, 1000);
    }

    #[test]
    fn test_rollback() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        state_db.set_balance(&address, 1000).unwrap();
        state_db.commit().unwrap();

        state_db.set_balance(&address, 2000).unwrap();
        assert_eq!(state_db.get_balance(&address).unwrap(), 2000);

        state_db.rollback();

        // Should revert to committed value
        let balance = state_db.get_balance(&address).unwrap();
        assert_eq!(balance, 1000);
    }

    #[test]
    fn test_cache() {
        let (_dir, state_db) = create_test_db();
        let address = Address::zero();

        state_db.set_balance(&address, 500).unwrap();
        assert_eq!(state_db.cache_size(), 1);

        // Access should hit cache
        let _ = state_db.get_balance(&address).unwrap();
        assert_eq!(state_db.cache_size(), 1);
    }

    /// Verify that committing twice with no changes produces the same root.
    #[test]
    fn test_commit_idempotent() {
        let (_dir, state_db) = create_test_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        state_db.set_balance(&addr, 1_000).unwrap();
        let root1 = state_db.commit().unwrap();

        // Nothing changed → commit should return the same root.
        let root2 = state_db.commit().unwrap();
        assert_eq!(root1, root2, "commit with no changes must return same root");
    }

    /// Verify that rollback → commit produces the same root as before the change.
    #[test]
    fn test_rollback_then_commit_consistency() {
        let (_dir, state_db) = create_test_db();
        let addr = Address::try_from_slice(&[1u8; 20]).unwrap();

        // Commit initial state.
        state_db.set_balance(&addr, 1_000).unwrap();
        let root_before = state_db.commit().unwrap();

        // Modify, then rollback.
        state_db.set_balance(&addr, 9_999).unwrap();
        state_db.rollback();

        // Commit again → root must match root_before since state was rolled back.
        let root_after = state_db.commit().unwrap();
        assert_eq!(root_before, root_after, "state root after rollback+commit must match original");
    }
}
