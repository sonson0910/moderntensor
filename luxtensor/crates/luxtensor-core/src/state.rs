use std::collections::HashMap;
use crate::{Account, Address, Hash, Result};

/// State database interface
pub struct StateDB {
    cache: HashMap<Address, Account>,
}

impl StateDB {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
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

    /// Calculate state root using Merkle Tree
    /// Each leaf is hash(address || serialized_account)
    /// Returns Result to handle serialization errors gracefully
    pub fn root_hash(&self) -> Result<Hash> {
        if self.cache.is_empty() {
            return Ok([0u8; 32]); // Empty state root
        }

        // Collect all accounts and sort by address for deterministic ordering
        let mut items: Vec<_> = self.cache.iter().collect();
        items.sort_by(|a, b| a.0.cmp(b.0));

        // Create leaf hashes: hash(address || account_data)
        let mut leaf_hashes: Vec<[u8; 32]> = Vec::with_capacity(items.len());
        for (address, account) in items.iter() {
            let mut data = Vec::new();
            data.extend_from_slice(address.as_bytes());
            // Serialize account - use ? for proper error handling
            let account_bytes = bincode::serialize(account)
                .map_err(|e| crate::CoreError::SerializationError(e.to_string()))?;
            data.extend_from_slice(&account_bytes);
            leaf_hashes.push(luxtensor_crypto::keccak256(&data));
        }

        // Build Merkle tree and return root
        let tree = luxtensor_crypto::MerkleTree::new(leaf_hashes);
        Ok(tree.root())
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
