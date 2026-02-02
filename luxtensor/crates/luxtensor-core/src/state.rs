use std::collections::HashMap;
use crate::{Account, Address, Hash, Result};

/// State database interface
pub struct StateDB {
    cache: HashMap<Address, Account>,
    pub vector_store: crate::semantic::SimpleVectorStore,
}

impl StateDB {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            // Default dimension 768 (common for BERT/LLM embeddings)
            vector_store: crate::semantic::SimpleVectorStore::new(768),
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

        // 2. Calculate Vector Root
        use crate::semantic::VectorStore;
        let vector_root = self.vector_store.root_hash()?;

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
