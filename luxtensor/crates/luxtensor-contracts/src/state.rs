use crate::types::ContractAddress;
use luxtensor_core::types::Hash;
use std::collections::HashMap;

/// Contract state storage
#[derive(Debug, Clone)]
pub struct ContractState {
    /// Storage mapping: contract -> key -> value
    storage: HashMap<ContractAddress, HashMap<Hash, Hash>>,
}

impl ContractState {
    /// Create a new contract state
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    /// Get a storage value
    pub fn get_storage(&self, contract: &ContractAddress, key: &Hash) -> Option<Hash> {
        self.storage
            .get(contract)
            .and_then(|contract_storage| contract_storage.get(key))
            .copied()
    }

    /// Set a storage value
    pub fn set_storage(&mut self, contract: &ContractAddress, key: Hash, value: Hash) {
        self.storage
            .entry(*contract)
            .or_insert_with(HashMap::new)
            .insert(key, value);
    }

    /// Clear all storage for a contract
    pub fn clear_contract_storage(&mut self, contract: &ContractAddress) {
        self.storage.remove(contract);
    }

    /// Get all storage for a contract
    pub fn get_contract_storage(&self, contract: &ContractAddress) -> Option<&HashMap<Hash, Hash>> {
        self.storage.get(contract)
    }

    /// Get total number of storage entries
    pub fn storage_size(&self) -> usize {
        self.storage.values().map(|s| s.len()).sum()
    }
}

impl Default for ContractState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_contract() -> ContractAddress {
        ContractAddress([1u8; 20])
    }

    #[test]
    fn test_state_creation() {
        let state = ContractState::new();
        assert_eq!(state.storage_size(), 0);
    }

    #[test]
    fn test_set_and_get_storage() {
        let mut state = ContractState::new();
        let contract = create_test_contract();
        let key = [1u8; 32];
        let value = [2u8; 32];

        state.set_storage(&contract, key, value);

        let retrieved = state.get_storage(&contract, &key);
        assert_eq!(retrieved, Some(value));
    }

    #[test]
    fn test_storage_not_found() {
        let state = ContractState::new();
        let contract = create_test_contract();
        let key = [1u8; 32];

        let retrieved = state.get_storage(&contract, &key);
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_clear_contract_storage() {
        let mut state = ContractState::new();
        let contract = create_test_contract();
        let key = [1u8; 32];
        let value = [2u8; 32];

        state.set_storage(&contract, key, value);
        assert_eq!(state.storage_size(), 1);

        state.clear_contract_storage(&contract);
        assert_eq!(state.storage_size(), 0);
    }

    #[test]
    fn test_multiple_contracts() {
        let mut state = ContractState::new();
        let contract1 = ContractAddress([1u8; 20]);
        let contract2 = ContractAddress([2u8; 20]);
        let key = [1u8; 32];
        let value1 = [10u8; 32];
        let value2 = [20u8; 32];

        state.set_storage(&contract1, key, value1);
        state.set_storage(&contract2, key, value2);

        assert_eq!(state.get_storage(&contract1, &key), Some(value1));
        assert_eq!(state.get_storage(&contract2, &key), Some(value2));
        assert_eq!(state.storage_size(), 2);
    }
}
