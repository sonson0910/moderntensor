use crate::{Result, StorageError};
use crate::trie::MerkleTrie;
use luxtensor_core::{Account, Address};
use luxtensor_crypto::{keccak256, Hash};
use parking_lot::RwLock;
use rocksdb::{DB, WriteOptions};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Key prefix for contract code storage
const CONTRACT_CODE_PREFIX: &[u8] = b"code:";

/// Key prefix for HNSW vector index storage
const HNSW_INDEX_PREFIX: &[u8] = b"hnsw:";

/// State database with RocksDB backend and LRU cache
pub struct StateDB {
    db: Arc<DB>,
    cache: RwLock<HashMap<Address, Account>>,
    dirty: RwLock<HashSet<Address>>,
    /// Contract bytecode storage: code_hash -> bytecode
    contract_code: RwLock<HashMap<Hash, Vec<u8>>>,
    /// Dirty contract codes awaiting commit
    dirty_codes: RwLock<HashSet<Hash>>,
}

impl StateDB {
    /// Create a new state database
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            db,
            cache: RwLock::new(HashMap::new()),
            dirty: RwLock::new(HashSet::new()),
            contract_code: RwLock::new(HashMap::new()),
            dirty_codes: RwLock::new(HashSet::new()),
        }
    }

    /// Get an account by address
    pub fn get_account(&self, address: &Address) -> Result<Account> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(account) = cache.get(address) {
                return Ok(account.clone());
            }
        }

        // Load from database
        match self.db.get(address.as_bytes())? {
            Some(bytes) => {
                let account: Account = bincode::deserialize(&bytes)?;

                // Update cache
                let mut cache = self.cache.write();
                cache.insert(*address, account.clone());

                Ok(account)
            }
            None => {
                // Return empty account if not found
                Ok(Account::default())
            }
        }
    }

    /// Set an account
    pub fn set_account(&self, address: Address, account: Account) {
        let mut cache = self.cache.write();
        cache.insert(address, account);

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
    pub fn increment_nonce(&self, address: &Address) -> Result<u64> {
        let mut account = self.get_account(address)?;
        account.nonce += 1;
        let new_nonce = account.nonce;
        self.set_account(*address, account);
        Ok(new_nonce)
    }

    /// Transfer value between accounts
    pub fn transfer(&self, from: &Address, to: &Address, value: u128) -> Result<()> {
        let mut from_account = self.get_account(from)?;
        let mut to_account = self.get_account(to)?;

        if from_account.balance < value {
            return Err(StorageError::DatabaseError("Insufficient balance".to_string()));
        }

        from_account.balance -= value;
        to_account.balance += value;

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

        tracing::info!("📦 Contract code stored at 0x{} (code_hash: 0x{})",
            hex::encode(address.as_bytes()), hex::encode(&code_hash[..8]));

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
                tracing::warn!("⚠️ Contract code not found for hash 0x{}",
                    hex::encode(&account.code_hash[..8]));
                Ok(None)
            }
        }
    }

    /// Check if address is a contract (has code)
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
        let mut key = HNSW_INDEX_PREFIX.to_vec();
        key.extend_from_slice(name.as_bytes());

        let opts = WriteOptions::default();
        self.db.put_opt(&key, &data, &opts)?;

        tracing::info!(
            "📊 HNSW index '{}' stored ({} bytes)",
            name,
            data.len()
        );

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
        let mut key = HNSW_INDEX_PREFIX.to_vec();
        key.extend_from_slice(name.as_bytes());

        self.db.delete(&key)?;

        tracing::info!("📊 HNSW index '{}' deleted", name);

        Ok(())
    }

    /// Commit all dirty accounts to database
    pub fn commit(&self) -> Result<Hash> {
        let dirty = self.dirty.read();
        let cache = self.cache.read();

        let mut batch = rocksdb::WriteBatch::default();

        let mut trie = MerkleTrie::new();

        // SECURITY FIX: Capture dirty addresses before clearing, for trie update below.
        // We only insert dirty (modified) accounts into the trie — previously this
        // iterated ALL cached accounts, but the cache is a partial view containing
        // only recently-accessed accounts, not the full state. Building a trie from
        // a partial cache produces an incorrect state root.
        let dirty_addresses: Vec<Address> = dirty.iter().cloned().collect();

        for address in dirty.iter() {
            if let Some(account) = cache.get(address) {
                let bytes = bincode::serialize(account)?;
                batch.put(address.as_bytes(), bytes);
            }
        }

        self.db.write(batch)?;

        // Clear dirty set
        drop(dirty);
        self.dirty.write().clear();

        // Flush dirty contract codes to RocksDB
        {
            let dirty_codes = self.dirty_codes.read();
            if !dirty_codes.is_empty() {
                let codes = self.contract_code.read();
                let mut code_batch = rocksdb::WriteBatch::default();
                for code_hash in dirty_codes.iter() {
                    if let Some(code) = codes.get(code_hash) {
                        let mut key = CONTRACT_CODE_PREFIX.to_vec();
                        key.extend_from_slice(code_hash);
                        code_batch.put(&key, code);
                    }
                }
                drop(codes);
                drop(dirty_codes);
                self.db.write(code_batch)?;
                self.dirty_codes.write().clear();
            }
        }

        // SECURITY FIX: Only insert dirty (modified) accounts into the trie.
        // Clean (unchanged) accounts retain their previous trie entries from
        // the prior state root. For a fully correct incremental state root,
        // the MerkleTrie should be persisted across commits rather than
        // recreated each time.
        if dirty_addresses.is_empty() {
            tracing::debug!("No dirty accounts — state root unchanged");
        } else {
            tracing::warn!(
                "⚠️ Building trie from {} dirty accounts (cache has {} total). \
                 For production correctness, persist trie across commits for incremental updates.",
                dirty_addresses.len(),
                cache.len()
            );
        }
        for address in dirty_addresses.iter() {
            if let Some(account) = cache.get(address) {
                let key = keccak256(address.as_bytes());
                let value = bincode::serialize(account)
                    .map_err(|e| StorageError::SerializationError(format!("account serialize: {}", e)))?;
                trie.insert(&key, &value)?;
            }
        }

        let state_root = trie.root_hash();

        Ok(state_root)
    }

    /// Rollback all uncommitted changes
    pub fn rollback(&self) {
        let dirty = self.dirty.read();
        let mut cache = self.cache.write();

        // Remove all dirty entries from cache
        for address in dirty.iter() {
            cache.remove(address);
        }

        drop(dirty);
        self.dirty.write().clear();
    }

    /// Clear cache (useful for testing)
    pub fn clear_cache(&self) {
        self.cache.write().clear();
        self.dirty.write().clear();
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }

    /// Get number of dirty accounts
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
}
