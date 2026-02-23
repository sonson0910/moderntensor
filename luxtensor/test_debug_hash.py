"""Debug test: compare eth_pendingTransactions hash with eth_getTransactionByHash"""
import requests, json, time, secrets
from eth_account import Account

NODE1 = "http://127.0.0.1:8545"
NODE3 = "http://127.0.0.1:8565"
CHAIN = 8898

KEY = "0x" + secrets.token_hex(32)
account = Account.from_key(KEY)

def rpc(url, method, params=None):
    r = requests.post(url, json={"jsonrpc":"2.0","method":method,"params":params or [],"id":1}, timeout=5)
    return r.json()

print(f"Sender: {account.address}")

tx = {"nonce": 0, "to": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      "value": 100, "gas": 21000, "gasPrice": 1000000000, "chainId": CHAIN}
signed = Account.sign_transaction(tx, KEY)
raw = "0x" + signed.raw_transaction.hex()

result = rpc(NODE1, "eth_sendRawTransaction", [raw])
print(f"Node1 send result: {json.dumps(result)}")

tx_hash = result.get("result")
if not tx_hash:
    print("TX send failed!")
    exit(1)

print(f"\n=== HASH COMPARISON ===")
print(f"TX hash from sendRaw: {tx_hash}")
print(f"TX hash length: {len(tx_hash)}")
print(f"TX hash without 0x: {tx_hash[2:] if tx_hash.startswith('0x') else tx_hash}")

# Node1: check immediately
r1_get = rpc(NODE1, "eth_getTransactionByHash", [tx_hash])
print(f"\nNode1 getByHash: {json.dumps(r1_get)[:200]}")

# Wait for propagation
print("\nWaiting 8s for P2P propagation...")
time.sleep(8)

# Node3: check pending
r3_pending = rpc(NODE3, "eth_pendingTransactions", [])
pending = r3_pending.get('result', [])
print(f"\nNode3 pending count: {len(pending) if isinstance(pending, list) else 'N/A'}")

if isinstance(pending, list) and len(pending) > 0:
    for i, ptx in enumerate(pending[:3]):
        pending_hash = ptx.get('hash', '?')
        print(f"  Pending TX[{i}] hash: {pending_hash}")
        print(f"  Match: {pending_hash == tx_hash}")
        print(f"  Pending hash bytes: {pending_hash[2:][:20]}...")
        print(f"  Lookup hash bytes: {tx_hash[2:][:20]}...")

        # Now try to get this specific pending hash
        r3_get = rpc(NODE3, "eth_getTransactionByHash", [pending_hash])
        r3_result = r3_get.get('result')
        print(f"  getByHash(pending_hash): {'FOUND' if r3_result else 'NULL'}")

        # Also try with our original hash
        r3_get2 = rpc(NODE3, "eth_getTransactionByHash", [tx_hash])
        r3_result2 = r3_get2.get('result')
        print(f"  getByHash(tx_hash):      {'FOUND' if r3_result2 else 'NULL'}")

        # Print full results for first one
        if i == 0:
            print(f"\n  Full getByHash response: {json.dumps(r3_get)[:500]}")
            print(f"  Full getByHash2 response: {json.dumps(r3_get2)[:500]}")
else:
    print("No pending transactions found!")
