#!/usr/bin/env python3
"""
REAL Contract Deployment & Interaction

This script:
1. Starts 3-node testnet
2. Sends REAL transactions via eth_sendTransaction
3. Deploys MDTVesting contract
4. Scans transactions across all nodes

eth_sendTransaction format (no signing required):
{
    "from": "0x...",
    "to": "0x..." | null (for contract deploy),
    "value": "0x...",
    "data": "0x...",
    "gas": "0x..."
}
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
from typing import List, Dict, Any, Optional
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

CHAIN_ID = 8898

# Test accounts (any address works, no signature needed)
DEPLOYER_ADDR = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
BENEFICIARY_ADDR = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"

# MDTVesting bytecode (compiled EVM bytecode)
VESTING_BYTECODE = "0x60a060405234801561001057600080fd5b5060405161135a38038061135a83398101604081905261002f9161011b565b338061005657604051631e4fbdf760e01b8152600060048201526024015b60405180910390fd5b61005f816100cb565b50600180556001600160a01b0381166100ba5760405162461bcd60e51b815260206004820152601560248201527f496e76616c696420746f6b656e20616464726573730000000000000000000000604482015260640161004d565b6001600160a01b031660805261014b565b600080546001600160a01b038381166001600160a01b0319831681178455604051919092169283917f8be0079c531659141344cd1fd0a4f28419497f9722a3daafe3b4186f6b6457e09190a35050565b60006020828403121561012d57600080fd5b81516001600160a01b038116811461014457600080fd5b9392505050565b6080516111df61017b6000396000818161029a0152818161067b0152818161075c015261085601526111df6000f3fe608060405234801561001057600080fd5b50600436106101165760003560e01c8063ce5b45c3116100a2578063eac449d911610071578063eac449d91461023c578063f2fde38b1461024f578063fb897ce414610262578063fc0c546a14610295578063ff1a318d146102bc57600080fd5b8063ce5b45c314610205578063d54ad2a114610218578063db2e21bc14610221578063dbf403af1461022957600080fd5b80634e71d92d116100e95780634e71d92d146101b2578063715018a6146101bc57806388d9e6f4146101c45780638da5cb5b146101d7578063a4317ef4146101fc57600080fd5b806317cf53911461011b578063402914f51461014157806345626bd61461015457806345f7f249146101a9575b600080fd5b61012e610129366004611055565b6102cf565b6040519081526020015b60405180910390f35b61012e61014f36600461107f565b61042c565b610167610162366004611055565b6104e7565b604080519889526020890197909752958701949094526060860192909252608085015260ff1660a0840152151560c0830152151560e082015261010001610138565b61012e60045481565b6101ba610551565b005b6101ba6106e3565b6101ba6101d2366004611055565b6106f5565b6000546001600160a01b03165b6040516001600160a01b039091168152602001610138565b61012e60025481565b6101ba610213366004611055565b61071a565b61012e60055481565b6101ba61073c565b6101ba610237366004611055565b61087d565b6101ba61024a366004611055565b61089b565b6101ba61025d36600461107f565b610a84565b61027561027036600461107f565b610ac2565b604080519485526020850193909352918301526060820152608001610138565b6101e47f000000000000000000000000000000000000000000000000000000000000000081565b6101ba6102ca3660046110a1565b610b9d565b"


# =============================================================================
# RPC CLIENT
# =============================================================================

class RpcClient:
    def __init__(self, url: str, name: str = ""):
        self.url = url
        self.name = name
        self.request_id = 0

    def call(self, method: str, params: List = None) -> Any:
        self.request_id += 1
        payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": self.request_id}
        resp = requests.post(self.url, json=payload, timeout=30)
        result = resp.json()
        if "error" in result:
            raise Exception(f"RPC Error: {result['error']}")
        return result.get("result")

    def get_block_number(self) -> int:
        r = self.call("eth_blockNumber")
        return int(r, 16) if isinstance(r, str) else r

    def get_balance(self, addr: str) -> int:
        r = self.call("eth_getBalance", [addr, "latest"])
        return int(r, 16) if isinstance(r, str) else r

    def get_code(self, addr: str) -> str:
        return self.call("eth_getCode", [addr, "latest"]) or "0x"

    def get_transaction(self, tx_hash: str) -> Dict:
        return self.call("eth_getTransactionByHash", [tx_hash])

    def get_receipt(self, tx_hash: str) -> Dict:
        return self.call("eth_getTransactionReceipt", [tx_hash])

    def send_transaction(self, tx: Dict) -> str:
        """Send transaction using eth_sendTransaction (no signing needed)"""
        return self.call("eth_sendTransaction", [tx])

    def wait_for_receipt(self, tx_hash: str, timeout: int = 30) -> Dict:
        start = time.time()
        while time.time() - start < timeout:
            try:
                receipt = self.get_receipt(tx_hash)
                if receipt and receipt.get("blockNumber") and receipt.get("blockNumber") != "0x0":
                    return receipt
            except:
                pass
            time.sleep(1)
        return None


# =============================================================================
# TESTNET MANAGER
# =============================================================================

@dataclass
class NodeConfig:
    name: str
    p2p_port: int
    rpc_port: int
    data_dir: Path
    is_validator: bool


class TestnetManager:
    def __init__(self):
        self.processes = []
        self.temp_dirs = []
        self.clients: Dict[str, RpcClient] = {}
        signal.signal(signal.SIGINT, lambda s, f: self.cleanup())

    def _find_binary(self) -> Path:
        paths = [
            Path(__file__).parent.parent / "target" / "release" / "luxtensor-node.exe",
            Path(__file__).parent.parent / "target" / "release" / "luxtensor-node",
        ]
        for p in paths:
            if p.exists():
                return p
        raise FileNotFoundError("Binary not found")

    def _create_config(self, node: NodeConfig) -> Path:
        config = f"""
