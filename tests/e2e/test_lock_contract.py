"""
E2E Smart Contract Test: SimpleLock — Deploy, Lock, Unlock, Query
Tests on both node1 (validator, port 8545) and node3 (sync, port 8549)
"""
import json
import time
import sys
import urllib.request
from eth_account import Account
from eth_abi import encode, decode
from eth_utils import to_checksum_address

# ───────── Config ─────────
NODE1_PORT = 8545
NODE3_PORT = 8549
CHAIN_ID = 8898
DEV_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"

# ───────── Pre-compiled SimpleLock Contract ─────────
# Solidity source (for reference):
# pragma solidity ^0.8.0;
# contract SimpleLock {
#     mapping(address => uint256) public locked;
#     event Locked(address indexed user, uint256 amount);
#     event Unlocked(address indexed user, uint256 amount);
#     function lock() external payable {
#         require(msg.value > 0, "Must send value");
#         locked[msg.sender] += msg.value;
#         emit Locked(msg.sender, msg.value);
#     }
#     function unlock(uint256 amount) external {
#         require(locked[msg.sender] >= amount, "Insufficient locked");
#         locked[msg.sender] -= amount;
#         payable(msg.sender).transfer(amount);
#         emit Unlocked(msg.sender, amount);
#     }
#     function getLockedBalance(address user) external view returns (uint256) {
#         return locked[user];
#     }
# }

def compile_contract():
    """Compile the SimpleLock contract using solcx. Install solc if needed."""
    try:
        import solcx
    except ImportError:
        print("ERROR: py-solc-x not installed. Run: pip install py-solc-x")
        sys.exit(1)

    # Install solc 0.8.20 if not present
    installed = solcx.get_installed_solc_versions()
    target_version = "0.8.20"
    need_install = True
    for v in installed:
        if str(v).startswith("0.8"):
            target_version = str(v)
            need_install = False
            break

    if need_install:
        print(f"Installing solc {target_version}...")
        solcx.install_solc(target_version)
        print(f"solc {target_version} installed.")

    source = """
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract SimpleLock {
    mapping(address => uint256) public locked;

    event Locked(address indexed user, uint256 amount);
    event Unlocked(address indexed user, uint256 amount);

    function lock() external payable {
        require(msg.value > 0, "Must send value");
        locked[msg.sender] += msg.value;
        emit Locked(msg.sender, msg.value);
    }

    function unlock(uint256 amount) external {
        require(locked[msg.sender] >= amount, "Insufficient locked");
        locked[msg.sender] -= amount;
        payable(msg.sender).transfer(amount);
        emit Unlocked(msg.sender, amount);
    }

    function getLockedBalance(address user) external view returns (uint256) {
        return locked[user];
    }
}
"""
    compiled = solcx.compile_source(
        source,
        output_values=["abi", "bin"],
        solc_version=target_version,
    )
    contract_id, contract_interface = compiled.popitem()
    return contract_interface["bin"], contract_interface["abi"]


# ───────── ABI encoding helpers ─────────
# Function selectors (keccak256 first 4 bytes)
from eth_utils import keccak

def fn_selector(sig: str) -> bytes:
    return keccak(text=sig)[:4]

LOCK_SELECTOR    = fn_selector("lock()")
UNLOCK_SELECTOR  = fn_selector("unlock(uint256)")
GET_BAL_SELECTOR = fn_selector("getLockedBalance(address)")
LOCKED_SELECTOR  = fn_selector("locked(address)")  # public mapping getter


# ───────── RPC helpers ─────────
def rpc(method, params=None, port=NODE1_PORT):
    if params is None:
        params = []
    data = json.dumps({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).encode()
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}",
        data=data,
        headers={"Content-Type": "application/json"},
    )
    resp = urllib.request.urlopen(req, timeout=10)
    return json.loads(resp.read())


def get_nonce(address, port=NODE1_PORT):
    r = rpc("eth_getTransactionCount", [address, "latest"], port=port)
    return int(r["result"], 16)


def send_tx(tx_dict, private_key, port=NODE1_PORT):
    """Sign and send a transaction, return tx hash."""
    acct = Account.from_key(private_key)
    signed = acct.sign_transaction(tx_dict)
    raw = "0x" + signed.raw_transaction.hex()
    result = rpc("eth_sendRawTransaction", [raw], port=port)
    if "error" in result:
        raise RuntimeError(f"Send TX error: {result['error']}")
    return result["result"]


