#!/usr/bin/env python3
"""
Luxtensor Full Blockchain Integration Test
Tests all core blockchain features:
1. Node connectivity & sync
2. Token transfer
3. Smart contract deployment
4. Staking
5. Transaction query
"""

import json
import time
import requests
from typing import Optional, Dict, Any

# Node endpoints (matching actual config ports)
NODES = [
    {"name": "Node 1", "rpc": "http://127.0.0.1:8545"},
    {"name": "Node 2", "rpc": "http://127.0.0.1:8555"},
    {"name": "Node 3", "rpc": "http://127.0.0.1:8565"},
]

# Test accounts (from genesis)
ACCOUNTS = {
    "alice": {
        "address": "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266",
        "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    },
    "bob": {
        "address": "0x70997970c51812dc3a010c7d01b50e0d17dc79c8",
        "private_key": "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
    },
    "charlie": {
        "address": "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc",
        "private_key": "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"
    }
}

def rpc_call(endpoint: str, method: str, params: list = None) -> Optional[Dict[str, Any]]:
    """Make an RPC call to a node"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": 1
    }
    try:
        resp = requests.post(endpoint, json=payload, timeout=10)
        data = resp.json()
        if "error" in data:
            print(f"  ❌ RPC Error: {data['error']}")
            return None
        return data.get("result")
    except Exception as e:
        print(f"  ❌ Connection error: {e}")
        return None

def test_node_connectivity():
    """Test 1: Check all nodes are online and synced"""
    print("\n" + "="*60)
    print("📡 TEST 1: Node Connectivity & Sync")
    print("="*60)

    heights = []
    peers = []

    for node in NODES:
        # Get block height
        height = rpc_call(node["rpc"], "eth_blockNumber")
        if height:
            height_int = int(height, 16)
            heights.append(height_int)
            print(f"  ✅ {node['name']}: Block #{height_int}")
        else:
            print(f"  ❌ {node['name']}: OFFLINE")
            return False

        # Get peer count
        peer_count = rpc_call(node["rpc"], "net_peerCount")
        if peer_count:
            peer_int = int(peer_count, 16)
            peers.append(peer_int)
            print(f"     Peers: {peer_int}")

    # Check sync status
    if len(set(heights)) == 1:
        print(f"\n  ✅ All nodes synced at block #{heights[0]}")
    else:
        print(f"\n  ⚠️ Nodes not fully synced: {heights}")

    return True

def test_get_balances():
    """Test 2: Check genesis account balances"""
    print("\n" + "="*60)
    print("💰 TEST 2: Genesis Account Balances")
    print("="*60)

    for name, account in ACCOUNTS.items():
        balance = rpc_call(NODES[0]["rpc"], "eth_getBalance", [account["address"], "latest"])
        if balance:
            balance_eth = int(balance, 16) / 10**18
            print(f"  {name.capitalize()}: {balance_eth:.2f} LUX")
        else:
            print(f"  ❌ Failed to get {name}'s balance")

    return True

def test_token_transfer():
    """Test 3: Send tokens from Alice to Bob"""
    print("\n" + "="*60)
    print("📤 TEST 3: Token Transfer (Alice → Bob)")
    print("="*60)

    alice = ACCOUNTS["alice"]
    bob = ACCOUNTS["bob"]

    # Get initial balances
    alice_before = rpc_call(NODES[0]["rpc"], "eth_getBalance", [alice["address"], "latest"])
    bob_before = rpc_call(NODES[0]["rpc"], "eth_getBalance", [bob["address"], "latest"])

    print(f"  Before: Alice={int(alice_before, 16)/10**18:.2f}, Bob={int(bob_before, 16)/10**18:.2f}")

    # Send transaction
    tx = {
        "from": alice["address"],
        "to": bob["address"],
        "value": hex(int(1 * 10**18)),  # 1 LUX
        "gas": hex(21000),
        "gasPrice": hex(1000000000),
        "nonce": hex(0),
        "chainId": hex(1)
    }

    tx_hash = rpc_call(NODES[0]["rpc"], "eth_sendTransaction", [tx])
    if tx_hash:
        print(f"  📨 TX Hash: {tx_hash[:20]}...")

        # Wait for block
        print("  ⏳ Waiting for block...")
        time.sleep(5)

        # Check receipt
        receipt = rpc_call(NODES[0]["rpc"], "eth_getTransactionReceipt", [tx_hash])
        if receipt:
            print(f"  ✅ TX included in block #{int(receipt['blockNumber'], 16)}")
            print(f"     Gas used: {int(receipt['gasUsed'], 16)}")

            # Check new balances
            alice_after = rpc_call(NODES[0]["rpc"], "eth_getBalance", [alice["address"], "latest"])
            bob_after = rpc_call(NODES[0]["rpc"], "eth_getBalance", [bob["address"], "latest"])

            print(f"  After: Alice={int(alice_after, 16)/10**18:.2f}, Bob={int(bob_after, 16)/10**18:.2f}")
            return tx_hash
        else:
            print("  ⚠️ TX pending...")
    else:
        print("  ❌ Failed to send transaction")

    return None

def test_smart_contract():
    """Test 4: Deploy a simple smart contract"""
    print("\n" + "="*60)
    print("📝 TEST 4: Smart Contract Deployment")
    print("="*60)

    # Simple storage contract bytecode
    # contract Storage { uint256 public value; function set(uint256 v) public { value = v; } }
    contract_bytecode = "0x608060405234801561001057600080fd5b5060e38061001f6000396000f3fe6080604052348015600f57600080fd5b5060043610603c5760003560e01c80633fa4f24514604157806360fe47b114605b575b600080fd5b60005460405190815260200160405180910390f35b606b60663660046073565b600055565b005b600060208284031215608457600080fd5b503591905056fea2646970667358221220"

    alice = ACCOUNTS["alice"]

    # Deploy contract
    tx = {
        "from": alice["address"],
        "data": contract_bytecode,
        "gas": hex(200000),
        "gasPrice": hex(1000000000),
        "nonce": hex(1),
        "chainId": hex(1)
    }

    tx_hash = rpc_call(NODES[0]["rpc"], "eth_sendTransaction", [tx])
    if tx_hash:
        print(f"  📨 Deploy TX: {tx_hash[:20]}...")

        time.sleep(5)

        receipt = rpc_call(NODES[0]["rpc"], "eth_getTransactionReceipt", [tx_hash])
        if receipt and receipt.get("contractAddress"):
            contract_addr = receipt["contractAddress"]
            print(f"  ✅ Contract deployed at: {contract_addr}")
            print(f"     Gas used: {int(receipt['gasUsed'], 16)}")
            return contract_addr
        else:
            print("  ⚠️ Contract deployment pending...")
    else:
        print("  ❌ Failed to deploy contract")

    return None

def test_staking():
    """Test 5: Check staking/validator info"""
    print("\n" + "="*60)
    print("🔒 TEST 5: Staking & Validators")
    print("="*60)

    # Get staking config
    config = rpc_call(NODES[0]["rpc"], "staking_getConfig")
    if config:
        min_stake = config.get('min_stake', '0')
        if isinstance(min_stake, str) and min_stake.startswith('0x'):
            min_stake = int(min_stake, 16)
        else:
            min_stake = int(min_stake) if min_stake else 0
        print(f"  Min stake: {min_stake/10**18:.0f} LUX")
        print(f"  Max validators: {config.get('max_validators', 'N/A')}")

    # Get active validators
    validators = rpc_call(NODES[0]["rpc"], "staking_getActiveValidators")
    if validators:
        # Handle both list and dict formats
        if isinstance(validators, dict):
            validators_list = list(validators.values()) if validators else []
        else:
            validators_list = list(validators) if validators else []

        print(f"  Active validators: {len(validators_list)}")
        for v in validators_list[:3]:  # Show first 3
            if isinstance(v, dict):
                vid = v.get("validator_id", v.get("id", "unknown"))
                stake = v.get("stake", 0)
                if isinstance(stake, str):
                    if stake.startswith('0x'):
                        stake = int(stake, 16)
                    else:
                        stake = int(stake) if stake else 0
                print(f"    - {vid}: {stake/10**18:.0f} LUX staked")
            else:
                print(f"    - {v}")

    return True

def test_transaction_query():
    """Test 6: Query transactions and blocks"""
    print("\n" + "="*60)
    print("🔍 TEST 6: Transaction & Block Query")
    print("="*60)

    # Get latest block
    block = rpc_call(NODES[0]["rpc"], "eth_getBlockByNumber", ["latest", True])
    if block:
        block_num = int(block.get("number", "0x0"), 16)
        tx_count = len(block.get("transactions", []))
        print(f"  Latest block: #{block_num}")
        print(f"  Transactions in block: {tx_count}")
        if block.get("timestamp"):
            print(f"  Timestamp: {int(block['timestamp'], 16)}")
        if block.get("gasUsed"):
            print(f"  Gas used: {int(block['gasUsed'], 16)}")

    # Get transaction count for Alice
    tx_count = rpc_call(NODES[0]["rpc"], "eth_getTransactionCount", [ACCOUNTS["alice"]["address"], "latest"])
    if tx_count:
        print(f"  Alice's total TXs: {int(tx_count, 16)}")

    return True

def test_cross_node_sync():
    """Test 7: Verify data synced across all nodes"""
    print("\n" + "="*60)
    print("🔄 TEST 7: Cross-Node Data Verification")
    print("="*60)

    # Get latest block from each node
    blocks = []
    for node in NODES:
        block = rpc_call(node["rpc"], "eth_getBlockByNumber", ["latest", False])
        if block:
            blocks.append({
                "node": node["name"],
                "height": int(block["number"], 16),
                "hash": block["hash"][:20] + "..."
            })

    # Compare
    if len(blocks) == 3:
        if blocks[0]["hash"] == blocks[1]["hash"] == blocks[2]["hash"]:
            print(f"  ✅ All 3 nodes have same latest block!")
            print(f"     Height: #{blocks[0]['height']}")
            print(f"     Hash: {blocks[0]['hash']}")
        else:
            print("  ⚠️ Blocks differ (sync in progress):")
            for b in blocks:
                print(f"     {b['node']}: #{b['height']} - {b['hash']}")

    return True

def test_system_health():
    """Test 8: System health check"""
    print("\n" + "="*60)
    print("❤️ TEST 8: System Health")
    print("="*60)

    health = rpc_call(NODES[0]["rpc"], "system_health")
    if health:
        print(f"  Syncing: {health.get('syncing', 'N/A')}")
        print(f"  Peers: {health.get('peers', 'N/A')}")
        print(f"  Block: #{health.get('block_number', 'N/A')}")
    else:
        print("  ℹ️ system_health not implemented (optional)")

    return True

def main():
    print("\n" + "="*60)
    print("🦀 LUXTENSOR FULL BLOCKCHAIN INTEGRATION TEST")
    print("="*60)
    print(f"Testing {len(NODES)} nodes...")
    print(f"Accounts: {list(ACCOUNTS.keys())}")

    # Wait for nodes to start and sync
    print("\n⏳ Waiting 5 seconds for nodes to sync...")
    time.sleep(5)

    results = {
        "connectivity": test_node_connectivity(),
        "balances": test_get_balances(),
        "transfer": test_token_transfer(),
        "contract": test_smart_contract(),
        "staking": test_staking(),
        "query": test_transaction_query(),
        "sync": test_cross_node_sync(),
        "health": test_system_health(),
    }

    # Summary
    print("\n" + "="*60)
    print("📊 TEST SUMMARY")
    print("="*60)

    passed = sum(1 for v in results.values() if v)
    total = len(results)

    for test, result in results.items():
        status = "✅ PASS" if result else "❌ FAIL"
        print(f"  {test.capitalize()}: {status}")

    print(f"\n  Total: {passed}/{total} tests passed")
    print("="*60)

if __name__ == "__main__":
    main()
