// Integration tests for LuxTensor blockchain
// These tests verify that all components work together correctly

use luxtensor_core::{Account, Address, Block, Transaction};
use luxtensor_crypto::{keccak256, KeyPair};
use luxtensor_storage::{BlockchainDB, StateDB};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_full_transaction_flow() {
    // Setup: Create temporary database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_db");

    // Initialize storage
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
    let state_db = Arc::new(StateDB::new(storage.inner_db()));

    // Create genesis block
    let genesis = Block::genesis();
    storage.store_block(&genesis).unwrap();

    // Step 1: Create keypairs for sender and receiver
    let sender_keypair = KeyPair::generate();
    let sender_address = Address::from(sender_keypair.address());

    let receiver_keypair = KeyPair::generate();
    let receiver_address = Address::from(receiver_keypair.address());

    // Step 2: Initialize sender account with balance
    let sender_account = Account {
        nonce: 0,
        balance: 1_000_000_000_000_000_000, // 1 token
        storage_root: [0u8; 32],
        code_hash: [0u8; 32],
        code: None,
    };
    state_db.set_account(sender_address, sender_account);

    // Initialize receiver account
    let receiver_account = Account {
        nonce: 0,
        balance: 0,
        storage_root: [0u8; 32],
        code_hash: [0u8; 32],
        code: None,
    };
    state_db.set_account(receiver_address, receiver_account);

    // Commit initial state
    let state_root = state_db.commit().unwrap();

    // Step 3: Create transaction
    let transfer_amount = 100_000_000_000_000_000; // 0.1 token
    let tx = Transaction::new(
        0, // nonce
        sender_address,
        Some(receiver_address),
        transfer_amount,
        1, // gas_price
        21000, // gas_limit
        vec![], // data
    );

    // Step 4: Execute transaction (simulate)
    state_db.transfer(&sender_address, &receiver_address, transfer_amount).unwrap();

    // Step 5: Verify balances
    let sender_balance = state_db.get_balance(&sender_address).unwrap();
    let receiver_balance = state_db.get_balance(&receiver_address).unwrap();

    assert_eq!(sender_balance, 900_000_000_000_000_000); // 0.9 token
    assert_eq!(receiver_balance, 100_000_000_000_000_000); // 0.1 token

    // Step 6: Create block with transaction
    let transactions = vec![tx];
    let new_state_root = state_db.commit().unwrap();

    let header = luxtensor_core::BlockHeader {
        version: 1,
        height: 1,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        previous_hash: genesis.hash(),
        state_root: new_state_root,
        txs_root: [0u8; 32],
        receipts_root: [0u8; 32],
        validator: [0u8; 32],
        signature: vec![0u8; 64],
        gas_used: 21000,
        gas_limit: 10_000_000,
        extra_data: vec![],
    };

    let block = Block::new(header, transactions);

    // Step 7: Store block
    storage.store_block(&block).unwrap();

    // Step 8: Verify block was stored
    let stored_block = storage.get_block(&block.hash()).unwrap();
    assert!(stored_block.is_some());

    let retrieved_block = stored_block.unwrap();
    assert_eq!(retrieved_block.height(), 1);
    assert_eq!(retrieved_block.transactions.len(), 1);
}

#[tokio::test]
async fn test_block_chain_continuity() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_db");
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
    let state_db = Arc::new(StateDB::new(storage.inner_db()));

    // Create and store genesis block
    let genesis = Block::genesis();
    storage.store_block(&genesis).unwrap();

    // Create a chain of blocks
    let mut previous_hash = genesis.hash();
    let num_blocks = 10;

    for i in 1..=num_blocks {
        let state_root = state_db.commit().unwrap();

        let header = luxtensor_core::BlockHeader {
            version: 1,
            height: i,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
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

        let block = Block::new(header, vec![]);
        previous_hash = block.hash();

        storage.store_block(&block).unwrap();
    }

    // Verify all blocks are accessible
    for i in 0..=num_blocks {
        let block = storage.get_block_by_height(i).unwrap();
        assert!(block.is_some());
        assert_eq!(block.unwrap().height(), i);
    }

    // Verify chain continuity
    let block_10 = storage.get_block_by_height(10).unwrap().unwrap();
    let block_9 = storage.get_block_by_height(9).unwrap().unwrap();
    assert_eq!(block_10.header.previous_hash, block_9.hash());
}