def wait_receipt(tx_hash, port=NODE1_PORT, timeout=30):
    """Wait for transaction to be mined and return receipt."""
    for _ in range(timeout // 2):
        time.sleep(2)
        r = rpc("eth_getTransactionReceipt", [tx_hash], port=port)
        if r.get("result"):
            return r["result"]
    return None


def eth_call(to, data_hex, port=NODE1_PORT):
    """Execute eth_call (read-only) and return result."""
    r = rpc("eth_call", [{"to": to, "data": data_hex}, "latest"], port=port)
    if "error" in r:
        raise RuntimeError(f"eth_call error: {r['error']}")
    return r["result"]


# ───────── Main flow ─────────
def main():
    print("=" * 64)
    print("  🔒 SimpleLock Contract E2E Test")
    print("=" * 64)

    # 0. Compile contract
    print("\n[0] Compiling SimpleLock contract...")
    bytecode, abi = compile_contract()
    print(f"    Bytecode length: {len(bytecode)} chars")
    print(f"    ABI functions: {[item['name'] for item in abi if item.get('type') == 'function']}")

    # Setup dev account
    dev = Account.from_key(DEV_KEY)
    print(f"\n    Dev account: {dev.address}")

    # Check balance on node1
    bal_r = rpc("eth_getBalance", [dev.address, "latest"])
    balance = int(bal_r["result"], 16)
    print(f"    Balance: {balance / 1e18:.4f} ETH")
    if balance == 0:
        print("ERROR: Dev account has 0 balance! Make sure dev_mode is on.")
        sys.exit(1)

    # ─────────────────────────────────────
    # 1. DEPLOY CONTRACT
    # ─────────────────────────────────────
    print("\n" + "─" * 64)
    print("[1] 📄 Deploying SimpleLock contract...")
    nonce = get_nonce(dev.address)
    deploy_tx = {
        "nonce": nonce,
        "value": 0,
        "gas": 2000000,
        "gasPrice": 1000000000,
        "chainId": CHAIN_ID,
        "data": "0x" + bytecode,
    }
    tx_hash = send_tx(deploy_tx, DEV_KEY)
    print(f"    Deploy TX: {tx_hash}")
    print("    Waiting for mining...")

    receipt = wait_receipt(tx_hash, timeout=30)
    if not receipt:
        print("ERROR: Deploy TX not mined within timeout!")
        sys.exit(1)

    raw_contract_addr = receipt.get("contractAddress")
    status = receipt.get("status")
    gas_used = receipt.get("gasUsed", "?")

    if not raw_contract_addr:
        print("ERROR: No contract address in receipt!")
        sys.exit(1)

    # Convert to checksum format for web3.py sign_transaction compatibility
    contract_addr = to_checksum_address(raw_contract_addr)
    print(f"    ✅ Contract deployed!")
    print(f"    Address: {contract_addr}")
    print(f"    Status:  {status}")
    print(f"    Gas:     {gas_used}")
    print(f"    Block:   {receipt.get('blockNumber')}")

    # Check contract code exists
    code_r = rpc("eth_getCode", [contract_addr, "latest"])
    code = code_r.get("result", "0x")
    print(f"    Code length: {len(code)} chars")
    if code == "0x" or code == "0x0" or len(code) < 10:
        print("WARNING: Contract code is empty — deployment may have failed internally")

    # ─────────────────────────────────────
    # 2. LOCK 1 ETH
    # ─────────────────────────────────────
    print("\n" + "─" * 64)
    lock_amount = 1 * 10**18  # 1 ETH
    print(f"[2] 🔒 Locking {lock_amount / 1e18} ETH...")

    nonce = get_nonce(dev.address)
    lock_tx = {
        "nonce": nonce,
        "to": contract_addr,
        "value": lock_amount,
        "gas": 100000,
        "gasPrice": 1000000000,
        "chainId": CHAIN_ID,
        "data": "0x" + LOCK_SELECTOR.hex(),
    }
    tx_hash = send_tx(lock_tx, DEV_KEY)
    print(f"    Lock TX: {tx_hash}")
    print("    Waiting for mining...")

    receipt = wait_receipt(tx_hash)
    if not receipt:
        print("ERROR: Lock TX not mined!")
        sys.exit(1)

    print(f"    ✅ Lock TX mined!")
    print(f"    Status: {receipt.get('status')}")
    print(f"    Gas:    {receipt.get('gasUsed')}")
    print(f"    Block:  {receipt.get('blockNumber')}")

    # Check logs for Locked event
    logs = receipt.get("logs", [])
    print(f"    Logs:   {len(logs)} event(s)")
    for i, log in enumerate(logs):
        print(f"      Log[{i}]: topics={log.get('topics', [])}")

    # ─────────────────────────────────────
    # 3. QUERY — getLockedBalance on node1
    # ─────────────────────────────────────
    print("\n" + "─" * 64)
    print("[3] 🔍 Querying locked balance on node1...")

    # Encode: getLockedBalance(address)
    call_data = "0x" + GET_BAL_SELECTOR.hex() + encode(["address"], [dev.address]).hex()
    result = eth_call(contract_addr, call_data, port=NODE1_PORT)
    locked_bal = int(result, 16) if result != "0x" else 0
    print(f"    Locked balance: {locked_bal / 1e18} ETH")

    if locked_bal != lock_amount:
        print(f"    ⚠️  Expected {lock_amount / 1e18} ETH, got {locked_bal / 1e18} ETH")
    else:
        print(f"    ✅ Correct!")

    # Also try the public mapping getter locked(address)
    call_data2 = "0x" + LOCKED_SELECTOR.hex() + encode(["address"], [dev.address]).hex()
    result2 = eth_call(contract_addr, call_data2, port=NODE1_PORT)
    locked_bal2 = int(result2, 16) if result2 != "0x" else 0
    print(f"    locked(address) = {locked_bal2 / 1e18} ETH")

    # ─────────────────────────────────────
    # 4. UNLOCK 0.3 ETH
    # ─────────────────────────────────────
    print("\n" + "─" * 64)
    unlock_amount = 3 * 10**17  # 0.3 ETH
    print(f"[4] 🔓 Unlocking {unlock_amount / 1e18} ETH...")

    nonce = get_nonce(dev.address)
    unlock_data = UNLOCK_SELECTOR + encode(["uint256"], [unlock_amount])
    unlock_tx = {
        "nonce": nonce,
        "to": contract_addr,
        "value": 0,
        "gas": 100000,
        "gasPrice": 1000000000,
        "chainId": CHAIN_ID,
        "data": "0x" + unlock_data.hex(),
    }
    tx_hash = send_tx(unlock_tx, DEV_KEY)
    print(f"    Unlock TX: {tx_hash}")
    print("    Waiting for mining...")

    receipt = wait_receipt(tx_hash)
    if not receipt:
        print("ERROR: Unlock TX not mined!")
        sys.exit(1)

    print(f"    ✅ Unlock TX mined!")
    print(f"    Status: {receipt.get('status')}")
    print(f"    Gas:    {receipt.get('gasUsed')}")
    print(f"    Block:  {receipt.get('blockNumber')}")

    logs = receipt.get("logs", [])
    print(f"    Logs:   {len(logs)} event(s)")

    # ─────────────────────────────────────
    # 5. QUERY — remaining locked balance
    # ─────────────────────────────────────
    print("\n" + "─" * 64)
    print("[5] 🔍 Querying remaining locked balance...")

    call_data = "0x" + GET_BAL_SELECTOR.hex() + encode(["address"], [dev.address]).hex()
    result = eth_call(contract_addr, call_data, port=NODE1_PORT)
    remaining = int(result, 16) if result != "0x" else 0
    expected_remaining = lock_amount - unlock_amount
    print(f"    Remaining locked: {remaining / 1e18} ETH")
    print(f"    Expected:         {expected_remaining / 1e18} ETH")

    if remaining == expected_remaining:
        print(f"    ✅ Correct!")
    else:
        print(f"    ⚠️  Mismatch!")

    # ─────────────────────────────────────
    # 6. VERIFY ON NODE3
    # ─────────────────────────────────────
    print("\n" + "─" * 64)
    print("[6] 🌐 Verifying on node3 (port 8549)...")

    try:
        # Wait a bit for P2P sync
        time.sleep(5)

        bn3 = rpc("eth_blockNumber", port=NODE3_PORT)
        print(f"    Node3 block: {bn3.get('result')}")

        # Check locked balance on node3
        result3 = eth_call(contract_addr, call_data, port=NODE3_PORT)
        locked3 = int(result3, 16) if result3 != "0x" else 0
        print(f"    Locked balance on node3: {locked3 / 1e18} ETH")

        if locked3 == expected_remaining:
            print(f"    ✅ Node3 state matches node1!")
        else:
            print(f"    ⚠️  Node3 mismatch (may need more sync time)")

        # Check TX receipt on node3
        r3 = rpc("eth_getTransactionReceipt", [tx_hash], port=NODE3_PORT)
        if r3.get("result"):
            print(f"    ✅ Unlock TX receipt found on node3!")
        else:
            print(f"    ⚠️  Unlock TX receipt not yet on node3")

    except Exception as e:
        print(f"    ⚠️  Node3 not available: {e}")

    # ─────────────────────────────────────
    # SUMMARY
    # ─────────────────────────────────────
    print("\n" + "=" * 64)
    print("  📊 SUMMARY")
    print("=" * 64)
    print(f"  Contract:     {contract_addr}")
    print(f"  Locked:       {lock_amount / 1e18} ETH")
    print(f"  Unlocked:     {unlock_amount / 1e18} ETH")
    print(f"  Remaining:    {remaining / 1e18} ETH (expected {expected_remaining / 1e18})")
    correct = remaining == expected_remaining
    print(f"  Status:       {'✅ ALL PASSED' if correct else '⚠️  ISSUES FOUND'}")
    print("=" * 64)

    sys.exit(0 if correct else 1)


if __name__ == "__main__":
    main()
