"""
LuxTensor Devnet Transaction Test
=================================
1. Fund address via dev_faucet
2. Sign TX with private key (EIP-155)
3. Send via eth_sendRawTransaction to Node 1
4. Query from Node 3 to verify propagation
"""

import json
import requests
from eth_account import Account
from eth_account.signers.local import LocalAccount

# ================================================================
# CONFIGURATION
# ================================================================
NODE1_URL = "http://127.0.0.1:8545"
NODE3_URL = "http://127.0.0.1:8565"
CHAIN_ID = 8898

# ================================================================
# STEP 0: Create wallet (deterministic for reproducibility)
# ================================================================
print("=" * 60)
print("STEP 0: Create Wallet")
print("=" * 60)

# Use a known private key for testing
PRIVATE_KEY = "0x4c0883a69102937d6231471b5dbb6204fe512961708279f219a32b6a25f2f057"
account: LocalAccount = Account.from_key(PRIVATE_KEY)
SENDER = account.address

# Receiver address (another test address)
RECEIVER = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

print(f"  Sender:   {SENDER}")
print(f"  Receiver: {RECEIVER}")
print(f"  Chain ID: {CHAIN_ID}")

def rpc_call(url, method, params=None, req_id=1):
    """Helper for JSON-RPC calls"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": req_id,
    }
    resp = requests.post(url, json=payload, timeout=10)
    data = resp.json()
    if "error" in data:
        print(f"  ❌ Error: {data['error']}")
        return None
    return data.get("result")

# ================================================================
# STEP 1: Fund sender via dev_faucet
# ================================================================
print("\n" + "=" * 60)
print("STEP 1: Fund sender via dev_faucet (Node 1)")
print("=" * 60)

result = rpc_call(NODE1_URL, "dev_faucet", [SENDER])
if result:
    print(f"  ✅ Faucet success!")
    print(f"  Response: {json.dumps(result, indent=4)}")
else:
    print("  ⚠️ Faucet failed (maybe cooldown), continuing anyway...")

# ================================================================
# STEP 2: Check balance on Node 1
# ================================================================
print("\n" + "=" * 60)
print("STEP 2: Check balance (Node 1)")
print("=" * 60)

balance = rpc_call(NODE1_URL, "eth_getBalance", [SENDER, "latest"])
if balance:
    bal_int = int(balance, 16)
    print(f"  Balance: {balance} ({bal_int} wei = {bal_int / 1e18:.4f} ETH)")

# ================================================================
# STEP 3: Get nonce
# ================================================================
print("\n" + "=" * 60)
print("STEP 3: Get nonce (Node 1)")
print("=" * 60)

nonce_hex = rpc_call(NODE1_URL, "eth_getTransactionCount", [SENDER, "latest"])
nonce = int(nonce_hex, 16) if nonce_hex else 0
print(f"  Nonce: {nonce}")

# ================================================================
# STEP 4: Sign Transaction (EIP-155)
# ================================================================
print("\n" + "=" * 60)
print("STEP 4: Sign Transaction (EIP-155)")
print("=" * 60)

# Build transaction dict
tx = {
    "nonce": nonce,
    "to": RECEIVER,
    "value": 1_000_000_000_000_000_000,  # 1 ETH in wei
    "gas": 21000,
    "gasPrice": 1_000_000_000,  # 1 Gwei
    "chainId": CHAIN_ID,
    "data": b"",
}
print(f"  TX: {SENDER} → {RECEIVER}")
print(f"  Value: 1 ETH ({tx['value']} wei)")
print(f"  Gas: {tx['gas']}, GasPrice: {tx['gasPrice']}")
print(f"  Nonce: {tx['nonce']}, Chain ID: {tx['chainId']}")

# Sign with private key
signed = Account.sign_transaction(tx, PRIVATE_KEY)
raw_tx_hex = "0x" + signed.raw_transaction.hex()

print(f"  ✅ Signed!")
print(f"  TX Hash: {signed.hash.hex()}")
print(f"  v: {signed.v}")
print(f"  r: {hex(signed.r)}")
print(f"  s: {hex(signed.s)}")
print(f"  Raw TX: {raw_tx_hex[:80]}...")

# ================================================================
# STEP 5: Send Raw Transaction to Node 1
# ================================================================
print("\n" + "=" * 60)
print("STEP 5: eth_sendRawTransaction → Node 1")
print("=" * 60)

result = rpc_call(NODE1_URL, "eth_sendRawTransaction", [raw_tx_hex])
if result:
    print(f"  ✅ TX submitted! Hash: {result}")
    tx_hash = result
else:
    print("  ❌ Failed to submit transaction")
    tx_hash = None

# ================================================================
# STEP 6: Query TX from Node 1
# ================================================================
print("\n" + "=" * 60)
print("STEP 6: Query TX from Node 1")
print("=" * 60)

if tx_hash:
    tx_data = rpc_call(NODE1_URL, "eth_getTransactionByHash", [tx_hash])
    if tx_data:
        print(f"  ✅ TX found on Node 1!")
        print(f"  {json.dumps(tx_data, indent=4)}")
    else:
        print("  ❌ TX not found on Node 1")

# ================================================================
# STEP 7: Query TX from Node 3 (propagation test!)
# ================================================================
print("\n" + "=" * 60)
print("STEP 7: Query TX from Node 3 (P2P propagation)")
print("=" * 60)

if tx_hash:
    # Wait a moment for propagation
    import time
    print("  ⏳ Waiting 3 seconds for P2P propagation...")
    time.sleep(3)

    tx_data3 = rpc_call(NODE3_URL, "eth_getTransactionByHash", [tx_hash])
    if tx_data3:
        print(f"  ✅ TX FOUND ON NODE 3! P2P propagation works!")
        print(f"  {json.dumps(tx_data3, indent=4)}")
    else:
        print("  ❌ TX not found on Node 3 (propagation may take longer)")

        # Try pending transactions
        print("\n  Checking pending txpool on Node 3...")
        pending = rpc_call(NODE3_URL, "txpool_content", [])
        if pending:
            print(f"  TXPool: {json.dumps(pending, indent=2)[:500]}")

# ================================================================
# STEP 8: Check receiver balance on both nodes
# ================================================================
print("\n" + "=" * 60)
print("STEP 8: Check receiver balance")
print("=" * 60)

bal1 = rpc_call(NODE1_URL, "eth_getBalance", [RECEIVER, "latest"])
bal3 = rpc_call(NODE3_URL, "eth_getBalance", [RECEIVER, "latest"])

print(f"  Node 1 - Receiver balance: {bal1}")
print(f"  Node 3 - Receiver balance: {bal3}")

print("\n" + "=" * 60)
print("TEST COMPLETE")
print("=" * 60)
