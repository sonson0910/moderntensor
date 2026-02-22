#!/usr/bin/env python3
"""
Production-like Smart Contract Test: Deploy → Lock → Unlock → Query (Node 3)

Uses proper wallet (secp256k1) + EIP-155 signed transactions + eth_sendRawTransaction.
Identical to MetaMask / ethers.js production workflow.

Flow:
  1. Generate random wallet (private key → address)
  2. Fund wallet via dev_faucet (only dev convenience)
  3. Deploy Vault contract with signed tx
  4. Lock value with signed tx
  5. Query on Node 1 & Node 3
  6. Unlock with signed tx
  7. Query again on both nodes
"""

import json, time, sys, urllib.request
from eth_account import Account
from eth_utils import to_checksum_address

# ═══════════════════════════════════════════════════════════════════
#  Network Config
# ═══════════════════════════════════════════════════════════════════
NODE1_URL = "http://127.0.0.1:8545"   # Validator
NODE3_URL = "http://127.0.0.1:8549"   # Follower
CHAIN_ID  = 8898                       # LuxTensor Devnet

# ═══════════════════════════════════════════════════════════════════
#  Wallet — real secp256k1 key pair
# ═══════════════════════════════════════════════════════════════════
wallet = Account.create()
PRIVATE_KEY = wallet.key
ADDRESS = wallet.address

print(f"🔑 Wallet Generated (secp256k1):")
print(f"   Address:     {ADDRESS}")
print(f"   Private Key: {wallet.key.hex()[:12]}...{wallet.key.hex()[-8:]}")


# ═══════════════════════════════════════════════════════════════════
#  Helpers
# ═══════════════════════════════════════════════════════════════════
def rpc(url, method, params=None):
    payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}
    req = urllib.request.Request(
        url, data=json.dumps(payload).encode(), headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=15) as resp:
        body = json.loads(resp.read().decode())
    if "error" in body:
        raise Exception(f"RPC error: {body['error']}")
    return body.get("result")


