#!/usr/bin/env python3
"""
Transaction Scanner - Shows detailed transaction and contract info

This script starts a testnet and displays detailed transaction information.
"""

import os
import sys
import time
import json
import signal
import shutil
import tempfile
import subprocess
from pathlib import Path
from typing import List, Dict, Any
from dataclasses import dataclass

# Fix Windows encoding
if sys.platform == "win32":
    import io
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8', errors='replace')

import requests

# =============================================================================
# CONFIGURATION
# =============================================================================

@dataclass
class NodeConfig:
    name: str
    p2p_port: int
    rpc_port: int
    data_dir: Path
    is_validator: bool


class RpcClient:
    def __init__(self, url: str, name: str = ""):
        self.url = url
        self.name = name
        self.request_id = 0

    def call(self, method: str, params: List = None) -> Any:
        self.request_id += 1
        payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": self.request_id}
        try:
            resp = requests.post(self.url, json=payload, timeout=30)
            result = resp.json()
            if "error" in result:
                return None
            return result.get("result")
        except:
            return None

    def get_block_number(self) -> int:
        r = self.call("eth_blockNumber")
        if isinstance(r, str) and r.startswith("0x"):
            return int(r, 16)
        return int(r) if r else 0

    def get_block(self, number: int) -> Dict:
        hex_num = hex(number)
        return self.call("eth_getBlockByNumber", [hex_num, True])

    def get_transaction(self, tx_hash: str) -> Dict:
        return self.call("eth_getTransactionByHash", [tx_hash])

    def get_receipt(self, tx_hash: str) -> Dict:
        return self.call("eth_getTransactionReceipt", [tx_hash])

    def get_balance(self, addr: str) -> int:
        r = self.call("eth_getBalance", [addr, "latest"])
        if isinstance(r, str) and r.startswith("0x"):
            return int(r, 16)
        return int(r) if r else 0

    def get_code(self, addr: str) -> str:
        return self.call("eth_getCode", [addr, "latest"]) or "0x"

    def send_transaction(self, tx: Dict) -> str:
        return self.call("eth_sendTransaction", [tx])


# =============================================================================
# TESTNET MANAGER
# =============================================================================

class TestnetManager:
    def __init__(self):
        self.processes = []
        self.temp_dirs = []
        self.clients: Dict[str, RpcClient] = {}
        signal.signal(signal.SIGINT, lambda s,f: self.cleanup())

    def _find_binary(self) -> Path:
        paths = [
            Path(__file__).parent.parent / "target" / "release" / "luxtensor-node.exe",
            Path(__file__).parent.parent / "target" / "release" / "luxtensor-node",
            Path(__file__).parent.parent / "target" / "debug" / "luxtensor-node.exe",
        ]
        for p in paths:
            if p.exists():
                return p
        raise FileNotFoundError("Binary not found")

    def _create_config(self, node: NodeConfig) -> Path:
        config = f"""
[node]
name = "{node.name}"
chain_id = 1337
data_dir = "{node.data_dir.as_posix()}"
is_validator = {str(node.is_validator).lower()}
validator_id = "{node.name}"
dao_address = "0xDAO0000000000000000000000000000000000001"

[consensus]
block_time = 3
epoch_length = 10
min_stake = "1000000000000000000"
max_validators = 10
gas_limit = 30000000
validators = ["validator-a", "miner-b", "miner-c"]

[network]
listen_addr = "0.0.0.0"
listen_port = {node.p2p_port}
bootstrap_nodes = []
max_peers = 50
enable_mdns = true

[storage]
db_path = "{(node.data_dir / 'db').as_posix()}"
enable_compression = true
max_open_files = 256
cache_size = 64

[rpc]
enabled = true
listen_addr = "127.0.0.1"
listen_port = {node.rpc_port}
threads = 2
cors_origins = ["*"]

[logging]
level = "info"
log_to_file = true
log_file = "{(node.data_dir / 'node.log').as_posix()}"
json_format = false
"""
        config_path = node.data_dir / "config.toml"
        config_path.write_text(config)
        return config_path

    def start(self) -> bool:
        print("=" * 70)
        print("🚀 STARTING LOCAL TESTNET")
        print("=" * 70)

        try:
            binary = self._find_binary()
            print(f"✅ Binary: {binary.name}")
        except:
            print("❌ Binary not found!")
            return False

        base_temp = Path(tempfile.mkdtemp(prefix="luxtensor_scan_"))
        self.temp_dirs.append(base_temp)

        nodes = [
            NodeConfig("validator-a", 30300, 9000, base_temp / "node_a", True),
            NodeConfig("miner-b", 30301, 9001, base_temp / "node_b", False),
            NodeConfig("miner-c", 30302, 9002, base_temp / "node_c", False),
        ]

        for node in nodes:
            node.data_dir.mkdir(parents=True, exist_ok=True)
            (node.data_dir / "db").mkdir(exist_ok=True)
            config = self._create_config(node)

            print(f"📦 Starting {node.name} @ RPC:{node.rpc_port}")

            log = open(node.data_dir / "stdout.log", "w")
            proc = subprocess.Popen(
                [str(binary), "--config", str(config)],
                stdout=log, stderr=subprocess.STDOUT,
                cwd=str(node.data_dir)
            )
            self.processes.append(proc)
            self.clients[node.name] = RpcClient(f"http://127.0.0.1:{node.rpc_port}", node.name)
            time.sleep(1)

        print("\n⏳ Waiting 12s for initialization...")
        time.sleep(12)
        return True

    def cleanup(self):
        print("\n🧹 Cleaning up...")
        for p in self.processes:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=5)
                except:
                    p.kill()
        for d in self.temp_dirs:
            try:
                shutil.rmtree(d)
            except:
                pass
        print("✅ Done")


