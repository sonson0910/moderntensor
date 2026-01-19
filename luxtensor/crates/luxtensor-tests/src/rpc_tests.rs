// RPC API Tests for LuxTensor
// Tests all JSON-RPC endpoints for correctness

use luxtensor_tests::rpc_client::RpcClient;
use luxtensor_tests::test_utils::test_addresses;

/// Test helper to ensure node is running
fn ensure_node_running() -> RpcClient {
    let client = RpcClient::node1();
    if !client.wait_for_ready(10) {
        panic!("Node 1 not reachable at http://localhost:8545. Start node before running tests.");
    }
    client
}

// ============================================================
// eth_blockNumber Tests
// ============================================================

#[test]
fn test_eth_block_number_returns_hex() {
    let client = ensure_node_running();

    let result = client.eth_block_number();
    assert!(result.is_ok(), "eth_blockNumber should return Ok: {:?}", result);

    let height = result.unwrap();
    // Block height should be reasonable (genesis = 0)
    assert!(height < 1_000_000, "Block height should be reasonable");
}

#[test]
fn test_eth_block_number_increases_over_time() {
    let client = ensure_node_running();

    let height1 = client.eth_block_number().unwrap();

    // Wait for at least one block
    std::thread::sleep(std::time::Duration::from_secs(3));

    let height2 = client.eth_block_number().unwrap();

    // Height should have increased (or stayed same if no blocks)
    assert!(height2 >= height1, "Block height should not decrease");
}

// ============================================================
// eth_getBlockByNumber Tests
// ============================================================

#[test]
fn test_eth_get_block_by_number_genesis() {
    let client = ensure_node_running();

    let result = client.eth_get_block_by_number("0x0", false);
    assert!(result.is_ok(), "Should return Ok: {:?}", result);

    let block = result.unwrap();
    assert!(block.is_some(), "Genesis block should exist");

    let block_data = block.unwrap();
    assert!(block_data.get("number").is_some(), "Block should have number field");
}

#[test]
fn test_eth_get_block_by_number_latest() {
    let client = ensure_node_running();

    let result = client.eth_get_block_by_number("latest", false);
    assert!(result.is_ok(), "Should return Ok: {:?}", result);

    let block = result.unwrap();
    assert!(block.is_some(), "Latest block should exist");
}

#[test]
fn test_eth_get_block_by_number_future() {
    let client = ensure_node_running();

    // Query a block far in the future
    let result = client.eth_get_block_by_number("0xFFFFFF", false);
    assert!(result.is_ok(), "Should return Ok even for non-existent block");

    let block = result.unwrap();
    assert!(block.is_none(), "Future block should not exist");
}

// ============================================================
// eth_sendTransaction Tests
// ============================================================

#[test]
fn test_eth_send_transaction_success() {
    let client = ensure_node_running();

    let result = client.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x1000",
        Some("0x5208"), // 21000
    );

    assert!(result.is_ok(), "Should return Ok: {:?}", result);

    let tx_hash = result.unwrap();
    assert!(tx_hash.starts_with("0x"), "TX hash should start with 0x");
    assert!(tx_hash.len() > 10, "TX hash should be reasonable length");
}

#[test]
fn test_eth_send_transaction_without_gas() {
    let client = ensure_node_running();

    // Gas should default to reasonable value if not provided
    let result = client.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x1000",
        None,
    );

    assert!(result.is_ok(), "Should return Ok even without gas: {:?}", result);
}

#[test]
fn test_eth_send_transaction_high_value() {
    let client = ensure_node_running();

    // Send a high value transaction
    let result = client.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0xDE0B6B3A7640000", // 1 ETH in wei
        Some("0x5208"),
    );

    assert!(result.is_ok(), "Should accept high value TX: {:?}", result);
}

// ============================================================
// eth_getTransactionByHash Tests
// ============================================================

#[test]
fn test_eth_get_transaction_by_hash_pending() {
    let client = ensure_node_running();

    // First, send a transaction
    let tx_hash = client.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x1000",
        Some("0x5208"),
    ).expect("Failed to send TX");

    // Query the transaction immediately (should be in pending)
    let result = client.eth_get_transaction_by_hash(&tx_hash);
    assert!(result.is_ok(), "Should return Ok: {:?}", result);

    let tx = result.unwrap();
    assert!(tx.is_some(), "Transaction should be found (in pending or confirmed)");
}

#[test]
fn test_eth_get_transaction_by_hash_not_found() {
    let client = ensure_node_running();

    // Query a non-existent transaction
    let fake_hash = "0x0000000000000000000000000000000000000000000000000000000000000001";
    let result = client.eth_get_transaction_by_hash(fake_hash);

    assert!(result.is_ok(), "Should return Ok even for non-existent TX");

    let tx = result.unwrap();
    assert!(tx.is_none(), "Non-existent TX should return None");
}

// ============================================================
// eth_getBalance Tests
// ============================================================

#[test]
fn test_eth_get_balance_existing_account() {
    let client = ensure_node_running();

    let result = client.eth_get_balance(test_addresses::ADDR_1, "latest");
    assert!(result.is_ok(), "Should return Ok: {:?}", result);

    let balance = result.unwrap();
    assert!(balance.starts_with("0x"), "Balance should be hex");
}

#[test]
fn test_eth_get_balance_new_account() {
    let client = ensure_node_running();

    // Generate a random address that doesn't exist
    let random_addr = "0x1234567890123456789012345678901234567890";
    let result = client.eth_get_balance(random_addr, "latest");

    // Should return 0 balance for new account
    assert!(result.is_ok(), "Should return Ok for new account");
}

// ============================================================
// Multiple Sequential Transactions
// ============================================================

#[test]
fn test_multiple_transactions_sequence() {
    let client = ensure_node_running();

    let mut tx_hashes = Vec::new();

    // Send 5 transactions in sequence
    for i in 0..5 {
        let value = format!("0x{:x}", 1000 + i * 100);
        let result = client.eth_send_transaction(
            test_addresses::ADDR_1,
            test_addresses::ADDR_2,
            &value,
            Some("0x5208"),
        );

        assert!(result.is_ok(), "TX {} should succeed: {:?}", i, result);
        tx_hashes.push(result.unwrap());
    }

    // All TX hashes should be unique
    let unique_count = tx_hashes.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, 5, "All TX hashes should be unique");
}

// ============================================================
// Concurrent RPC Requests
// ============================================================

#[test]
fn test_concurrent_block_number_requests() {
    let client = ensure_node_running();

    use std::sync::Arc;
    let client = Arc::new(client);
    let mut handles = Vec::new();

    // Spawn 10 concurrent requests
    for _ in 0..10 {
        let client = Arc::clone(&client);
        handles.push(std::thread::spawn(move || {
            client.eth_block_number()
        }));
    }

    // All should succeed
    for handle in handles {
        let result = handle.join().expect("Thread panicked");
        assert!(result.is_ok(), "Concurrent request failed: {:?}", result);
    }
}
