#!/usr/bin/env python3
"""
Integration Tests for ModernTensor Python SDK
Tests run against a live LuxTensor node (default: http://localhost:8545)

Usage:
    python tests/integration_test_sdk.py
    python tests/integration_test_sdk.py --rpc http://localhost:8545

Requirements:
    pip install requests
"""

import argparse
import sys
import time
import requests
from typing import Any, Callable, Dict, Optional


RPC_URL = "http://localhost:8545"
PASSED = 0
FAILED = 0


def rpc_call(method: str, params: list = None, url: str = None) -> Dict[str, Any]:
    """Make a JSON-RPC 2.0 call to the node."""
    if url is None:
        url = RPC_URL
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": int(time.time() * 1000),
    }
    try:
        resp = requests.post(url, json=payload, timeout=10)
        resp.raise_for_status()
        return resp.json()
    except requests.exceptions.ConnectionError:
        return {"error": {"code": -32000, "message": "Connection refused — is the node running?"}}
    except Exception as e:
        return {"error": {"code": -32000, "message": str(e)}}


def assert_ok(result: Dict, test_name: str, check_fn: Optional[Callable] = None) -> bool:
    """Assert a result is successful and optionally validate the result."""
    global PASSED, FAILED
    if "error" in result:
        print(f"  ✗ {test_name}: RPC error — {result['error']}")
        FAILED += 1
        return False
    v = result.get("result")
    if check_fn is not None:
        try:
            check_fn(v)
        except AssertionError as e:
            print(f"  ✗ {test_name}: Assertion failed — {e}")
            FAILED += 1
            return False
        except Exception as e:
            print(f"  ✗ {test_name}: Error in check — {e}")
            FAILED += 1
            return False
    print(f"  ✓ {test_name}  →  {str(v)[:80]}")
    PASSED += 1
    return True


def assert_error(result: Dict, test_name: str) -> bool:
    """Assert a result is an error (for negative testing)."""
    global PASSED, FAILED
    if "error" in result:
        print(f"  ✓ {test_name} (expected error: {result['error'].get('message', '')})")
        PASSED += 1
        return True
    print(f"  ✗ {test_name}: Expected error but got success")
    FAILED += 1
    return False


def chk(cond: bool, msg: str = "") -> None:
    """Safe assert helper for use in check functions."""
    if not cond:
        raise AssertionError(msg)


# ============================================================
# Test Groups
# ============================================================

def test_node_health():
    """Test: Node is alive and healthy."""
    print("\n[1] Node Health & Status")

    r = rpc_call("system_health")
    assert_ok(r, "system_health", lambda v: chk(v is not None, "result is None"))

    r = rpc_call("eth_blockNumber")
    assert_ok(r, "eth_blockNumber", lambda v: chk(
        v is not None and str(v).startswith("0x"),
        f"Expected hex block number, got: {v}"
    ))

    r = rpc_call("system_nodeStats")
    assert_ok(r, "system_nodeStats", lambda v: chk(
        isinstance(v, dict) and "chain_id" in v,
        f"Expected dict with chain_id, got: {v}"
    ))


def test_lux_subnet_methods():
    """Test: lux_* subnet RPC methods."""
    print("\n[2] lux_* Subnet Methods (MetagraphDB)")

    r = rpc_call("lux_listSubnets")
    assert_ok(r, "lux_listSubnets", lambda v: chk(isinstance(v, list), f"Expected list, got {type(v)}"))

    r = rpc_call("lux_getSubnetCount")
    assert_ok(r, "lux_getSubnetCount", lambda v: chk(v is not None, "Count is None"))

    r = rpc_call("lux_getSubnetInfo", [1])
    assert_ok(r, "lux_getSubnetInfo [id=1]")

    r = rpc_call("lux_getEmissions", [1])
    assert_ok(r, "lux_getEmissions [subnet_id=1]")


