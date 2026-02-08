#!/usr/bin/env python3
"""
Complete Blockchain Scanner + Vesting Lock/Unlock Test

Features:
1. Detailed transaction query (like Etherscan)
2. Deploy MDTVesting contract
3. Test Lock: createTeamVesting (lock tokens)
4. Test Unlock: claim (unlock vested tokens)
5. Multi-node transaction scanning

All transaction fields are displayed in detail.
"""

import os
import sys
import time
import json
import signal
import shutil
import hashlib
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

# Test accounts
DEPLOYER = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
BENEFICIARY = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
TOKEN_ADDR = "0x0000000000000000000000000000000000000002"

# Function selectors (keccak256 first 4 bytes)
# setTGE(uint256): 0xff1a318d
# createTeamVesting(address,uint256): 0x88d9e6f4
# claim(): 0x4e71d92d
# claimable(address): 0x402914f5
# getVestingInfo(address): 0xfb897ce4
# totalAllocated(): 0x45f7f249

FUNC_SET_TGE = "0xff1a318d"
FUNC_CREATE_TEAM_VESTING = "0x88d9e6f4"
FUNC_CLAIM = "0x4e71d92d"
FUNC_CLAIMABLE = "0x402914f5"
FUNC_GET_VESTING_INFO = "0xfb897ce4"
FUNC_TOTAL_ALLOCATED = "0x45f7f249"

# MDTVesting bytecode (truncated for deployment)
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

    def get_block(self, num: int) -> Dict:
        return self.call("eth_getBlockByNumber", [hex(num), True])

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
        return self.call("eth_sendTransaction", [tx])

    def eth_call(self, tx: Dict) -> str:
        return self.call("eth_call", [tx, "latest"])

    def wait_for_receipt(self, tx_hash: str, timeout: int = 30) -> Dict:
        start = time.time()
        while time.time() - start < timeout:
            try:
                receipt = self.get_receipt(tx_hash)
                if receipt:
                    # Return receipt even if pending (blockNumber=0x0)
                    # Contract address may be computed even before mining
                    return receipt
            except:
                pass
            time.sleep(1)
        return None


# =============================================================================
# TRANSACTION DETAILS FORMATTER (Like Etherscan)
# =============================================================================

def format_wei(wei: int) -> str:
    if wei >= 10**18:
        return f"{wei / 10**18:.6f} ETH"
    elif wei >= 10**9:
        return f"{wei / 10**9:.4f} Gwei"
    return f"{wei} Wei"


