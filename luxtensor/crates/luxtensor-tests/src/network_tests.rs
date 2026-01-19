// Multi-Node Network Tests for LuxTensor
// Tests TX propagation and block sync across multiple nodes
//
// NOTE: These tests require manually starting nodes before running.
// Run: cargo test --package luxtensor-tests --test network_tests -- --ignored
// Or use the test runner script.

use luxtensor_tests::rpc_client::RpcClient;
use luxtensor_tests::test_utils::{test_addresses, sleep_ms_blocking};
use std::time::Duration;

// ============================================================
// Test Helpers
// ============================================================

fn ensure_two_nodes_running() -> (RpcClient, RpcClient) {
    let node1 = RpcClient::node1();
    let node2 = RpcClient::node2();

    if !node1.wait_for_ready(10) {
        panic!("Node 1 not reachable. Start nodes before running network tests.");
    }
    if !node2.wait_for_ready(10) {
        panic!("Node 2 not reachable. Start nodes before running network tests.");
    }

    (node1, node2)
}

fn ensure_three_nodes_running() -> (RpcClient, RpcClient, RpcClient) {
    let (node1, node2) = ensure_two_nodes_running();
    let node3 = RpcClient::node3();

    if !node3.wait_for_ready(10) {
        panic!("Node 3 not reachable. Start all 3 nodes before running network tests.");
    }

    (node1, node2, node3)
}

// ============================================================
// Transaction Propagation Tests
// ============================================================

#[test]
#[ignore] // Requires running nodes
fn test_tx_propagation_node1_to_node2() {
    let (node1, node2) = ensure_two_nodes_running();

    // Send TX to Node 1
    let tx_hash = node1.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x1000",
        Some("0x5208"),
    ).expect("Failed to send TX to Node 1");

    println!("TX Hash: {}", tx_hash);

    // Wait for propagation
    std::thread::sleep(Duration::from_secs(5));

    // Query TX from Node 1 (should be found)
    let tx1 = node1.eth_get_transaction_by_hash(&tx_hash).unwrap();
    assert!(tx1.is_some(), "TX should be found on Node 1");

    // Query TX from Node 2 (should be propagated)
    let tx2 = node2.eth_get_transaction_by_hash(&tx_hash).unwrap();
    assert!(tx2.is_some(), "TX should be propagated to Node 2");
}

#[test]
#[ignore] // Requires running nodes
fn test_tx_propagation_node2_to_node1() {
    let (node1, node2) = ensure_two_nodes_running();

    // Send TX to Node 2
    let tx_hash = node2.eth_send_transaction(
        test_addresses::ADDR_2,
        test_addresses::ADDR_1,
        "0x2000",
        Some("0x5208"),
    ).expect("Failed to send TX to Node 2");

    println!("TX Hash: {}", tx_hash);

    // Wait for propagation
    std::thread::sleep(Duration::from_secs(5));

    // Query TX from both nodes
    let tx1 = node1.eth_get_transaction_by_hash(&tx_hash).unwrap();
    let tx2 = node2.eth_get_transaction_by_hash(&tx_hash).unwrap();

    assert!(tx2.is_some(), "TX should be found on Node 2 (originator)");
    assert!(tx1.is_some(), "TX should be propagated to Node 1");
}

#[test]
#[ignore] // Requires running nodes
fn test_tx_propagation_to_all_3_nodes() {
    let (node1, node2, node3) = ensure_three_nodes_running();

    // Send TX to Node 1
    let tx_hash = node1.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x3000",
        Some("0x5208"),
    ).expect("Failed to send TX");

    println!("TX Hash: {}", tx_hash);

    // Wait for propagation
    std::thread::sleep(Duration::from_secs(8));

    // Query from all nodes
    let tx1 = node1.eth_get_transaction_by_hash(&tx_hash).unwrap();
    let tx2 = node2.eth_get_transaction_by_hash(&tx_hash).unwrap();
    let tx3 = node3.eth_get_transaction_by_hash(&tx_hash).unwrap();

    assert!(tx1.is_some(), "TX should be found on Node 1");
    assert!(tx2.is_some(), "TX should be propagated to Node 2");
    assert!(tx3.is_some(), "TX should be propagated to Node 3");
}

#[test]
#[ignore] // Requires running nodes
fn test_concurrent_tx_from_multiple_nodes() {
    let (node1, node2, node3) = ensure_three_nodes_running();

    // Send TX from Node 1 and Node 2 concurrently
    let tx_hash1 = node1.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x1000",
        Some("0x5208"),
    ).expect("Failed to send TX from Node 1");

    let tx_hash2 = node2.eth_send_transaction(
        test_addresses::ADDR_2,
        test_addresses::ADDR_1,
        "0x2000",
        Some("0x5208"),
    ).expect("Failed to send TX from Node 2");

    println!("TX1 Hash: {}", tx_hash1);
    println!("TX2 Hash: {}", tx_hash2);

    // Wait for propagation
    std::thread::sleep(Duration::from_secs(8));

    // Both TXs should be found on Node 3
    let tx1_on_node3 = node3.eth_get_transaction_by_hash(&tx_hash1).unwrap();
    let tx2_on_node3 = node3.eth_get_transaction_by_hash(&tx_hash2).unwrap();

    assert!(tx1_on_node3.is_some(), "TX1 should be on Node 3");
    assert!(tx2_on_node3.is_some(), "TX2 should be on Node 3");
}

