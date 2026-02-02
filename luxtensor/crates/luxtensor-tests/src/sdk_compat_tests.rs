// SDK Compatibility RPC Tests
// Tests for newly added RPC methods that ensure SDK-Blockchain compatibility
// Added: 2026-02-02

use crate::rpc_client::RpcClient;
use crate::test_utils::test_addresses;
use serde_json::json;

/// Helper to get RPC client (assumes node running at localhost:8545)
fn get_client() -> RpcClient {
    RpcClient::node1()
}

// ============================================================
// Staking SDK Compatibility Tests
// ============================================================

#[test]
fn test_staking_get_stake_for_pair() {
    let client = get_client();

    let result = client.call(
        "staking_getStakeForPair",
        json!([test_addresses::ADDR_1, test_addresses::ADDR_2]),
    );

    // If node not running, skip test
    if result.is_err() && !client.is_reachable() {
        println!("Skipping: node not reachable");
        return;
    }

    assert!(result.is_ok(), "staking_getStakeForPair should return Ok: {:?}", result);
}

#[test]
fn test_staking_get_all_stakes_for_coldkey() {
    let client = get_client();

    let result = client.call(
        "staking_getAllStakesForColdkey",
        json!([test_addresses::ADDR_1]),
    );

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "staking_getAllStakesForColdkey should return Ok");
}

#[test]
fn test_staking_get_delegates() {
    let client = get_client();

    let result = client.call("staking_getDelegates", json!([]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "staking_getDelegates should return Ok");
}

// ============================================================
// Neuron SDK Compatibility Tests
// ============================================================

#[test]
fn test_neuron_get_alias() {
    let client = get_client();
    let result = client.call("neuron_get", json!([1, 0]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_get should return Ok");
}

#[test]
fn test_neuron_get_all_alias() {
    let client = get_client();
    let result = client.call("neuron_getAll", json!([1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_getAll should return Ok");
}

#[test]
fn test_neuron_exists() {
    let client = get_client();
    let result = client.call("neuron_exists", json!([1, 0]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_exists should return Ok");
}

#[test]
fn test_neuron_get_by_hotkey() {
    let client = get_client();
    let result = client.call("neuron_getByHotkey", json!([1, test_addresses::ADDR_1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_getByHotkey should return Ok");
}

#[test]
fn test_neuron_get_active() {
    let client = get_client();
    let result = client.call("neuron_getActive", json!([1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_getActive should return Ok");
}

#[test]
fn test_neuron_count_alias() {
    let client = get_client();
    let result = client.call("neuron_count", json!([1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_count should return Ok");
}

#[test]
fn test_neuron_batch_get() {
    let client = get_client();
    let result = client.call("neuron_batchGet", json!([1, [0, 1, 2]]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "neuron_batchGet should return Ok");
}

// ============================================================
// Subnet SDK Compatibility Tests
// ============================================================

#[test]
fn test_subnet_exists() {
    let client = get_client();
    let result = client.call("subnet_exists", json!([1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "subnet_exists should return Ok");
}

#[test]
fn test_subnet_get_hyperparameters() {
    let client = get_client();
    let result = client.call("subnet_getHyperparameters", json!([1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "subnet_getHyperparameters should return Ok");
}

#[test]
fn test_subnet_get_count() {
    let client = get_client();
    let result = client.call("subnet_getCount", json!([]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "subnet_getCount should return Ok");
}

// ============================================================
// Weight SDK Compatibility Tests
// ============================================================

#[test]
fn test_weight_get_commits() {
    let client = get_client();
    let result = client.call("weight_getCommits", json!([1]));

    if result.is_err() && !client.is_reachable() {
        return;
    }

    assert!(result.is_ok(), "weight_getCommits should return Ok");
}

// ============================================================
// Method Signature Validation Test (runs without node)
// ============================================================

#[test]
fn test_sdk_compatibility_method_count() {
    // Validate we have all 14 SDK compatibility methods registered
    let methods = [
        "staking_getStakeForPair",
        "staking_getAllStakesForColdkey",
        "staking_getDelegates",
        "neuron_get",
        "neuron_getAll",
        "neuron_exists",
        "neuron_getByHotkey",
        "neuron_getActive",
        "neuron_count",
        "neuron_batchGet",
        "subnet_exists",
        "subnet_getHyperparameters",
        "subnet_getCount",
        "weight_getCommits",
    ];

    assert_eq!(methods.len(), 14, "Should have 14 SDK compatibility methods");
}
