use serde::{Deserialize, Serialize};
use crate::Hash;

/// Account state
/// Represents both EOA (Externally Owned Account) and Contract accounts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub nonce: u64,
    pub balance: u128,
    pub storage_root: Hash,
    pub code_hash: Hash,
    /// Contract bytecode (None for EOA, Some for contracts)
    #[serde(default)]
    pub code: Option<Vec<u8>>,
}

/// Error when balance operation fails
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BalanceError {
    /// Insufficient balance for subtraction
    InsufficientBalance { have: u128, need: u128 },
    /// Overflow would occur
    Overflow,
}

impl std::fmt::Display for BalanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BalanceError::InsufficientBalance { have, need } => {
                write!(f, "Insufficient balance: have {}, need {}", have, need)
            }
            BalanceError::Overflow => write!(f, "Balance overflow"),
        }
    }
}

impl std::error::Error for BalanceError {}

impl Account {
    /// Create a new empty EOA (Externally Owned Account)
    pub fn new() -> Self {
        Self {
            nonce: 0,
            balance: 0,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        }
    }

    /// Create a new EOA with initial balance
    pub fn with_balance(balance: u128) -> Self {
        Self {
            nonce: 0,
            balance,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
            code: None,
        }
    }

    /// Create a contract account with bytecode
    ///
    /// # Arguments
    /// * `balance` - Initial balance for the contract
    /// * `code` - Contract bytecode
    /// * `code_hash` - Pre-computed keccak256 hash of the code
    pub fn contract(balance: u128, code: Vec<u8>, code_hash: Hash) -> Self {
        Self {
            nonce: 1, // Contracts start with nonce 1 per EIP-161
            balance,
            storage_root: [0u8; 32],
            code_hash,
            code: Some(code),
        }
    }

    /// Check if this is a contract account
    /// Returns true if the account has bytecode
    pub fn is_contract(&self) -> bool {
        self.code.is_some() && !self.code.as_ref().map_or(true, |c| c.is_empty())
    }

    /// Get the contract code if this is a contract account
    pub fn get_code(&self) -> Option<&[u8]> {
        self.code.as_deref()
    }

    /// Set contract code (used during deployment)
    pub fn set_code(&mut self, code: Vec<u8>, code_hash: Hash) {
        self.code = Some(code);
        self.code_hash = code_hash;
    }

    /// Add to balance with overflow protection
    /// Returns error if overflow would occur
    pub fn checked_add_balance(&mut self, amount: u128) -> Result<(), BalanceError> {
        self.balance = self.balance
            .checked_add(amount)
            .ok_or(BalanceError::Overflow)?;
        Ok(())
    }

    /// Subtract from balance with underflow protection
    /// Returns error if insufficient balance
    pub fn checked_sub_balance(&mut self, amount: u128) -> Result<(), BalanceError> {
        if self.balance < amount {
            return Err(BalanceError::InsufficientBalance {
                have: self.balance,
                need: amount,
            });
        }
        self.balance -= amount;
        Ok(())
    }

    /// Add to balance with saturating arithmetic (caps at u128::MAX)
    pub fn saturating_add_balance(&mut self, amount: u128) {
        self.balance = self.balance.saturating_add(amount);
    }

    /// Subtract from balance with saturating arithmetic (floors at 0)
    pub fn saturating_sub_balance(&mut self, amount: u128) {
        self.balance = self.balance.saturating_sub(amount);
    }

    /// Increment nonce with overflow protection
    pub fn increment_nonce(&mut self) -> Result<(), BalanceError> {
        self.nonce = self.nonce
            .checked_add(1)
            .ok_or(BalanceError::Overflow)?;
        Ok(())
    }

    /// Check if account can afford a transfer
    pub fn can_afford(&self, amount: u128, gas_cost: u128) -> bool {
        self.balance >= amount.saturating_add(gas_cost)
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
