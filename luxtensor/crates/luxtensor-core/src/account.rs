use serde::{Deserialize, Serialize};
use crate::Hash;

/// Account state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub nonce: u64,
    pub balance: u128,
    pub storage_root: Hash,
    pub code_hash: Hash,
}

impl Account {
    pub fn new() -> Self {
        Self {
            nonce: 0,
            balance: 0,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
        }
    }
    
    pub fn with_balance(balance: u128) -> Self {
        Self {
            nonce: 0,
            balance,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
        }
    }
}

impl Default for Account {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_account_creation() {
        let account = Account::new();
        assert_eq!(account.nonce, 0);
        assert_eq!(account.balance, 0);
    }
    
    #[test]
    fn test_account_with_balance() {
        let account = Account::with_balance(1000);
        assert_eq!(account.balance, 1000);
    }
}