def test_lux_neuron_methods():
    """Test: lux_* neuron RPC methods."""
    print("\n[3] lux_* Neuron Methods (MetagraphDB)")

    r = rpc_call("lux_getNeurons", [1])
    assert_ok(r, "lux_getNeurons [subnet_id=1]", lambda v: chk(isinstance(v, list), f"Expected list, got {type(v)}"))

    # Check neuron fields if any neurons exist
    neurons = r.get("result", [])
    if neurons:
        n = neurons[0]
        required = ["uid", "hotkey", "coldkey", "stake", "trust", "consensus", "rank",
                    "incentive", "dividends", "emission", "last_update", "active"]
        missing = [f for f in required if f not in n]
        if missing:
            print(f"  ⚠ lux_getNeurons: missing fields: {missing}")
        else:
            print(f"  ✓ lux_getNeurons: all {len(required)} required fields present")

    r = rpc_call("lux_getNeuron", [1, 0])
    assert_ok(r, "lux_getNeuron [subnet_id=1, uid=0]")

    r = rpc_call("lux_getNeuronCount", [1])
    assert_ok(r, "lux_getNeuronCount [subnet_id=1]", lambda v: chk(v is not None, "Count is None"))


def test_lux_weight_methods():
    """Test: lux_* weight RPC methods."""
    print("\n[4] lux_* Weight Methods (MetagraphDB)")

    r = rpc_call("lux_getWeights", [1, 0])
    assert_ok(r, "lux_getWeights [subnet_id=1, uid=0]", lambda v: chk(isinstance(v, list), f"Expected list, got {type(v)}"))

    r = rpc_call("lux_getAllWeights", [1])
    assert_ok(r, "lux_getAllWeights [subnet_id=1]", lambda v: chk(isinstance(v, list), f"Expected list, got {type(v)}"))


def test_metagraph_methods():
    """Test: metagraph_* RPC methods."""
    print("\n[5] metagraph_* Methods")

    r = rpc_call("metagraph_getState", [1])
    assert_ok(r, "metagraph_getState [subnet_id=1]")

    r = rpc_call("metagraph_getWeights", [1])
    assert_ok(r, "metagraph_getWeights [subnet_id=1]", lambda v: chk(
        isinstance(v, (list, dict)), f"Expected list or dict, got {type(v)}"
    ))


def test_system_methods():
    """Test: system_* RPC methods."""
    print("\n[6] system_* Methods")

    r = rpc_call("system_nodeRoles")
    v = r.get("result")
    if "error" in r and r["error"].get("code") == -32601:
        print(f"  ~ system_nodeRoles: not implemented (optional)")
    else:
        assert_ok(r, "system_nodeRoles", lambda v: chk(isinstance(v, (list, dict)), f"Expected list/dict, got {type(v)}"))

    r = rpc_call("system_getAICircuitBreakerStatus")
    assert_ok(r, "system_getAICircuitBreakerStatus", lambda v: chk(isinstance(v, dict), f"Expected dict, got {type(v)}"))


def test_neuron_info_consistency():
    """Test: neuron_getInfo / query_getNeuron have consistent fields."""
    print("\n[7] Neuron Handler Consistency (neuron.rs ↔ metagraph.rs)")

    r1 = rpc_call("lux_getNeurons", [1])
    r2 = rpc_call("query_getNeurons", [1, 0])  # subnet_id, page/offset

    neurons_lux = r1.get("result", []) or []
    neurons_query = r2.get("result", []) or []

    assert_ok(r1, "lux_getNeurons returns list", lambda v: chk(isinstance(v, list), "not list"))

    if neurons_lux and isinstance(neurons_query, list) and neurons_query:
        n1 = neurons_lux[0]
        n2 = neurons_query[0]
        lux_keys = set(n1.keys())
        query_keys = set(n2.keys())
        only_in_lux = lux_keys - query_keys
        only_in_query = query_keys - lux_keys
        if only_in_lux:
            print(f"  ⚠ Fields only in lux_getNeurons: {only_in_lux}")
        if only_in_query:
            print(f"  ⚠ Fields only in query_getNeurons: {only_in_query}")
        if not only_in_lux and not only_in_query:
            print(f"  ✓ lux_getNeurons vs query_getNeurons: field sets match ({len(lux_keys)} fields)")
    else:
        print(f"  ~ Skipped field comparison (no neurons or query_getNeurons returned non-list)")