def wait_tx(url, tx_hash, timeout=30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        r = rpc(url, "eth_getTransactionReceipt", [tx_hash])
        if r is not None:
            return r
        time.sleep(1)
    raise TimeoutError(f"Tx {tx_hash} not mined in {timeout}s")


def hx(v):
    return int(v, 16) if isinstance(v, str) and v else 0


def poll(fn, check, timeout=20):
    deadline = time.time() + timeout
    last = None
    while time.time() < deadline:
        last = fn()
        if check(last):
            return last
        time.sleep(2)
    return last


def get_nonce(url, address):
    result = rpc(url, "eth_getTransactionCount", [address, "latest"])
    return hx(result)


def sign_and_send(url, tx_dict, private_key):
    """Sign tx with secp256k1 + EIP-155 and submit via eth_sendRawTransaction."""
    signed = Account.sign_transaction(tx_dict, private_key)
    raw_hex = "0x" + signed.raw_transaction.hex()
    tx_hash = rpc(url, "eth_sendRawTransaction", [raw_hex])
    return tx_hash


# ═══════════════════════════════════════════════════════════════════
#  EVM Bytecodes (hand-assembled)
# ═══════════════════════════════════════════════════════════════════
# Vault contract: slot[0] = stored value
#   set(uint256): 0x60fe47b1 → SSTORE(0, calldata[4..36])
#   reset():      0xd826f88f → SSTORE(0, 0)
#   fallback:     return SLOAD(0)
RUNTIME = (
    "6004361060335760003560e01c8063" "60fe47b1" "14603e57"
    "8063" "d826f88f" "14604757" "50"
    "5b60005460005260206000f3"
    "5b50600435600055" "00"
    "5b506000600055" "00"
)
rt_bytes = bytes.fromhex(RUNTIME)
INIT_CODE = bytes.fromhex(
    f"60{len(rt_bytes):02x}" + "80" + f"60{10:02x}" + "6000" + "39" + "6000" + "f3"
) + rt_bytes


# ═══════════════════════════════════════════════════════════════════
#  Test Flow
# ═══════════════════════════════════════════════════════════════════
def main():
    print("\n" + "=" * 64)
    print("  🔐 Production-style Smart Contract Test")
    print("  Wallet → Sign → Deploy → Lock → Unlock → Query")
    print("=" * 64)

    # 0. Health check
    print("\n📡 Checking nodes...")
    for name, url in [("Node 1 (validator)", NODE1_URL), ("Node 3 (follower)", NODE3_URL)]:
        try:
            bn = rpc(url, "eth_blockNumber")
            cid = rpc(url, "eth_chainId") if url == NODE1_URL else None
            print(f"  ✅ {name}: block #{hx(bn)}" + (f", chain_id={hx(cid)}" if cid else ""))
        except Exception as e:
            print(f"  ❌ {name}: {e}")
            return 1

    # 1. Fund wallet
    print(f"\n💰 Step 1: Fund wallet via faucet...")
    rpc(NODE1_URL, "dev_faucet", [ADDRESS, "0xDE0B6B3A7640000"])  # 1 ETH
    time.sleep(5)
    bal = poll(lambda: rpc(NODE1_URL, "eth_getBalance", [ADDRESS, "latest"]),
               lambda b: hx(b) > 0, timeout=15)
    bal_val = hx(bal)
    print(f"  ✅ Balance: {bal_val:,} wei ({bal_val / 1e18:.4f} ETH)")

    if bal_val == 0:
        print("  ❌ Faucet failed — no balance!")
        return 1

    # 2. Deploy contract (SIGNED tx)
    print("\n📦 Step 2: Deploy Vault contract (signed EIP-155 tx)...")
    nonce = get_nonce(NODE1_URL, ADDRESS)
    deploy_tx = {
        "nonce": nonce,
        "gasPrice": 1_000_000_000,
        "gas": 200_000,
        "to": b"",              # empty bytes = contract creation
        "value": 0,
        "data": INIT_CODE,
        "chainId": CHAIN_ID,
    }
    tx_hash = sign_and_send(NODE1_URL, deploy_tx, PRIVATE_KEY)
    print(f"  📤 Deploy tx hash:  {tx_hash}")
    print(f"     Signer: {ADDRESS}, Nonce: {nonce}")

    receipt = wait_tx(NODE1_URL, tx_hash)
    contract_addr = receipt.get("contractAddress")
    print(f"  ✅ Contract: {contract_addr}")
    print(f"     Block: {receipt.get('blockNumber')}, Status: {receipt.get('status')}")

    if not contract_addr:
        print("  ❌ No contract address!")
        return 1

    # Checksum the contract address for subsequent calls
    contract_cs = to_checksum_address(contract_addr)

    # 3. Lock 1,000,000 (SIGNED tx)
    LOCK_VAL = 1_000_000
    print(f"\n🔒 Step 3: Lock {LOCK_VAL:,} (signed tx)...")
    nonce = get_nonce(NODE1_URL, ADDRESS)
    lock_data = bytes.fromhex("60fe47b1") + LOCK_VAL.to_bytes(32, "big")
    lock_tx = {
        "nonce": nonce,
        "gasPrice": 1_000_000_000,
        "gas": 100_000,
        "to": contract_cs,
        "value": 0,
        "data": lock_data,
        "chainId": CHAIN_ID,
    }
    tx_hash = sign_and_send(NODE1_URL, lock_tx, PRIVATE_KEY)
    print(f"  📤 Lock tx: {tx_hash}")
    receipt = wait_tx(NODE1_URL, tx_hash)
    print(f"  ✅ Locked in block {receipt.get('blockNumber')}")

    # 4. Read on Node 1
    print("\n🔍 Step 4: Read locked value — Node 1...")
    v1 = poll(lambda: rpc(NODE1_URL, "eth_getStorageAt", [contract_addr, "0x0", "latest"]),
              lambda r: hx(r) == LOCK_VAL)
    v1i = hx(v1)
    print(f"  📊 slot[0] = {v1i:,} {'✅' if v1i == LOCK_VAL else '❌'}")

    # 5. Read on Node 3 (cross-node sync)
    print("\n🌐 Step 5: Read locked value — Node 3 (cross-node)...")
    time.sleep(5)
    v3 = poll(lambda: rpc(NODE3_URL, "eth_getStorageAt", [contract_addr, "0x0", "latest"]),
              lambda r: hx(r) == LOCK_VAL, timeout=25)
    v3i = hx(v3)
    print(f"  📊 slot[0] = {v3i:,} {'✅' if v3i == LOCK_VAL else '⏳'}")

    # 6. Unlock (SIGNED tx)
    print(f"\n🔓 Step 6: Unlock (signed tx)...")
    nonce = get_nonce(NODE1_URL, ADDRESS)
    unlock_tx = {
        "nonce": nonce,
        "gasPrice": 1_000_000_000,
        "gas": 100_000,
        "to": contract_cs,
        "value": 0,
        "data": bytes.fromhex("d826f88f"),
        "chainId": CHAIN_ID,
    }
    tx_hash = sign_and_send(NODE1_URL, unlock_tx, PRIVATE_KEY)
    print(f"  📤 Unlock tx: {tx_hash}")
    receipt = wait_tx(NODE1_URL, tx_hash)
    print(f"  ✅ Unlocked in block {receipt.get('blockNumber')}")

    # 7. Read after unlock — Node 1
    print("\n🔍 Step 7: Read after unlock — Node 1...")
    u1 = poll(lambda: rpc(NODE1_URL, "eth_getStorageAt", [contract_addr, "0x0", "latest"]),
              lambda r: hx(r) == 0)
    u1i = hx(u1)
    print(f"  📊 slot[0] = {u1i} {'✅ unlocked!' if u1i == 0 else '❌'}")

    # 8. Read after unlock — Node 3
    print("\n🌐 Step 8: Read after unlock — Node 3 (cross-node)...")
    time.sleep(5)
    u3 = poll(lambda: rpc(NODE3_URL, "eth_getStorageAt", [contract_addr, "0x0", "latest"]),
              lambda r: hx(r) == 0, timeout=25)
    u3i = hx(u3)
    print(f"  📊 slot[0] = {u3i} {'✅ unlocked!' if u3i == 0 else '⏳'}")

    # ═══════════════════════════════════════════════════════════════
    #  Summary
    # ═══════════════════════════════════════════════════════════════
    print("\n" + "=" * 64)
    print("  📋 TEST RESULTS")
    print("=" * 64)
    print(f"  Wallet:     {ADDRESS}")
    print(f"  Contract:   {contract_addr}")
    print(f"  Chain ID:   {CHAIN_ID}")
    print(f"  Signing:    secp256k1 (EIP-155 Legacy TX)")
    print(f"  Transport:  eth_sendRawTransaction")
    print()
    print(f"  Lock({LOCK_VAL:,})  → Node1={v1i:>10,} {'✅' if v1i==LOCK_VAL else '❌'}   "
          f"Node3={v3i:>10,} {'✅' if v3i==LOCK_VAL else '❌'}")
    print(f"  Unlock(0)  → Node1={u1i:>10} {'✅' if u1i==0 else '❌'}   "
          f"Node3={u3i:>10} {'✅' if u3i==0 else '❌'}")

    ok = v1i == LOCK_VAL and v3i == LOCK_VAL and u1i == 0 and u3i == 0
    print(f"\n  {'🎉 ALL CHECKS PASSED!' if ok else '⚠️ Some checks failed'}")
    print("=" * 64)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
