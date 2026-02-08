use std::collections::HashMap;
use crate::{Account, Address, Hash, Result};
use crate::hnsw::HnswVectorStore;

/// Key prefix for state accounts in RocksDB
const STATE_ACCOUNT_PREFIX: &[u8] = b"state:account:";

/// State database interface
/// Provides an in-memory cache with optional RocksDB persistence.
pub struct StateDB {
    cache: HashMap<Address, Account>,
    /// HNSW-backed vector store for Semantic Layer
    /// Provides O(log N) approximate nearest neighbor search
    pub vector_store: HnswVectorStore,
}

impl StateDB {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            // Default dimension 768 (common for BERT/LLM embeddings)
            vector_store: HnswVectorStore::new(768),
        }
    }

    /// Get account from state
    pub fn get_account(&self, address: &Address) -> Option<Account> {
        self.cache.get(address).cloned()
    }

    /// Set account in state
    pub fn set_account(&mut self, address: Address, account: Account) {
        self.cache.insert(address, account);
    }

    /// Get account balance
    pub fn get_balance(&self, address: &Address) -> u128 {
        self.get_account(address)
            .map(|acc| acc.balance)
            .unwrap_or(0)
    }

    /// Get account nonce
    pub fn get_nonce(&self, address: &Address) -> u64 {
        self.get_account(address)
            .map(|acc| acc.nonce)
            .unwrap_or(0)
    }

    /// Get contract bytecode from account
    pub fn get_code(&self, address: &Address) -> Option<Vec<u8>> {
        self.get_account(address)
            .and_then(|acc| acc.code.clone())
    }

    /// Calculate state root using Merkle Tree (Hybrid: Account Tree + Vector Tree)
    /// Root = Keccak256(AccountRoot || VectorRoot)
    pub fn root_hash(&self) -> Result<Hash> {
        // 1. Calculate Account Root
        let account_root = if self.cache.is_empty() {
            [0u8; 32]
        } else {
            let mut items: Vec<_> = self.cache.iter().collect();
            items.sort_by(|a, b| a.0.cmp(b.0));

            let mut leaf_hashes: Vec<[u8; 32]> = Vec::with_capacity(items.len());
            for (address, account) in items.iter() {
                let mut data = Vec::new();
                data.extend_from_slice(address.as_bytes());
                let account_bytes = bincode::serialize(account)
                    .map_err(|e| crate::CoreError::SerializationError(e.to_string()))?;
                data.extend_from_slice(&account_bytes);
                leaf_hashes.push(luxtensor_crypto::keccak256(&data));
            }
            luxtensor_crypto::MerkleTree::new(leaf_hashes).root()
        };

        // 2. Calculate Vector Root (using HNSW's deterministic hash)
        let vector_root = self.vector_store.root_hash();

        // 3. Combine Roots
        let mut combined = Vec::new();
        combined.extend_from_slice(&account_root);
        combined.extend_from_slice(&vector_root);

        Ok(luxtensor_crypto::keccak256(&combined))
    }

    /// Commit changes and return state root
    pub fn commit(&mut self) -> Result<Hash> {
        self.root_hash()
    }

    /// Flush all cached accounts to a RocksDB instance for persistence.
    /// Call this after `commit()` to durably persist state across restarts.
    ///
    /// Key format: `state:account:<20-byte address>` → bincode-serialized Account
    pub fn flush_to_db(&self, db: &impl RocksDbLike) -> Result<usize> {
        let mut count = 0usize;
        for (address, account) in &self.cache {
            let mut key = STATE_ACCOUNT_PREFIX.to_vec();
            key.extend_from_slice(address.as_bytes());
            let value = bincode::serialize(account)
                .map_err(|e| crate::CoreError::SerializationError(e.to_string()))?;
            db.put(&key, &value)
                .map_err(|e| crate::CoreError::SerializationError(format!("RocksDB put failed: {}", e)))?;
            count += 1;
        }
        Ok(count)
    }

    /// Load accounts from a RocksDB instance into the in-memory cache.
    /// Call this on startup to restore state from disk.
    ///
    /// Scans all keys with prefix `state:account:` and deserializes.
    pub fn load_from_db(&mut self, db: &impl RocksDbLike) -> Result<usize> {
        let mut count = 0usize;
        let entries = db.prefix_scan(STATE_ACCOUNT_PREFIX)
            .map_err(|e| crate::CoreError::SerializationError(format!("RocksDB scan failed: {}", e)))?;

        for (key, value) in entries {
            if key.len() == STATE_ACCOUNT_PREFIX.len() + 20 {
                let mut addr_bytes = [0u8; 20];
                addr_bytes.copy_from_slice(&key[STATE_ACCOUNT_PREFIX.len()..]);
                let address = Address::from(addr_bytes);
                let account: Account = bincode::deserialize(&value)
                    .map_err(|e| crate::CoreError::SerializationError(e.to_string()))?;
                self.cache.insert(address, account);
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get the number of accounts in cache
    pub fn account_count(&self) -> usize {
        self.cache.len()
    }

    /// Iterate over all cached accounts (address, account) pairs.
    /// Used by block production to sync state into UnifiedStateDB for RPC.
    pub fn accounts(&self) -> impl Iterator<Item = (&Address, &Account)> {
        self.cache.iter()
    }
}

/// Trait abstracting RocksDB-like key-value store operations.
/// Implemented by wrappers around `rocksdb::DB` in the storage crate.
pub trait RocksDbLike {
    fn put(&self, key: &[u8], value: &[u8]) -> std::result::Result<(), String>;
    fn get(&self, key: &[u8]) -> std::result::Result<Option<Vec<u8>>, String>;
    fn prefix_scan(&self, prefix: &[u8]) -> std::result::Result<Vec<(Vec<u8>, Vec<u8>)>, String>;
}

impl Default for StateDB {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_db_creation() {
        let state = StateDB::new();
        let addr = Address::zero();
        assert_eq!(state.get_balance(&addr), 0);
    }

    #[test]
    fn test_state_db_set_account() {
        let mut state = StateDB::new();
        let addr = Address::zero();
        let account = Account::with_balance(1000);

        state.set_account(addr, account);
        assert_eq!(state.get_balance(&addr), 1000);
    }
}
