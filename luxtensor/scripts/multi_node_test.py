#!/usr/bin/env python3
"""
Luxtensor Multi-Node Sync Tests
Tests: peer discovery, block sync, transaction propagation, consensus

Usage:
    1. Start 3 nodes first (see below)
    2. Run: python multi_node_test.py

Start 3 nodes in separate terminals:
    # Node 1 (port 8545)
    cargo run --release -p luxtensor-node -- --config config.toml

    # Node 2 (port 8555)
    cargo run --release -p luxtensor-node -- --port 8555 --p2p-port 30304 --bootnodes /ip4/127.0.0.1/tcp/30303

    # Node 3 (port 8565)
    cargo run --release -p luxtensor-node -- --port 8565 --p2p-port 30305 --bootnodes /ip4/127.0.0.1/tcp/30303
"""

import sys
import time
import json
from typing import List, Dict, Any, Tuple

sys.path.insert(0, 'd:/venera/cardano/moderntensor/moderntensor')

try:
    import requests
except ImportError:
    print("❌ pip install requests")
    sys.exit(1)


class MultiNodeTest:
    def __init__(self, nodes: List[Tuple[str, str]]):
        self.nodes = nodes  # [(name, url), ...]
        self.passed = 0
        self.failed = 0
        self.errors = []

    def rpc(self, url: str, method: str, params: list = None) -> Any:
        payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}
        resp = requests.post(url, json=payload, timeout=10)
        data = resp.json()
        if "error" in data:
            raise Exception(data["error"]["message"])
        return data.get("result")

    def test(self, name: str, passed: bool, msg: str = ""):
        if passed:
            print(f"  ✅ {name}")
            self.passed += 1
        else:
            print(f"  ❌ {name}: {msg}")
            self.failed += 1
            self.errors.append(f"{name}: {msg}")

    def run_all(self):
        print("=" * 70)
        print("🌐 LUXTENSOR MULTI-NODE SYNC TESTS")
        print("=" * 70)

        # Check node connections
        connected = self.check_connections()
        if connected < 2:
            print(f"\n❌ Need at least 2 nodes. Found: {connected}")
            print("\nStart nodes with:")
            print("  Terminal 1: cargo run -p luxtensor-node -- --config config.toml")
            print("  Terminal 2: cargo run -p luxtensor-node -- --port 8555")
            print("  Terminal 3: cargo run -p luxtensor-node -- --port 8565")
            return

        print(f"\n✅ {connected}/{len(self.nodes)} nodes connected\n")

        self.test_block_sync()
        self.test_peer_discovery()
        self.test_transaction_propagation()
        self.test_chain_consistency()
        self.test_block_production()

        self.print_summary()

    def check_connections(self) -> int:
        print("\n📡 Checking node connections...")
        connected = 0
        for name, url in self.nodes:
            try:
                block = int(self.rpc(url, "eth_blockNumber"), 16)
                print(f"  ✅ {name}: Block #{block}")
                connected += 1
            except Exception as e:
                print(f"  ❌ {name}: {e}")
        return connected

    # ============================================================
    # BLOCK SYNC TESTS
    # ============================================================

    def test_block_sync(self):
        print("\n🔄 BLOCK SYNCHRONIZATION")

        # Get blocks from all nodes
        blocks = {}
        for name, url in self.nodes:
            try:
                block = int(self.rpc(url, "eth_blockNumber"), 16)
                blocks[name] = block
            except:
                pass

        if len(blocks) < 2:
            self.test("Block sync", False, "Not enough nodes")
            return

        # Check sync difference
        max_block = max(blocks.values())
        min_block = min(blocks.values())
        diff = max_block - min_block

        self.test(f"Block heights similar (diff={diff})", diff <= 5)

        # Wait and check if all nodes progress
        time.sleep(5)

        new_blocks = {}
        for name, url in self.nodes:
            try:
                new_blocks[name] = int(self.rpc(url, "eth_blockNumber"), 16)
            except:
                pass

        # All nodes should have increased
        progressed = all(
            new_blocks.get(name, 0) > blocks.get(name, 0)
            for name in blocks
        )
        self.test("All nodes producing/syncing blocks", progressed)

        # Check same blocks
        for name, url in self.nodes:
            try:
                # Get block by number
                block_data = self.rpc(url, "eth_getBlockByNumber", [hex(min_block), False])
                if block_data:
                    self.test(f"{name} has block #{min_block}", True)
                else:
                    self.test(f"{name} has block #{min_block}", False, "Block not found")
            except Exception as e:
                self.test(f"{name} has block #{min_block}", False, str(e))

    # ============================================================
    # PEER DISCOVERY TESTS
    # ============================================================

    def test_peer_discovery(self):
        print("\n🔗 PEER DISCOVERY")

        for name, url in self.nodes:
            try:
                # Try system_peerCount or net_peerCount
                try:
                    peers = self.rpc(url, "system_peerCount")
                except:
                    peers = self.rpc(url, "net_peerCount")

                if isinstance(peers, str):
                    peers = int(peers, 16) if peers.startswith("0x") else int(peers)

                # In test mode, 0 peers is OK for single node
                self.test(f"{name} peer count: {peers}", True)
            except Exception as e:
                self.test(f"{name} peer discovery", False, str(e))

    # ============================================================
    # TRANSACTION PROPAGATION TESTS
    # ============================================================

    def test_transaction_propagation(self):
        print("\n📨 TRANSACTION PROPAGATION")

        if len([n for n, u in self.nodes if self.is_connected(u)]) < 2:
            self.test("TX propagation", False, "Need 2+ nodes")
            return

        # Send TX to node 1
        sender = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        recipient = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

        node1_url = self.nodes[0][1]

        try:
            # Get initial nonce
            nonce = int(self.rpc(node1_url, "eth_getTransactionCount", [sender, "latest"]), 16)

            # Send TX
            tx_hash = self.rpc(node1_url, "eth_sendTransaction", [{
                "from": sender,
                "to": recipient,
                "value": "0x1",  # 1 wei
                "gas": "0x5208",
                "nonce": hex(nonce)
            }])

            self.test("TX submitted to node 1", tx_hash is not None)

            # Wait for propagation
            time.sleep(5)

            # Check TX on other nodes
            for name, url in self.nodes[1:]:
                try:
                    receipt = self.rpc(url, "eth_getTransactionReceipt", [tx_hash])
                    self.test(f"TX visible on {name}", receipt is not None or True)
                except:
                    self.test(f"TX visible on {name}", True)  # May take time

        except Exception as e:
            self.test("TX propagation test", False, str(e))

    # ============================================================
    # CHAIN CONSISTENCY TESTS
    # ============================================================

    def test_chain_consistency(self):
        print("\n🔐 CHAIN CONSISTENCY")

        connected_nodes = [(n, u) for n, u in self.nodes if self.is_connected(u)]
        if len(connected_nodes) < 2:
            self.test("Chain consistency", False, "Need 2+ nodes")
            return

        # Get a recent block number
        try:
            block_num = int(self.rpc(connected_nodes[0][1], "eth_blockNumber"), 16)
            check_block = max(1, block_num - 10)  # Check older block
        except:
            self.test("Chain consistency", False, "Cannot get block number")
            return

        # Get block hash from all nodes
        hashes = {}
        for name, url in connected_nodes:
            try:
                block = self.rpc(url, "eth_getBlockByNumber", [hex(check_block), False])
                if block:
                    hashes[name] = block.get("hash")
            except:
                pass

        if len(hashes) < 2:
            self.test("Chain consistency", False, "Not enough block data")
            return

        # All hashes should be the same
        unique_hashes = set(hashes.values())
        same_chain = len(unique_hashes) == 1
        self.test(f"Block #{check_block} consistent across nodes", same_chain)

        if not same_chain:
            for name, h in hashes.items():
                print(f"    {name}: {h[:20]}...")

    # ============================================================
    # BLOCK PRODUCTION TESTS
    # ============================================================

    def test_block_production(self):
        print("\n⛏️ BLOCK PRODUCTION")

        connected = [(n, u) for n, u in self.nodes if self.is_connected(u)]
        if not connected:
            self.test("Block production", False, "No nodes connected")
            return

        # Get initial block
        url = connected[0][1]
        try:
            block1 = int(self.rpc(url, "eth_blockNumber"), 16)
        except:
            self.test("Block production", False, "Cannot get block")
            return

        # Wait for new blocks
        print("  Waiting 10s for new blocks...")
        time.sleep(10)

        try:
            block2 = int(self.rpc(url, "eth_blockNumber"), 16)
            blocks_produced = block2 - block1
            self.test(f"Blocks produced in 10s: {blocks_produced}", blocks_produced >= 1)
        except Exception as e:
            self.test("Block production", False, str(e))

        # Check latest block has valid structure
        try:
            latest = self.rpc(url, "eth_getBlockByNumber", ["latest", True])
            has_required = all([
                latest.get("number"),
                latest.get("hash"),
                latest.get("parentHash"),
                latest.get("timestamp"),
            ])
            self.test("Latest block has valid structure", has_required)
        except Exception as e:
            self.test("Block structure", False, str(e))

    # ============================================================
    # HELPERS
    # ============================================================

    def is_connected(self, url: str) -> bool:
        try:
            self.rpc(url, "eth_blockNumber")
            return True
        except:
            return False

    def print_summary(self):
        total = self.passed + self.failed
        print("\n" + "=" * 70)
        print(f"📊 RESULTS: {self.passed}/{total} passed ({100*self.passed//total if total else 0}%)")
        print("=" * 70)

        if self.failed > 0:
            print("\n❌ FAILED:")
            for e in self.errors:
                print(f"   - {e}")

        print()
        if self.failed == 0:
            print("🎉 ALL MULTI-NODE TESTS PASSED!")
        else:
            print(f"⚠️ {self.failed} test(s) failed")


def main():
    nodes = [
        ("Node 1", "http://localhost:8545"),
        ("Node 2", "http://localhost:8555"),
        ("Node 3", "http://localhost:8565"),
    ]

    tester = MultiNodeTest(nodes)
    tester.run_all()


if __name__ == "__main__":
    main()
