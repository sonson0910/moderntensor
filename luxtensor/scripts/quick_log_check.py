#!/usr/bin/env python3
"""Quick test to check node log file exists and has content"""

import os
import sys
import time
import shutil
import tempfile
import subprocess
from pathlib import Path

if sys.platform == "win32":
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

import requests


def main():
    binary = Path(__file__).parent.parent / "target" / "release" / "luxtensor-node.exe"
    temp = Path(tempfile.mkdtemp(prefix="quick_"))
    node_dir = temp / "node"
    node_dir.mkdir(parents=True)
    (node_dir / "db").mkdir()

    config = f"""
[node]
name = "test"
chain_id = 1337
data_dir = "{node_dir.as_posix()}"
is_validator = true
validator_id = "test"
dao_address = "0xDAO0000000000000000000000000000000000001"
dev_mode = true

[consensus]
block_time = 2
epoch_length = 10
min_stake = "1000000000000000000"
max_validators = 10
gas_limit = 30000000
validators = ["test"]

[network]
listen_addr = "0.0.0.0"
listen_port = 30600
bootstrap_nodes = []
max_peers = 50
enable_mdns = false

[storage]
db_path = "{(node_dir / 'db').as_posix()}"
enable_compression = true
max_open_files = 256
cache_size = 64

[rpc]
enabled = true
listen_addr = "127.0.0.1"
listen_port = 9300
threads = 2
cors_origins = ["*"]

[logging]
level = "debug"
log_to_file = true
log_file = "{(node_dir / 'node.log').as_posix()}"
json_format = false
"""
    (node_dir / "config.toml").write_text(config)

    print("[START] Node...")
    stdout = open(node_dir / "stdout.log", "w")
    proc = subprocess.Popen([str(binary), "--config", str(node_dir / "config.toml")],
                           stdout=stdout, stderr=subprocess.STDOUT, cwd=str(node_dir))

    time.sleep(8)

    # Send TX
    try:
        r = requests.post("http://127.0.0.1:9300",
                         json={"jsonrpc": "2.0", "method": "eth_sendTransaction",
                               "params": [{"from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                                          "data": "0x60806040", "gas": "0x100000"}], "id": 1}, timeout=5)
        print(f"[TX] Response: {r.json()}")
    except Exception as e:
        print(f"[TX] Error: {e}")

    time.sleep(10)

    proc.terminate()
    time.sleep(1)

    # Check log
    log_path = node_dir / "node.log"
    stdout_path = node_dir / "stdout.log"

    print(f"\n[LOG FILE] {log_path}")
    print(f"  Exists: {log_path.exists()}")
    if log_path.exists():
        print(f"  Size: {log_path.stat().st_size} bytes")
        print("\n[LOG CONTENT] (last 50 lines):")
        with open(log_path, "r", errors="replace") as f:
            lines = f.readlines()
            for line in lines[-50:]:
                print(line.rstrip())

    print(f"\n[STDOUT FILE] {stdout_path}")
    if stdout_path.exists():
        print(f"  Size: {stdout_path.stat().st_size} bytes")
        print("\n[SEARCH STDOUT FOR TX FLOW]:")
        with open(stdout_path, "r", errors="replace") as f:
            for i, line in enumerate(f, 1):
                line_lower = line.lower()
                if any(kw in line_lower for kw in ["queue", "drain", "sign", "mempool", "produced block", "execute", "contract", "deploy", "failed", "error"]) or \
                   any(emoji in line for emoji in ["📤", "📦", "📥", "🔑", "📄", "❌", "✅"]):
                    print(f"  L{i}: {line.rstrip()[:180]}")

    shutil.rmtree(temp, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