def print_transaction_details(client: RpcClient, tx_hash: str, title: str = ""):
    """Print detailed transaction info like Etherscan"""
    print("\n" + "=" * 70)
    if title:
        print(f"  TRANSACTION: {title}")
    print("=" * 70)

    tx = client.get_transaction(tx_hash)
    receipt = client.get_receipt(tx_hash)

    if not tx:
        print(f"  Transaction not found: {tx_hash}")
        return

    # ---- Basic Info ----
    print("\n[BASIC INFO]")
    print(f"  Transaction Hash   : {tx.get('hash', 'N/A')}")

    status = "N/A"
    if receipt:
        status_code = receipt.get('status', '0x0')
        status = "SUCCESS" if status_code == "0x1" else "FAILED"
    print(f"  Status             : {status}")

    block_num = tx.get('blockNumber')
    if block_num:
        print(f"  Block              : {int(block_num, 16)}")
    else:
        print(f"  Block              : Pending")

    block_hash = tx.get('blockHash', 'N/A')
    if block_hash and block_hash != "0x" + "0" * 64:
        print(f"  Block Hash         : {block_hash[:22]}...{block_hash[-8:]}")

    # ---- Addresses ----
    print("\n[ADDRESSES]")
    print(f"  From               : {tx.get('from', 'N/A')}")
    to_addr = tx.get('to')
    if to_addr:
        print(f"  To                 : {to_addr}")
        print(f"  Type               : Transfer / Contract Call")
    else:
        print(f"  To                 : (Contract Creation)")
        print(f"  Type               : Contract Deployment")
        if receipt and receipt.get('contractAddress'):
            print(f"  Contract Address   : {receipt.get('contractAddress')}")

    # ---- Value ----
    print("\n[VALUE]")
    value = int(tx.get('value', '0x0'), 16) if tx.get('value') else 0
    print(f"  Value              : {format_wei(value)}")
    print(f"  Value (Wei)        : {value}")

    # ---- Gas ----
    print("\n[GAS]")
    gas_limit = int(tx.get('gas', '0x0'), 16) if tx.get('gas') else 0
    gas_price = int(tx.get('gasPrice', '0x0'), 16) if tx.get('gasPrice') else 0
    print(f"  Gas Limit          : {gas_limit:,}")
    print(f"  Gas Price          : {format_wei(gas_price)}")

    if receipt:
        gas_used = int(receipt.get('gasUsed', '0x0'), 16)
        print(f"  Gas Used           : {gas_used:,} ({100*gas_used/gas_limit:.1f}%)" if gas_limit else f"  Gas Used           : {gas_used:,}")
        tx_fee = gas_used * gas_price
        print(f"  Transaction Fee    : {format_wei(tx_fee)}")

    # ---- Nonce & Index ----
    print("\n[NONCE & INDEX]")
    nonce = int(tx.get('nonce', '0x0'), 16) if tx.get('nonce') else 0
    print(f"  Nonce              : {nonce}")
    if receipt:
        tx_index = int(receipt.get('transactionIndex', '0x0'), 16)
        print(f"  Transaction Index  : {tx_index}")

    # ---- Input Data ----
    print("\n[INPUT DATA]")
    data = tx.get('input', tx.get('data', '0x'))
    if data and data != "0x":
        data_size = (len(data) - 2) // 2
        print(f"  Data Size          : {data_size} bytes")

        if len(data) >= 10:
            func_selector = data[:10]
            print(f"  Function Selector  : {func_selector}")

            # Decode known functions
            known_funcs = {
                "0xff1a318d": "setTGE(uint256)",
                "0x88d9e6f4": "createTeamVesting(address,uint256)",
                "0x4e71d92d": "claim()",
                "0x402914f5": "claimable(address)",
                "0xfb897ce4": "getVestingInfo(address)",
            }
            if func_selector in known_funcs:
                print(f"  Function Name      : {known_funcs[func_selector]}")

        # Show first 200 chars of data
        if len(data) > 200:
            print(f"  Raw Data           : {data[:100]}...{data[-50:]}")
        else:
            print(f"  Raw Data           : {data}")
    else:
        print(f"  Data               : (empty)")

    # ---- Logs ----
    if receipt and receipt.get('logs'):
        print("\n[LOGS / EVENTS]")
        for i, log in enumerate(receipt['logs']):
            print(f"  Log #{i}:")
            print(f"    Address          : {log.get('address', 'N/A')}")
            topics = log.get('topics', [])
            for j, topic in enumerate(topics):
                print(f"    Topic[{j}]         : {topic}")
            log_data = log.get('data', '0x')
            if log_data and log_data != '0x':
                print(f"    Data             : {log_data[:50]}..." if len(log_data) > 50 else f"    Data             : {log_data}")