#[tokio::test]
async fn test_state_persistence() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_db");

    let num_accounts = 100;
    let mut addresses = Vec::new();

    // Create and persist accounts
    {
        let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
        let state_db = Arc::new(StateDB::new(storage.inner_db()));

        for i in 0..num_accounts {
            let keypair = KeyPair::generate();
            let address = Address::from(keypair.address());
            addresses.push(address);

            let account = Account {
                nonce: i,
                balance: (i as u128 + 1) * 1_000_000_000_000_000_000,
                storage_root: [0u8; 32],
                code_hash: [0u8; 32],
                code: None,
            };

            state_db.set_account(address, account);
        }

        state_db.commit().unwrap();
    }

    // Re-open database and verify persistence
    {
        let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
        let state_db = Arc::new(StateDB::new(storage.inner_db()));

        for (i, address) in addresses.iter().enumerate() {
            let account = state_db.get_account(address).unwrap();
            assert_eq!(account.nonce, i as u64);
            assert_eq!(account.balance, (i as u128 + 1) * 1_000_000_000_000_000_000);
        }
    }
}

#[tokio::test]
async fn test_concurrent_state_access() {
    use tokio::task::JoinSet;

    // Setup
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_db");
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
    let state_db = Arc::new(StateDB::new(storage.inner_db()));

    // Create initial account
    let keypair = KeyPair::generate();
    let address = Address::from(keypair.address());

    let account = Account {
        nonce: 0,
        balance: 1_000_000_000_000_000_000,
        storage_root: [0u8; 32],
        code_hash: [0u8; 32],
        code: None,
    };
    state_db.set_account(address, account);
    state_db.commit().unwrap();

    // Perform concurrent reads
    let mut tasks = JoinSet::new();

    for _ in 0..10 {
        let state_db_clone = state_db.clone();
        let addr = address;

        tasks.spawn(async move {
            let balance = state_db_clone.get_balance(&addr).unwrap();
            assert_eq!(balance, 1_000_000_000_000_000_000);
        });
    }

    // Wait for all tasks to complete
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
}

#[tokio::test]
async fn test_transaction_nonce_validation() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_db");
    let storage = Arc::new(BlockchainDB::open(&db_path).unwrap());
    let state_db = Arc::new(StateDB::new(storage.inner_db()));

    // Create account
    let keypair = KeyPair::generate();
    let address = Address::from(keypair.address());

    let account = Account {
        nonce: 5,
        balance: 1_000_000_000_000_000_000,
        storage_root: [0u8; 32],
        code_hash: [0u8; 32],
        code: None,
    };
    state_db.set_account(address, account);

    // Get and verify nonce
    let nonce = state_db.get_nonce(&address).unwrap();
    assert_eq!(nonce, 5);

    // Increment nonce
    let new_nonce = state_db.increment_nonce(&address).unwrap();
    assert_eq!(new_nonce, 6);

    // Verify increment persisted
    let current_nonce = state_db.get_nonce(&address).unwrap();
    assert_eq!(current_nonce, 6);
}

#[test]
fn test_block_hash_consistency() {
    // Create the same block twice and verify hashes match
    let genesis1 = Block::genesis();
    let genesis2 = Block::genesis();

    assert_eq!(genesis1.hash(), genesis2.hash());

    // Verify hash is deterministic
    let hash1 = genesis1.hash();
    let hash2 = genesis1.hash();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_transaction_hash_consistency() {
    let keypair = KeyPair::generate();
    let address = Address::from(keypair.address());

    let tx1 = Transaction::new(
        0,
        address,
        Some(address),
        1000,
        1,
        21000,
        vec![],
    );

    let tx2 = Transaction::new(
        0,
        address,
        Some(address),
        1000,
        1,
        21000,
        vec![],
    );

    // Same transaction data should produce same hash
    assert_eq!(tx1.hash(), tx2.hash());
}
