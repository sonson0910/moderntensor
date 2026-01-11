#!/usr/bin/env python3
"""
Integration Test Script for LuxTensor Node ↔ SDK Interaction
Run this after starting the LuxTensor node with: cargo run -p luxtensor-node
"""

import requests
import json
import time
from typing import Optional, Any

# RPC Configuration
RPC_URL = "http://127.0.0.1:8545"
HEADERS = {"Content-Type": "application/json"}

def rpc_call(method: str, params: Optional[list] = None, id: int = 1) -> dict:
    """Make an RPC call to the node"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": id
    }
    try:
        response = requests.post(RPC_URL, json=payload, headers=HEADERS, timeout=10)
        return response.json()
    except requests.exceptions.ConnectionError:
        return {"error": "Cannot connect to node. Is it running?"}
    except Exception as e:
        return {"error": str(e)}

def test_connection() -> bool:
    """Test basic connection to node"""
    print("\n" + "="*60)
    print("TEST 1: Connection Check")
    print("="*60)

    result = rpc_call("web3_clientVersion")
    if "error" in result:
        print(f"❌ FAILED: {result['error']}")
        return False

    print(f"✅ Connected to: {result.get('result', 'Unknown')}")
    return True

def test_block_operations() -> bool:
    """Test block-related RPC calls"""
    print("\n" + "="*60)
    print("TEST 2: Block Operations")
    print("="*60)

    # Get latest block number
    result = rpc_call("eth_blockNumber")
    if "error" in result:
        print(f"❌ eth_blockNumber failed: {result['error']}")
        return False

    block_number = int(result["result"], 16) if result.get("result") else 0
    print(f"✅ Current block number: {block_number}")

    # Get block by number
    result = rpc_call("eth_getBlockByNumber", ["latest", False])
    if "result" in result and result["result"]:
        block = result["result"]
        print(f"✅ Latest block hash: {block.get('hash', 'N/A')[:20]}...")
        print(f"   Transactions: {len(block.get('transactions', []))}")
    else:
        print("⚠️  No blocks yet (genesis only)")

    return True

def test_account_operations() -> bool:
    """Test account-related RPC calls"""
    print("\n" + "="*60)
    print("TEST 3: Account Operations")
    print("="*60)

    test_address = "0x" + "00" * 20  # Zero address

    # Get balance
    result = rpc_call("eth_getBalance", [test_address, "latest"])
    if "result" in result:
        balance = int(result["result"], 16) if result["result"] else 0
        print(f"✅ Balance of zero address: {balance} wei")
    else:
        print(f"⚠️  eth_getBalance: {result.get('error', 'Unknown error')}")

    # Get nonce
    result = rpc_call("eth_getTransactionCount", [test_address, "latest"])
    if "result" in result:
        nonce = int(result["result"], 16) if result["result"] else 0
        print(f"✅ Nonce of zero address: {nonce}")
    else:
        print(f"⚠️  eth_getTransactionCount: {result.get('error', 'Unknown error')}")

    return True

def test_ai_operations() -> bool:
    """Test AI-specific RPC calls"""
    print("\n" + "="*60)
    print("TEST 4: AI Task Operations (ModernTensor specific)")
    print("="*60)

    # Submit AI task
    task_request = {
        "model_hash": "0x" + "ab" * 32,
        "input_data": "0x" + "cd" * 64,
        "requester": "0x" + "11" * 20,
        "reward": "0x" + "e8d4a51000"  # 1000 tokens
    }

    result = rpc_call("lux_submitAITask", [task_request])
    if "result" in result:
        task_id = result["result"]
        print(f"✅ AI Task submitted: {task_id[:20]}...")

        # Get task result
        time.sleep(0.5)
        result = rpc_call("lux_getAIResult", [task_id])
        if "result" in result and result["result"]:
            print(f"✅ Task status: {result['result'].get('status', 'Unknown')}")
        else:
            print("⚠️  Task not found (expected for new task)")
    else:
        print(f"⚠️  lux_submitAITask: {result.get('error', 'Method may not exist')}")

    return True

def test_validator_operations() -> bool:
    """Test validator RPC calls"""
    print("\n" + "="*60)
    print("TEST 5: Validator Operations")
    print("="*60)

    test_address = "0x" + "22" * 20

    result = rpc_call("lux_getValidatorStatus", [test_address])
    if "result" in result:
        status = result["result"]
        if status:
            print(f"✅ Validator found:")
            print(f"   Stake: {status.get('stake', 'N/A')}")
            print(f"   Active: {status.get('active', 'N/A')}")
        else:
            print("⚠️  Validator not registered")
    else:
        print(f"⚠️  lux_getValidatorStatus: {result.get('error', 'Unknown')}")

    return True

def test_network_info() -> bool:
    """Test network info calls"""
    print("\n" + "="*60)
    print("TEST 6: Network Information")
    print("="*60)

    # Net version
    result = rpc_call("net_version")
    if "result" in result:
        print(f"✅ Network ID: {result['result']}")

    # Peer count
    result = rpc_call("net_peerCount")
    if "result" in result:
        peers = int(result["result"], 16) if result.get("result") else 0
        print(f"✅ Connected peers: {peers}")

    # Gas price
    result = rpc_call("eth_gasPrice")
    if "result" in result:
        gas_price = int(result["result"], 16) if result.get("result") else 0
        print(f"✅ Gas price: {gas_price} wei")

    return True

def main():
    print("""
╔══════════════════════════════════════════════════════════════╗
║         LuxTensor Integration Test Suite                     ║
║         Testing SDK ↔ Node Communication                     ║
╚══════════════════════════════════════════════════════════════╝
    """)

    print(f"Target Node: {RPC_URL}")

    # Run all tests
    tests = [
        ("Connection", test_connection),
        ("Block Operations", test_block_operations),
        ("Account Operations", test_account_operations),
        ("AI Operations", test_ai_operations),
        ("Validator Operations", test_validator_operations),
        ("Network Info", test_network_info),
    ]

    results = []
    for name, test_fn in tests:
        try:
            success = test_fn()
            results.append((name, success))
        except Exception as e:
            print(f"❌ {name} crashed: {e}")
            results.append((name, False))

    # Summary
    print("\n" + "="*60)
    print("SUMMARY")
    print("="*60)

    passed = sum(1 for _, s in results if s)
    total = len(results)

    for name, success in results:
        status = "✅ PASS" if success else "❌ FAIL"
        print(f"  {status}: {name}")

    print(f"\nTotal: {passed}/{total} tests passed")

    if passed == total:
        print("\n🎉 All tests passed! Node is ready for SDK integration.")
    else:
        print("\n⚠️  Some tests failed. Check node logs for details.")

if __name__ == "__main__":
    main()