def print_transaction_summary_table(transactions: List[tuple], client: RpcClient):
    """Print transactions in table format like explorer"""
    print("\n" + "=" * 100)
    print("  TRANSACTION LIST")
    print("=" * 100)
    print(f"{'#':<3} {'TX Hash':<20} {'Type':<15} {'From':<15} {'To':<15} {'Value':<15} {'Status':<8}")
    print("-" * 100)

    for i, (name, tx_hash) in enumerate(transactions, 1):
        try:
            tx = client.get_transaction(tx_hash)
            receipt = client.get_receipt(tx_hash)

            if tx:
                from_addr = tx.get('from', '')[:12] + "..." if tx.get('from') else "N/A"
                to_addr = tx.get('to')
                if to_addr:
                    to_str = to_addr[:12] + "..."
                else:
                    to_str = "Contract"

                value = int(tx.get('value', '0x0'), 16) if tx.get('value') else 0
                value_str = f"{value / 10**18:.4f} ETH" if value > 0 else "0"

                status = "OK" if receipt and receipt.get('status') == '0x1' else "Pending"

                print(f"{i:<3} {tx_hash[:18]}.. {name:<15} {from_addr:<15} {to_str:<15} {value_str:<15} {status:<8}")
        except:
            print(f"{i:<3} {tx_hash[:18]}.. {name:<15} Error querying")

    print("-" * 100)


# =============================================================================
# HELPER: Encode function calls
# =============================================================================

def encode_address(addr: str) -> str:
    """Pad address to 32 bytes"""
    return addr.lower().replace("0x", "").zfill(64)


def encode_uint256(value: int) -> str:
    """Encode uint256"""
    return hex(value)[2:].zfill(64)


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
        print("  STARTING LOCAL TESTNET")
        print("=" * 70)

        try:
            binary = self._find_binary()
            print(f"[OK] Binary: {binary.name}")
        except:
            print("[ERROR] Binary not found!")
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

        print("\n[WAIT] Waiting 15s...")
        time.sleep(15)

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
# MAIN
# =============================================================================

