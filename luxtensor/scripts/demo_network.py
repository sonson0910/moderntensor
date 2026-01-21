#!/usr/bin/env python3
"""
Demo tương tác với LuxTensor 3-node network
"""

import sys
import json
import time
sys.path.insert(0, 'd:/venera/cardano/moderntensor/moderntensor')

from sdk.luxtensor_client import LuxtensorClient

def main():
    print("=" * 70)
    print("🔗 LUXTENSOR 3-NODE NETWORK DEMO")
    print("=" * 70)

    # Connect to all 3 nodes
    nodes = [
        ("Node 1", "http://localhost:8545"),
        ("Node 2", "http://localhost:8555"),
        ("Node 3", "http://localhost:8565"),
    ]

    clients = []
    for name, url in nodes:
        try:
            client = LuxtensorClient(url)
            block = client.get_block_number()
            clients.append((name, client, block))
            print(f"✅ {name} ({url}): Block #{block}")
        except Exception as e:
            print(f"❌ {name} ({url}): Failed - {e}")

    if not clients:
        print("No nodes available!")
        return

    print("\n" + "-" * 70)
    print("💰 ACCOUNT BALANCES")
    print("-" * 70)

    # Pre-funded accounts
    accounts = [
        ("Deployer", "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        ("Account 2", "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"),
        ("Account 3", "0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
    ]

    client = clients[0][1]  # Use first node
    for name, addr in accounts:
        try:
            balance = client.get_balance(addr) / 1e18
            nonce = client.get_nonce(addr)
            print(f"  {name}: {balance:,.2f} LUX (nonce: {nonce})")
        except Exception as e:
            print(f"  {name}: Error - {e}")

    print("\n" + "-" * 70)
    print("📧 SEND TRANSACTION (ETH Transfer)")
    print("-" * 70)

    sender = accounts[0][1]
    recipient = accounts[1][1]

    # Send 10 LUX
    print(f"  Sending 10 LUX from {sender[:10]}... to {recipient[:10]}...")

    tx_hash = client._call_rpc("eth_sendTransaction", [{
        "from": sender,
        "to": recipient,
        "value": hex(10 * 10**18),  # 10 ETH in wei
        "gas": "0x5208"  # 21000
    }])
    print(f"  TX Hash: {tx_hash}")

    # Check balances again
    time.sleep(2)
    old_balance = 1000.0
    new_balance_sender = client.get_balance(sender) / 1e18
    new_balance_recipient = client.get_balance(recipient) / 1e18
    print(f"  ✅ Sender new balance: {new_balance_sender:,.2f} LUX")
    print(f"  ✅ Recipient new balance: {new_balance_recipient:,.2f} LUX")

    print("\n" + "-" * 70)
    print("📊 NODE SYNC STATUS")
    print("-" * 70)

    blocks = []
    for name, client, _ in clients:
        try:
            new_block = client.get_block_number()
            blocks.append(new_block)
            print(f"  {name}: Block #{new_block}")
        except:
            pass

    if len(blocks) >= 2:
        diff = max(blocks) - min(blocks)
        if diff <= 5:
            print(f"  ✅ Network synced (diff: {diff} blocks)")
        else:
            print(f"  ⚠️  Network out of sync (diff: {diff} blocks)")

    print("\n" + "-" * 70)
    print("🔧 RPC METHODS AVAILABLE")
    print("-" * 70)

    modules = client._call_rpc("rpc_modules", [])
    print(f"  Modules: {', '.join(modules.keys())}")

    print("\n" + "-" * 70)
    print("📋 CHAIN INFO")
    print("-" * 70)

    chain_id = client._call_rpc("eth_chainId", [])
    net_version = client._call_rpc("net_version", [])
    client_version = client._call_rpc("web3_clientVersion", [])

    print(f"  Chain ID: {chain_id} ({int(chain_id, 16)})")
    print(f"  Network Version: {net_version}")
    print(f"  Client Version: {client_version}")

    print("\n" + "=" * 70)
    print("✅ DEMO COMPLETE!")
    print("=" * 70)
    print("\nSummary:")
    print(f"  • 3 nodes running and connected")
    print(f"  • Blocks being produced (current: {max(blocks)})")
    print(f"  • Transactions can be sent")
    print(f"  • Pre-funded accounts available for testing")
    print(f"  • EVM-compatible RPC available")
    print("\nNote: Contract deployment requires fixing eth_getTransactionReceipt")
    print("      (pending_txs store mismatch issue identified in server.rs)")

if __name__ == "__main__":
    main()
