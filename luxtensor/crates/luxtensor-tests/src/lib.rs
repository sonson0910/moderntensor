// LuxTensor Tests Library
// This crate contains integration tests and benchmarks for LuxTensor

pub mod test_utils {
    use luxtensor_crypto::KeyPair;
    use luxtensor_core::{Account, Address};
    
    /// Generate a test account with a balance
    pub fn create_test_account(balance: u128, nonce: u64) -> (Address, Account) {
        let keypair = KeyPair::generate();
        let address = Address::from(keypair.address());
        
        let account = Account {
            nonce,
            balance,
            storage_root: [0u8; 32],
            code_hash: [0u8; 32],
        };
        
        (address, account)
    }
    
    /// Create multiple test accounts
    pub fn create_test_accounts(count: usize, balance_per_account: u128) -> Vec<(Address, Account)> {
        (0..count)
            .map(|i| create_test_account(balance_per_account, i as u64))
            .collect()
    }
}