def main():
    print("""
========================================================================
       BLOCKCHAIN SCANNER + VESTING LOCK/UNLOCK TEST

       1. Deploy MDTVesting Contract
       2. LOCK: createTeamVesting (lock tokens for beneficiary)
       3. UNLOCK: claim (beneficiary claims vested tokens)
       4. Detailed Transaction Query (like Etherscan)
========================================================================
    """)

    manager = TestnetManager()
    all_transactions = []
    contract_address = None

    try:
        if not manager.start():
            return 1

        validator = manager.clients["validator-a"]

        # ================================================================
        # PHASE 1: DEPLOY CONTRACT
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 1: DEPLOY VESTING CONTRACT")
        print("=" * 70)

        constructor_arg = encode_address(TOKEN_ADDR)
        deploy_data = VESTING_BYTECODE + constructor_arg

        print(f"\n[TX] Deploying MDTVesting...")
        tx = {"from": DEPLOYER, "data": deploy_data, "gas": hex(3000000)}
        tx_hash = validator.send_transaction(tx)
        print(f"  TX Hash: {tx_hash}")
        all_transactions.append(("Deploy Contract", tx_hash))

        time.sleep(6)
        receipt = validator.wait_for_receipt(tx_hash, timeout=20)

        if receipt and receipt.get('contractAddress'):
            contract_address = receipt.get('contractAddress')
            print(f"  Contract: {contract_address}")
            print_transaction_details(validator, tx_hash, "DEPLOY MDTVesting")
        else:
            print("  [ERROR] Contract not deployed!")
            contract_address = None

        if not contract_address:
            print("\n[SKIP] Cannot continue without contract")
            return 1

        # ================================================================
        # PHASE 2: SET TGE (Token Generation Event)
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 2: SET TGE (Token Generation Event)")
        print("=" * 70)

        # setTGE(uint256 timestamp) - set TGE to now
        tge_timestamp = int(time.time())
        call_data = FUNC_SET_TGE + encode_uint256(tge_timestamp)

        print(f"\n[TX] Setting TGE to {tge_timestamp}...")
        tx = {"from": DEPLOYER, "to": contract_address, "data": call_data, "gas": hex(100000)}
        tx_hash = validator.send_transaction(tx)
        print(f"  TX Hash: {tx_hash}")
        all_transactions.append(("Set TGE", tx_hash))

        time.sleep(5)
        print_transaction_details(validator, tx_hash, "SET TGE")

        # ================================================================
        # PHASE 3: LOCK TOKENS (createTeamVesting)
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 3: LOCK TOKENS (createTeamVesting)")
        print("=" * 70)

        # createTeamVesting(address beneficiary, uint256 amount)
        lock_amount = 1000 * 10**18  # 1000 tokens
        call_data = FUNC_CREATE_TEAM_VESTING + encode_address(BENEFICIARY) + encode_uint256(lock_amount)

        print(f"\n[TX] Locking 1000 tokens for {BENEFICIARY[:20]}...")
        tx = {"from": DEPLOYER, "to": contract_address, "data": call_data, "gas": hex(500000)}
        tx_hash = validator.send_transaction(tx)
        print(f"  TX Hash: {tx_hash}")
        all_transactions.append(("Lock Tokens", tx_hash))

        time.sleep(5)
        print_transaction_details(validator, tx_hash, "LOCK TOKENS (createTeamVesting)")

        # ================================================================
        # PHASE 4: CHECK VESTING INFO
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 4: CHECK VESTING INFO")
        print("=" * 70)

        # getVestingInfo(address beneficiary)
        call_data = FUNC_GET_VESTING_INFO + encode_address(BENEFICIARY)

        print(f"\n[CALL] getVestingInfo({BENEFICIARY[:20]}...)...")
        try:
            result = validator.eth_call({"to": contract_address, "data": call_data})
            print(f"  Result: {result}")
            if result and result != "0x":
                # Decode: (scheduleCount, totalVested, totalClaimable, totalClaimed)
                if len(result) >= 258:  # 0x + 4*64
                    schedule_count = int(result[2:66], 16)
                    total_vested = int(result[66:130], 16)
                    total_claimable = int(result[130:194], 16)
                    total_claimed = int(result[194:258], 16)
                    print(f"  Schedule Count: {schedule_count}")
                    print(f"  Total Vested: {total_vested / 10**18:.4f} tokens")
                    print(f"  Total Claimable: {total_claimable / 10**18:.4f} tokens")
                    print(f"  Total Claimed: {total_claimed / 10**18:.4f} tokens")
        except Exception as e:
            print(f"  [INFO] eth_call: {e}")

        # ================================================================
        # PHASE 5: UNLOCK TOKENS (claim)
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 5: UNLOCK TOKENS (claim)")
        print("=" * 70)

        # claim() - beneficiary claims vested tokens
        call_data = FUNC_CLAIM

        print(f"\n[TX] Beneficiary claiming tokens...")
        tx = {"from": BENEFICIARY, "to": contract_address, "data": call_data, "gas": hex(300000)}
        tx_hash = validator.send_transaction(tx)
        print(f"  TX Hash: {tx_hash}")
        all_transactions.append(("Unlock/Claim", tx_hash))

        time.sleep(5)
        print_transaction_details(validator, tx_hash, "UNLOCK TOKENS (claim)")

        # ================================================================
        # PHASE 6: TRANSACTION SUMMARY TABLE
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 6: TRANSACTION SUMMARY")
        print("=" * 70)

        print_transaction_summary_table(all_transactions, validator)

        # ================================================================
        # PHASE 7: MULTI-NODE VERIFICATION
        # ================================================================
        print("\n" + "=" * 70)
        print("  PHASE 7: MULTI-NODE VERIFICATION")
        print("=" * 70)

        time.sleep(3)

        print("\n[Contract Code Verification]")
        for name, client in manager.clients.items():
            code = client.get_code(contract_address)
            code_size = (len(code) - 2) // 2 if code.startswith("0x") else 0
            print(f"  {name}: {code_size} bytes")

        print("\n[Transaction Visibility]")
        for tx_name, tx_hash in all_transactions:
            visible_count = 0
            for name, client in manager.clients.items():
                tx = client.get_transaction(tx_hash)
                if tx:
                    visible_count += 1
            print(f"  {tx_name}: Visible on {visible_count}/3 nodes")

        print("\n[Final Block Numbers]")
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
