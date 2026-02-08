#!/usr/bin/env python3
"""
Debug Script: Check signing flow by examining node logs
"""

import os
import sys
import time
import json
import shutil
import tempfile
import subprocess
from pathlib import Path

if sys.platform == "win32":
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

import requests

CHAIN_ID = 8898
DEPLOYER = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
BYTECODE = "0x60806040526000805534801561001457600080fd5b50610150806100246000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80632e64cec11461003b5780636057361d14610059575b600080fd5b610043610075565b60405161005091906100a1565b60405180910390f35b610073600480360381019061006e91906100ed565b61007e565b005b60008054905090565b8060008190555050565b6000819050919050565b61009b81610088565b82525050565b60006020820190506100b66000830184610092565b92915050565b600080fd5b6100ca81610088565b81146100d557600080fd5b50565b6000813590506100e7816100c1565b92915050565b600060208284031215610103576101026100bc565b5b6000610111848285016100d8565b9150509291505056fea2"


def rpc(url, method, params=None):
    try:
        r = requests.post(url, json={"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}, timeout=10)
        return r.json().get("result")
    except:
        return None


def main():
    print("=" * 70)
    print("  SIGNING DEBUG TEST")
    print("=" * 70)

    binary = Path(__file__).parent.parent / "target" / "release" / "luxtensor-node.exe"
    if not binary.exists():
        print("[ERROR] Binary not found")
        return 1

    temp = Path(tempfile.mkdtemp(prefix="sign_debug_"))
    node_dir = temp / "node"
    node_dir.mkdir(parents=True)
    (node_dir / "db").mkdir()

    config = f"""
[node]
name = "debug-node"
chain_id = {CHAIN_ID}
data_dir = "{node_dir.as_posix()}"
is_validator = true
validator_id = "debug-node"
dao_address = "0xDAO0000000000000000000000000000000000001"

[consensus]
block_time = 2
epoch_length = 10
min_stake = "1000000000000000000"
max_validators = 10
validators = ["debug-node"]

[network]
listen_port = 30500
enable_mdns = false

[storage]
db_path = "{(node_dir / 'db').as_posix()}"

[rpc]
enabled = true
listen_port = 9200

[logging]
level = "debug"
log_to_file = true
log_file = "{(node_dir / 'debug.log').as_posix()}"
"""
    (node_dir / "config.toml").write_text(config)

    print(f"[CONFIG] chain_id = {CHAIN_ID}")

    log_file = open(node_dir / "stdout.log", "w")
    proc = subprocess.Popen([str(binary), "--config", str(node_dir / "config.toml")],
                           stdout=log_file, stderr=subprocess.STDOUT, cwd=str(node_dir))

    time.sleep(10)

    url = "http://127.0.0.1:9200"
    block = rpc(url, "eth_blockNumber")
    print(f"[OK] Node block #{int(block, 16) if block else 'N/A'}")

    print("\n[TX] Sending simple storage contract...")
    tx_hash = rpc(url, "eth_sendTransaction", [{"from": DEPLOYER, "data": BYTECODE, "gas": hex(500000)}])
    print(f"  Hash: {tx_hash}")

    print("\n[WAIT] Waiting 12s for blocks and mining...")
    time.sleep(12)

    receipt = rpc(url, "eth_getTransactionReceipt", [tx_hash])
    if receipt:
        addr = receipt.get("contractAddress")
        print(f"\n[RECEIPT]")
        print(f"  Contract: {addr}")
        print(f"  Status: {receipt.get('status')}")
        print(f"  Block: {receipt.get('blockNumber')}")

        if addr:
            code = rpc(url, "eth_getCode", [addr, "latest"])
            print(f"  Code: {len(code or '') - 2 if code else 0} chars")

    proc.terminate()
    time.sleep(2)

    print("\n" + "=" * 70)
    print("  KEY LOG ENTRIES (signing, mempool, executor)")
    print("=" * 70 + "\n")

    log_path = node_dir / "debug.log"
    keywords = ["sign", "mempool", "transaction added", "execute", "contract", "deploy",
                "failed", "error", "invalid", "chain_id", "pending", "block #"]

    if log_path.exists():
        with open(log_path, "r", errors="replace") as f:
            for line in f:
                line_lower = line.lower()
                if any(kw in line_lower for kw in keywords):
                    print(line.rstrip()[:200])

    shutil.rmtree(temp, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
