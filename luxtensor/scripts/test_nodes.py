#!/usr/bin/env python3
"""
Deploy and interact with MDTToken on LuxTensor using SDK
"""

import sys
import json
sys.path.insert(0, 'd:/venera/cardano/moderntensor/moderntensor')

from sdk.luxtensor_client import LuxtensorClient

# MDTToken ABI (essential functions)
MDT_TOKEN_ABI = [
    {"name": "name", "type": "function", "inputs": [], "outputs": [{"type": "string"}]},
    {"name": "symbol", "type": "function", "inputs": [], "outputs": [{"type": "string"}]},
    {"name": "totalSupply", "type": "function", "inputs": [], "outputs": [{"type": "uint256"}]},
    {"name": "balanceOf", "type": "function", "inputs": [{"name": "account", "type": "address"}], "outputs": [{"type": "uint256"}]},
    {"name": "transfer", "type": "function", "inputs": [{"name": "to", "type": "address"}, {"name": "amount", "type": "uint256"}], "outputs": [{"type": "bool"}]},
]

def main():
    print("=" * 60)
    print("🔗 LuxTensor Smart Contract Interaction")
    print("=" * 60)

    # Connect to nodes
    client1 = LuxtensorClient("http://localhost:8545")
    client2 = LuxtensorClient("http://localhost:8555")
    client3 = LuxtensorClient("http://localhost:8565")

    # Check connection
    print("\n📊 Node Status:")
    for i, client in enumerate([client1, client2, client3], 1):
        try:
            block = client.get_block_number()
            print(f"  Node {i} (port {8545 + (i-1)*10}): Block #{block}")
        except Exception as e:
            print(f"  Node {i}: Error - {e}")

    # Get pre-funded accounts
    print("\n💰 Pre-funded Accounts:")
    deployer = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    balance = client1.get_balance(deployer)
    print(f"  {deployer[:10]}...: {balance / 1e18:.2f} LUX")

    # Check validator status
    print("\n🔐 Validator Status:")
    validators = client1.get_validators()
    print(f"  Active validators: {len(validators)}")
    for v in validators[:5]:
        print(f"    - {v.get('address', 'N/A')[:10]}...")

    # Check stake
    print("\n📈 Staking Info:")
    total_stake = client1.get_total_stake()
    print(f"  Total stake: {total_stake / 1e18:.2f} MDT")

    # Get AI task status (if any)
    print("\n🤖 AI Task Queue:")
    try:
        # Custom RPC for AI tasks
        result = client1._call_rpc("lux_getPendingAITasks", [])
        print(f"  Pending tasks: {len(result) if result else 0}")
    except Exception as e:
        print(f"  AI tasks not enabled or error: {e}")

    # Network sync status
    print("\n🔄 Network Sync:")
    blocks = []
    for i, client in enumerate([client1, client2, client3], 1):
        try:
            block = client.get_block_number()
            blocks.append(block)
        except:
            blocks.append(0)

    if max(blocks) - min(blocks) <= 2:
        print(f"  ✅ Nodes synced (blocks: {blocks})")
    else:
        print(f"  ⚠️ Nodes out of sync (blocks: {blocks})")

    print("\n" + "=" * 60)
    print("✅ LuxTensor network is running!")
    print("=" * 60)

if __name__ == "__main__":
    main()
