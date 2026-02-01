use crate::{Result, StorageError};
use luxtensor_core::{Account, Address};
use luxtensor_crypto::{keccak256, Hash};
use parking_lot::RwLock;
use rocksdb::DB;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// State database with RocksDB backend and LRU cache
pub struct StateDB {
    db: Arc<DB>,
    cache: RwLock<HashMap<Address, Account>>,
    dirty: RwLock<HashSet<Address>>,
}

impl StateDB {
    /// Create a new state database
    pub fn new(db: Arc<DB>) -> Self {
        Self {
            db,
            cache: RwLock::new(HashMap::new()),
            dirty: RwLock::new(HashSet::new()),
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

    /// Commit all dirty accounts to database
    pub fn commit(&self) -> Result<Hash> {
        let dirty = self.dirty.read();
        let cache = self.cache.read();

        let mut batch = rocksdb::WriteBatch::default();
        let mut account_hashes = Vec::new();

        for address in dirty.iter() {
            if let Some(account) = cache.get(address) {
                let bytes = bincode::serialize(account)?;
                batch.put(address.as_bytes(), bytes);

                // Collect for state root calculation
                let mut data = Vec::new();
                data.extend_from_slice(address.as_bytes());
                data.extend_from_slice(&bincode::serialize(account)?);
                account_hashes.push(keccak256(&data));
            }
        }

        self.db.write(batch)?;

        // Clear dirty set
        drop(dirty);
        self.dirty.write().clear();

        // Calculate state root (simplified - just hash all account hashes)
        let state_root = if account_hashes.is_empty() {
            [0u8; 32]
        } else {
            account_hashes.sort();
            let mut data = Vec::new();
            for hash in account_hashes {
                data.extend_from_slice(&hash);
            }
            keccak256(&data)
        };

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
        let from = Address::from_slice(&[1u8; 20]);
        let to = Address::from_slice(&[2u8; 20]);

        state_db.set_balance(&from, 1000).unwrap();
        state_db.transfer(&from, &to, 300).unwrap();

        assert_eq!(state_db.get_balance(&from).unwrap(), 700);
        assert_eq!(state_db.get_balance(&to).unwrap(), 300);
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let (_dir, state_db) = create_test_db();
        let from = Address::from_slice(&[1u8; 20]);
        let to = Address::from_slice(&[2u8; 20]);

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