# =============================================================================
# SCANNER
# =============================================================================

def format_wei(wei: int) -> str:
    """Format wei to human readable"""
    if wei >= 10**18:
        return f"{wei / 10**18:.4f} MDT"
    elif wei >= 10**9:
        return f"{wei / 10**9:.4f} Gwei"
    return f"{wei} Wei"


def scan_and_display(clients: Dict[str, RpcClient]):
    """Scan blockchain and display details"""
    print("\n" + "=" * 70)
    print("📊 BLOCKCHAIN SCAN RESULTS")
    print("=" * 70)

    # 1. Node Status
    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│ 📡 NODE STATUS                                                      │")
    print("├─────────────────────────────────────────────────────────────────────┤")

    blocks = {}
    for name, client in clients.items():
        block = client.get_block_number()
        blocks[name] = block
        status = "🟢 ONLINE" if block > 0 else "🔴 OFFLINE"
        print(f"│ {name:15} │ Block #{block:6} │ RPC: {client.url:25} │ {status} │")

    print("└─────────────────────────────────────────────────────────────────────┘")

    # 2. Block Details
    validator = clients["validator-a"]
    current_block = validator.get_block_number()

    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│ 🧱 RECENT BLOCKS                                                    │")
    print("├─────────────────────────────────────────────────────────────────────┤")

    all_txs = []
    for i in range(max(0, current_block - 5), current_block + 1):
        block = validator.get_block(i)
        if block:
            tx_count = len(block.get("transactions", []))
            timestamp = int(block.get("timestamp", "0x0"), 16) if block.get("timestamp") else 0
            block_hash = block.get("hash", "")[:18] + "..." if block.get("hash") else "N/A"
            print(f"│ Block #{i:6} │ Hash: {block_hash} │ TXs: {tx_count:3} │")

            for tx in block.get("transactions", []):
                if isinstance(tx, dict):
                    all_txs.append(tx)

    print("└─────────────────────────────────────────────────────────────────────┘")

    # 3. Transactions
    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│ 💸 TRANSACTIONS                                                     │")
    print("├─────────────────────────────────────────────────────────────────────┤")

    if all_txs:
        for tx in all_txs[:10]:  # Show first 10
            tx_hash = tx.get("hash", "")[:18] + "..." if tx.get("hash") else "N/A"
            from_addr = tx.get("from", "")[:12] + "..." if tx.get("from") else "N/A"
            to_addr = tx.get("to", "")[:12] + "..." if tx.get("to") else "CONTRACT CREATE"
            value = int(tx.get("value", "0x0"), 16) if tx.get("value") else 0

            print(f"│ TX: {tx_hash}")
            print(f"│   From: {from_addr} → To: {to_addr}")
            print(f"│   Value: {format_wei(value)}")
            print("│")
    else:
        print("│ No transactions found in recent blocks                            │")

    print("└─────────────────────────────────────────────────────────────────────┘")

    # 4. Known Addresses & Contracts
    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│ 📋 KNOWN ADDRESSES                                                  │")
    print("├─────────────────────────────────────────────────────────────────────┤")

    addresses = [
        ("Genesis Account", "0x0000000000000000000000000000000000000001"),
        ("DAO Treasury", "0xDAO0000000000000000000000000000000000001"),
        ("Test Account 1", "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
        ("Test Account 2", "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    ]

    for name, addr in addresses:
        balance = validator.get_balance(addr)
        code = validator.get_code(addr)
        is_contract = len(code) > 2  # More than just "0x"
        addr_type = "📜 CONTRACT" if is_contract else "👤 EOA"

        print(f"│ {name:20} │ {addr[:20]}... │ {format_wei(balance):15} │ {addr_type} │")

    print("└─────────────────────────────────────────────────────────────────────┘")

    # 5. Multi-node Sync Check
    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│ 🔄 MULTI-NODE SYNC STATUS                                           │")
    print("├─────────────────────────────────────────────────────────────────────┤")

    max_block = max(blocks.values())
    min_block = min(blocks.values())
    sync_diff = max_block - min_block
    sync_status = "✅ SYNCED" if sync_diff <= 2 else "⚠️ SYNCING"

    print(f"│ Highest Block: #{max_block} │ Lowest Block: #{min_block} │ Diff: {sync_diff} │ {sync_status} │")

    # Check if all nodes can see transactions
    print("│")
    print("│ Transaction Visibility Across Nodes:")
    if all_txs:
        sample_tx = all_txs[0].get("hash") if isinstance(all_txs[0], dict) else None
        if sample_tx:
            for name, client in clients.items():
                tx = client.get_transaction(sample_tx)
                visible = "✅ Visible" if tx else "❌ Not found"
                print(f"│   {name}: TX {sample_tx[:18]}... → {visible}")
    else:
        print("│   No transactions to verify")

    print("└─────────────────────────────────────────────────────────────────────┘")

    # 6. Contract Deployment Info (if any)
    print("\n┌─────────────────────────────────────────────────────────────────────┐")
    print("│ 📜 DEPLOYED CONTRACTS                                               │")
    print("├─────────────────────────────────────────────────────────────────────┤")

    contracts_found = False
    for tx in all_txs:
        if isinstance(tx, dict) and tx.get("to") is None:
            # This is a contract creation
            tx_hash = tx.get("hash", "")
            receipt = validator.get_receipt(tx_hash)
            if receipt and receipt.get("contractAddress"):
                contract_addr = receipt.get("contractAddress")
                print(f"│ Contract: {contract_addr}")
                print(f"│   Created in TX: {tx_hash[:42]}...")
                print(f"│   Status: {'✅ Success' if receipt.get('status') == '0x1' else '❌ Failed'}")
                contracts_found = True

    if not contracts_found:
        print("│ No contract deployments found in recent blocks                    │")
        print("│                                                                   │")
        print("│ 📝 MDTVesting Contract Available:                                 │")
        print("│   - ABI: contracts/artifacts/src/MDTVesting.sol/MDTVesting.json   │")
        print("│   - Bytecode: 5KB+ (ready for deployment)                         │")
        print("│   - Features: createTeamVesting, claim, revoke, etc.              │")

    print("└─────────────────────────────────────────────────────────────────────┘")


# =============================================================================
# MAIN
# =============================================================================

def main():
    print("""
╔══════════════════════════════════════════════════════════════════════╗
║       ModernTensor Transaction Scanner & Contract Viewer             ║
║                                                                      ║
║  Shows: Block details, Transactions, Addresses, Contracts           ║
╚══════════════════════════════════════════════════════════════════════╝
    """)

    manager = TestnetManager()

    try:
        if not manager.start():
            return 1

        print("\n✅ Testnet running! Waiting for blocks...")
        time.sleep(10)  # Wait for some blocks

        scan_and_display(manager.clients)

        return 0

    except KeyboardInterrupt:
        print("\n⚠️ Interrupted")
        return 130
    finally:
        manager.cleanup()


if __name__ == "__main__":
    sys.exit(main())