// ============================================================
// Block Sync Tests
// ============================================================

#[test]
#[ignore] // Requires running nodes
fn test_block_sync_between_nodes() {
    let (node1, node2) = ensure_two_nodes_running();

    // Get current heights
    let height1 = node1.eth_block_number().unwrap();
    let height2 = node2.eth_block_number().unwrap();

    println!("Node 1 height: {}", height1);
    println!("Node 2 height: {}", height2);

    // Heights should be within a few blocks of each other
    let diff = if height1 > height2 { height1 - height2 } else { height2 - height1 };
    assert!(diff < 5, "Nodes should be synced within 5 blocks, diff: {}", diff);
}

#[test]
#[ignore] // Requires running nodes
fn test_block_sync_3_nodes() {
    let (node1, node2, node3) = ensure_three_nodes_running();

    // Wait for sync
    std::thread::sleep(Duration::from_secs(10));

    let height1 = node1.eth_block_number().unwrap();
    let height2 = node2.eth_block_number().unwrap();
    let height3 = node3.eth_block_number().unwrap();

    println!("Node 1 height: {}", height1);
    println!("Node 2 height: {}", height2);
    println!("Node 3 height: {}", height3);

    // All nodes should be synced
    let max_height = height1.max(height2).max(height3);
    let min_height = height1.min(height2).min(height3);

    assert!(max_height - min_height < 5,
        "All nodes should be synced within 5 blocks. Heights: {}, {}, {}",
        height1, height2, height3);
}

#[test]
#[ignore] // Requires running nodes
fn test_blocks_are_identical() {
    let (node1, node2) = ensure_two_nodes_running();

    // Get a block that both should have (genesis)
    let block1 = node1.eth_get_block_by_number("0x0", false).unwrap();
    let block2 = node2.eth_get_block_by_number("0x0", false).unwrap();

    assert!(block1.is_some(), "Node 1 should have genesis");
    assert!(block2.is_some(), "Node 2 should have genesis");

    // Compare block hashes
    let block1_data = block1.unwrap();
    let block2_data = block2.unwrap();
    let hash1 = block1_data.get("hash").and_then(|v| v.as_str());
    let hash2 = block2_data.get("hash").and_then(|v| v.as_str());

    assert_eq!(hash1, hash2, "Genesis blocks should be identical");
}

// ============================================================
// Stress Tests (Multi-Node)
// ============================================================

#[test]
#[ignore] // Requires running nodes
fn test_rapid_tx_submission() {
    let (node1, _node2) = ensure_two_nodes_running();

    let mut tx_hashes = Vec::new();

    // Submit 20 TXs rapidly
    for i in 0..20 {
        let value = format!("0x{:x}", 1000 + i * 100);
        let result = node1.eth_send_transaction(
            test_addresses::ADDR_1,
            test_addresses::ADDR_2,
            &value,
            Some("0x5208"),
        );

        if let Ok(hash) = result {
            tx_hashes.push(hash);
        }

        // Small delay to avoid overwhelming
        sleep_ms_blocking(50);
    }

    println!("Submitted {} transactions", tx_hashes.len());
    assert!(tx_hashes.len() >= 15, "At least 15 TXs should succeed");
}

#[test]
#[ignore] // Requires running nodes
fn test_tx_hash_consistency_across_nodes() {
    let (node1, node2) = ensure_two_nodes_running();

    // Send TX to Node 1
    let tx_hash = node1.eth_send_transaction(
        test_addresses::ADDR_1,
        test_addresses::ADDR_2,
        "0x5000",
        Some("0x5208"),
    ).expect("Failed to send TX");

    // Wait for propagation
    std::thread::sleep(Duration::from_secs(5));

    // Get TX details from both nodes
    let tx1 = node1.eth_get_transaction_by_hash(&tx_hash).unwrap();
    let tx2 = node2.eth_get_transaction_by_hash(&tx_hash).unwrap();

    assert!(tx1.is_some(), "TX should be on Node 1");
    assert!(tx2.is_some(), "TX should be on Node 2");

    // TX details should be identical
    let tx1_data = tx1.unwrap();
    let tx2_data = tx2.unwrap();

    assert_eq!(
        tx1_data.get("from"),
        tx2_data.get("from"),
        "TX 'from' should match across nodes"
    );
    assert_eq!(
        tx1_data.get("to"),
        tx2_data.get("to"),
        "TX 'to' should match across nodes"
    );
}
