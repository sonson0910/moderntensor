// Test utilities for LuxTensor tests
// Provides helper functions for creating test accounts, transactions, and blocks

use luxtensor_crypto::KeyPair;
use luxtensor_core::{Account, Address, Transaction, Block, BlockHeader};
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a test account with a balance
pub fn create_test_account(balance: u128, nonce: u64) -> (Address, Account, KeyPair) {
    let keypair = KeyPair::generate();
    let address = Address::from(keypair.address());

    let account = Account {
        nonce,
        balance,
        storage_root: [0u8; 32],
        code_hash: [0u8; 32],
        code: None,
    };

    (address, account, keypair)
}

/// Create multiple test accounts
pub fn create_test_accounts(count: usize, balance_per_account: u128) -> Vec<(Address, Account, KeyPair)> {
    (0..count)
        .map(|i| create_test_account(balance_per_account, i as u64))
        .collect()
}

/// Create a test transaction
pub fn create_test_transaction(
    from: Address,
    to: Option<Address>,
    value: u128,
    nonce: u64,
) -> Transaction {
    Transaction::new(
        nonce,
        from,
        to,
        value,
        1, // gas_price
        21000, // gas_limit
        vec![], // data
    )
}

/// Create a test block
pub fn create_test_block(
    height: u64,
    previous_hash: [u8; 32],
    state_root: [u8; 32],
    transactions: Vec<Transaction>,
) -> Block {
    let header = BlockHeader {
        version: 1,
        height,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        previous_hash,
        state_root,
        txs_root: [0u8; 32],
        receipts_root: [0u8; 32],
        validator: [0u8; 32],
        signature: vec![0u8; 64],
        gas_used: 0,
        gas_limit: 10_000_000,
        extra_data: vec![],
    };

    Block::new(header, transactions)
}

/// Predefined test addresses (matching Hardhat defaults)
pub mod test_addresses {
    pub const ADDR_1: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
    pub const ADDR_2: &str = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";
    pub const ADDR_3: &str = "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC";
}

/// Wait for condition with timeout
pub async fn wait_for<F, Fut>(timeout_secs: u64, interval_ms: u64, mut condition: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);
    let interval = std::time::Duration::from_millis(interval_ms);

    while start.elapsed() < timeout {
        if condition().await {
            return true;
        }
        tokio::time::sleep(interval).await;
    }

    false
}

/// Sleep helper
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

/// Sleep helper (blocking)
pub fn sleep_ms_blocking(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