[node]
name = "{node.name}"
chain_id = {CHAIN_ID}
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
        print("  STARTING LOCAL TESTNET (3 nodes)")
        print("=" * 70)

        try:
            binary = self._find_binary()
            print(f"[OK] Binary: {binary.name}")
        except:
            print("[ERROR] Binary not found!")
            return False

        base_temp = Path(tempfile.mkdtemp(prefix="luxtensor_deploy_"))
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

            print(f"[START] {node.name} @ RPC:{node.rpc_port}")

            log = open(node.data_dir / "stdout.log", "w")
            proc = subprocess.Popen(
                [str(binary), "--config", str(config)],
                stdout=log, stderr=subprocess.STDOUT,
                cwd=str(node.data_dir)
            )
            self.processes.append(proc)
            self.clients[node.name] = RpcClient(f"http://127.0.0.1:{node.rpc_port}", node.name)
            time.sleep(1)

        print("\n[WAIT] Waiting 15s for nodes to start...")
        time.sleep(15)

        # Verify
        for name, client in self.clients.items():
            try:
                block = client.get_block_number()
                print(f"[OK] {name}: Block #{block}")
            except Exception as e:
                print(f"[ERROR] {name}: {e}")
                return False

        return True

    def cleanup(self):
        print("\n[CLEANUP] Stopping nodes...")
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
        print("[DONE] Cleanup complete")


# =============================================================================
# MAIN TEST
# =============================================================================

