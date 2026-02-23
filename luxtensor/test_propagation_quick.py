"""Quick propagation test: send TX to Node1, query from Node3"""
import requests, json, time, secrets
from eth_account import Account

NODE1 = "http://127.0.0.1:8545"
NODE3 = "http://127.0.0.1:8565"
CHAIN = 8898

# Use a fresh random key each time to avoid nonce/duplicate issues
KEY = "0x" + secrets.token_hex(32)
account = Account.from_key(KEY)

def rpc(url, method, params=None):
    r = requests.post(url, json={"jsonrpc":"2.0","method":method,"params":params or [],"id":1}, timeout=5)
    return r.json()

print(f"Sender: {account.address}")

# Sign + send (nonce always 0 for fresh account)
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

# Immediately query Node 1
r1 = rpc(NODE1, "eth_getTransactionByHash", [tx_hash])
node1_found = r1.get("result") is not None
print(f"Node1 has TX: {node1_found}")

# Wait and query Node 3 with increasing delays
total_wait = 0
for delay in [1, 2, 3, 5, 5]:
    time.sleep(delay)
    total_wait += delay
    r3 = rpc(NODE3, "eth_getTransactionByHash", [tx_hash])
    node3_found = r3.get("result") is not None
    print(f"Node3 after {total_wait}s: found={node3_found}")
    if node3_found:
        print("PROPAGATION SUCCESS!")
        exit(0)

print(f"PROPAGATION FAILED after {total_wait}s")
# Debug: check what Node3 has
try:
    r = rpc(NODE3, "eth_pendingTransactions", [])
    pending = r.get('result', [])
    print(f"Node3 pending count: {len(pending) if isinstance(pending, list) else 'N/A'}")
    if isinstance(pending, list):
        hashes = [t.get('hash', '?') for t in pending[:5]]
        print(f"Node3 pending hashes: {hashes}")
        print(f"Looking for: {tx_hash}")
except Exception as e:
    print(f"Error checking pending: {e}")