def test_legacy_write_reads_from_metagraph():
    """Test dual-write: neuron_register → lux_getNeurons."""
    print("\n[8] Dual-Write Path (neuron_register → lux_getNeurons)")

    before_r = rpc_call("lux_getNeuronCount", [99])
    before_v = before_r.get("result", 0)
    if isinstance(before_v, dict):
        before_count = int(before_v.get("count", 0))
    elif str(before_v).startswith("0x"):
        before_count = int(str(before_v), 16)
    else:
        before_count = int(before_v or 0)

    r = rpc_call("neuron_register", [
        99,
        "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        "0xcafebabecafebabecafebabecafebabecafebabe",
        "http://test-neuron-integration.local:8080",
        "0x0",
    ])
    assert_ok(r, "neuron_register [subnet_id=99]")

    if "error" not in r:
        time.sleep(0.3)
        r2 = rpc_call("lux_getNeurons", [99])
        assert_ok(
            r2,
            "lux_getNeurons after neuron_register [dual-write check]",
            lambda v: chk(isinstance(v, list) and len(v) >= 1,
                          f"Expected ≥1 neuron in subnet 99, got {len(v) if isinstance(v, list) else v}")
        )


def test_weight_dual_write():
    """Test dual-write: weight_setWeights → lux_getWeights."""
    print("\n[9] Dual-Write Path (weight_setWeights → lux_getWeights)")

    r = rpc_call("weight_setWeights", [1, 0, [[1, 100], [2, 200], [3, 150]]])
    assert_ok(r, "weight_setWeights [subnet_id=1, uid=0]")

    if "error" not in r:
        time.sleep(0.3)
        r2 = rpc_call("lux_getWeights", [1, 0])
        assert_ok(
            r2,
            "lux_getWeights after weight_setWeights [dual-write check]",
            lambda v: chk(isinstance(v, list), f"Expected list, got {type(v)}")
        )


def test_error_handling():
    """Test: Invalid parameters return proper errors."""
    print("\n[10] Error Handling")

    r = rpc_call("lux_getNeuron", [])
    assert_error(r, "lux_getNeuron with no params → error")

    r = rpc_call("nonexistent_method")
    assert_error(r, "nonexistent_method → error")

    r = rpc_call("lux_getWeights", ["bad", "bad"])
    print(f"  ~ lux_getWeights with bad params → {'error' if 'error' in r else 'ok (lenient parsing)'}")


def run_all_tests(rpc_url: str):
    """Run all integration tests."""
    global RPC_URL
    RPC_URL = rpc_url

    print(f"🧪 ModernTensor SDK Integration Tests")
    print(f"   Node: {rpc_url}")
    print(f"   Time: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    print("=" * 60)

    # Quick reachability check
    r = rpc_call("eth_blockNumber")
    if "error" in r and "Connection refused" in str(r["error"]):
        print(f"\n❌ Cannot connect to node at {rpc_url}")
        print("   Start the node first: cargo run --bin luxtensor-node")
        sys.exit(1)

    test_node_health()
    test_lux_subnet_methods()
    test_lux_neuron_methods()
    test_lux_weight_methods()
    test_metagraph_methods()
    test_system_methods()
    test_neuron_info_consistency()
    test_legacy_write_reads_from_metagraph()
    test_weight_dual_write()
    test_error_handling()

    print("\n" + "=" * 60)
    total = PASSED + FAILED
    print(f"Results: {PASSED}/{total} passed", end="")
    if FAILED > 0:
        print(f" ({FAILED} failed) ❌")
        sys.exit(1)
    else:
        print(" ✅")
        sys.exit(0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="ModernTensor SDK Integration Tests")
    parser.add_argument(
        "--rpc",
        default="http://localhost:8545",
        help="RPC URL of the LuxTensor node (default: http://localhost:8545)",
    )
    args = parser.parse_args()
    run_all_tests(args.rpc)