def main():
    print("""
========================================================================
       REAL CONTRACT DEPLOYMENT & INTERACTION TEST

       Using eth_sendTransaction (JSON format, no signature)
========================================================================
    """)

    manager = TestnetManager()
    all_transactions = []

    try:
        if not manager.start():
            return 1

        validator = manager.clients["validator-a"]
        miner_b = manager.clients["miner-b"]
        miner_c = manager.clients["miner-c"]

        print("\n" + "=" * 70)
        print("  PHASE 1: NATIVE TRANSFER")
        print("=" * 70)

        print(f"\nDeployer: {DEPLOYER_ADDR}")
        print(f"Beneficiary: {BENEFICIARY_ADDR}")

        # TX 1: Native transfer
        print("\n[TX 1] Sending native transfer (1 ETH)...")
        try:
            tx = {
                "from": DEPLOYER_ADDR,
                "to": BENEFICIARY_ADDR,
                "value": hex(10**18),  # 1 ETH
                "gas": hex(21000),
            }
            tx_hash = validator.send_transaction(tx)
            print(f"  TX Hash: {tx_hash}")
            all_transactions.append(("Native Transfer", tx_hash))

            # Wait for mining
            time.sleep(5)
            receipt = validator.wait_for_receipt(tx_hash, timeout=15)
            if receipt:
                print(f"  Status: SUCCESS")
                print(f"  Block: {int(receipt.get('blockNumber', '0x0'), 16)}")
                print(f"  Gas Used: {int(receipt.get('gasUsed', '0x0'), 16)}")
            else:
                print("  Status: Pending (not mined yet)")

        except Exception as e:
            print(f"  [ERROR] {e}")

        print("\n" + "=" * 70)
        print("  PHASE 2: DEPLOY VESTING CONTRACT")
        print("=" * 70)

        # TX 2: Deploy contract
        # Constructor arg: token address (padded to 32 bytes)
        dummy_token = "0x0000000000000000000000000000000000000002"
        constructor_arg = dummy_token[2:].zfill(64)
        deploy_data = VESTING_BYTECODE + constructor_arg

        print("\n[TX 2] Deploying MDTVesting contract...")
        print(f"  Bytecode size: {len(deploy_data) // 2} bytes")

        try:
            tx = {
                "from": DEPLOYER_ADDR,
                # No "to" field = contract creation
                "data": deploy_data,
                "gas": hex(3000000),
            }
            tx_hash = validator.send_transaction(tx)
            print(f"  TX Hash: {tx_hash}")
            all_transactions.append(("Contract Deploy", tx_hash))

            # Wait for mining
            time.sleep(6)
            receipt = validator.wait_for_receipt(tx_hash, timeout=20)

            if receipt:
                contract_address = receipt.get("contractAddress")
                print(f"  Status: SUCCESS")
                print(f"  Contract Address: {contract_address}")
                print(f"  Block: {int(receipt.get('blockNumber', '0x0'), 16)}")
                print(f"  Gas Used: {int(receipt.get('gasUsed', '0x0'), 16)}")

                # Verify contract code
                if contract_address:
                    code = validator.get_code(contract_address)
                    code_size = (len(code) - 2) // 2 if code.startswith("0x") else len(code) // 2
                    print(f"  Deployed Code Size: {code_size} bytes")
            else:
                print("  Status: Pending (not mined yet)")
                # Check if pending tx exists
                pending_tx = validator.get_transaction(tx_hash)
                if pending_tx:
                    print("  Transaction exists in mempool")
                    contract_address = None

        except Exception as e:
            print(f"  [ERROR] {e}")
            contract_address = None

        print("\n" + "=" * 70)
        print("  PHASE 3: MULTI-NODE TRANSACTION SCAN")
        print("=" * 70)

        time.sleep(5)  # Wait for sync

        for tx_name, tx_hash in all_transactions:
            print(f"\n--- {tx_name} ---")
            print(f"TX Hash: {tx_hash}")

            for node_name, client in manager.clients.items():
                try:
                    tx = client.get_transaction(tx_hash)
                    if tx:
                        print(f"  [{node_name}] FOUND")
                        print(f"    From: {tx.get('from', 'N/A')}")
                        to_addr = tx.get('to')
                        if to_addr:
                            print(f"    To: {to_addr}")
                        else:
                            print(f"    To: CONTRACT CREATION")
                        value = int(tx.get('value', '0x0'), 16) if tx.get('value') else 0
                        print(f"    Value: {value / 10**18:.6f} ETH")
                        block_num = tx.get('blockNumber')
                        if block_num:
                            print(f"    Block: {int(block_num, 16)}")
                    else:
                        print(f"  [{node_name}] Not found")
                except Exception as e:
                    print(f"  [{node_name}] Error: {e}")

        print("\n" + "=" * 70)
        print("  SUMMARY")
        print("=" * 70)

        print(f"\nTotal Transactions: {len(all_transactions)}")
        for i, (name, hash) in enumerate(all_transactions, 1):
            print(f"  {i}. {name}: {hash}")

        # Final block numbers
        print("\nFinal Block Numbers:")
        for name, client in manager.clients.items():
            block = client.get_block_number()
            print(f"  {name}: Block #{block}")

        return 0

    except KeyboardInterrupt:
        print("\n[INTERRUPTED]")
        return 130
    except Exception as e:
        print(f"\n[ERROR] {e}")
        import traceback
        traceback.print_exc()
        return 1
    finally:
        manager.cleanup()


if __name__ == "__main__":
    sys.exit(main())
