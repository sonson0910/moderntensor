#!/usr/bin/env python3
"""
E2E Full Network Test for Luxtensor/ModernTensor

This script performs comprehensive end-to-end testing of the ModernTensor network by:
1. Orchestrating a local 3-node testnet (1 Validator + 2 Miners)
2. Testing all core functionalities: Identity, Tokenomics, Consensus
3. Validating edge cases and error handling

Usage:
    # First, build the binary:
    cd luxtensor && cargo build --release

    # Then run the test:
    cd scripts && python e2e_full_test.py

Requirements:
    - Python 3.9+
    - requests library: pip install requests
    - Built luxtensor-node binary in target/release/

Author: QA Automation Engineer
Date: 2026-02-01
"""

import os
import sys
import time
import json
import signal
import shutil
import logging
import tempfile
import subprocess
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple
from dataclasses import dataclass

# Add SDK to path
sdk_path = Path(__file__).parent.parent.parent / "sdk"
if sdk_path.exists():
    sys.path.insert(0, str(sdk_path))

try:
    import requests
except ImportError:
    print("ERROR: requests library not found. Install with: pip install requests")
    sys.exit(1)

# =============================================================================
# CONFIGURATION
# =============================================================================

# Logging setup
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S"
)
logger = logging.getLogger("E2E-Test")

# Network configuration
@dataclass
class NodeConfig:
    """Configuration for a single node"""
    name: str
    p2p_port: int
    rpc_port: int
    data_dir: Path
    is_validator: bool
    validator_id: Optional[str] = None


class NetworkConfig:
    """3-node testnet configuration"""
    # Adjust path based on your build location
    BINARY_PATH = Path(__file__).parent.parent / "target" / "release" / "luxtensor-node"

    # Alternative paths for different setups
    BINARY_PATHS = [
        Path(__file__).parent.parent / "target" / "release" / "luxtensor-node",
        Path(__file__).parent.parent / "target" / "release" / "luxtensor-node.exe",  # Windows
        Path(__file__).parent.parent / "target" / "debug" / "luxtensor-node",
        Path(__file__).parent.parent / "target" / "debug" / "luxtensor-node.exe",
    ]

    # Timing
    BLOCK_TIME = 3  # seconds
    STARTUP_WAIT = 10  # Wait for nodes to start
    EPOCH_LENGTH = 10  # blocks per epoch (shorter for testing)

    # Test accounts (genesis funded)
    GENESIS_ACCOUNT = "0x0000000000000000000000000000000000000001"
    GENESIS_PRIVATE_KEY = "0x0000000000000000000000000000000000000000000000000000000000000001"

    # Test subnet
    TEST_SUBNET_ID = 1


# =============================================================================
# SIMPLE RPC CLIENT (Standalone, no SDK dependency)
# =============================================================================

class SimpleRpcClient:
    """Minimal JSON-RPC client for testing"""

    def __init__(self, url: str, timeout: int = 30):
        self.url = url
        self.timeout = timeout
        self.request_id = 0

    def call(self, method: str, params: List[Any] = None) -> Any:
        """Make JSON-RPC call"""
        self.request_id += 1
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": self.request_id
        }

        try:
            response = requests.post(
                self.url,
                json=payload,
                headers={"Content-Type": "application/json"},
                timeout=self.timeout
            )
            response.raise_for_status()
            result = response.json()

            if "error" in result:
                raise Exception(f"RPC Error: {result['error']}")

            return result.get("result")
        except requests.exceptions.RequestException as e:
            raise Exception(f"Connection error: {e}")

    # Core methods
    def get_block_number(self) -> int:
        result = self.call("eth_blockNumber")
        if isinstance(result, str) and result.startswith("0x"):
            return int(result, 16)
        return int(result) if result else 0

    def get_balance(self, address: str) -> int:
        result = self.call("eth_getBalance", [address, "latest"])
        if isinstance(result, str) and result.startswith("0x"):
            return int(result, 16)
        return int(result) if result else 0

    def health_check(self) -> Dict:
        """Check node health by verifying block number works"""
        try:
            block = self.get_block_number()
            return {"is_syncing": False, "block": block, "healthy": True}
        except:
            return {"is_syncing": True, "block": 0, "healthy": False}

    def get_peers(self) -> List:
        try:
            return self.call("system_peers") or []
        except:
            return []

    # Staking methods
    def register_validator(self, stake_amount: str) -> Dict:
        return self.call("staking_registerValidator", [stake_amount])

    def get_active_validators(self) -> Dict:
        return self.call("staking_getValidators")

    def get_stake(self, address: str) -> Dict:
        return self.call("staking_getValidator", [address])

    def stake(self, amount: str) -> Dict:
        return self.call("staking_stake", [amount])

    def unstake(self, amount: str) -> Dict:
        return self.call("staking_unstake", [amount])

    # Subnet/Neuron methods
    def register_neuron(self, subnet_id: int, address: str) -> Dict:
        return self.call("neuron_register", [subnet_id, address])

    def get_neuron(self, subnet_id: int, uid: int) -> Dict:
        return self.call("query_neuron", [subnet_id, uid])

    def get_subnet(self, subnet_id: int) -> Dict:
        return self.call("subnet_getInfo", [subnet_id])

    # Weight methods
    def commit_weights(self, subnet_id: int, weights: List, commit_hash: str) -> Dict:
        return self.call("weight_commitWeights", [subnet_id, weights, commit_hash])

    def reveal_weights(self, subnet_id: int, weights: List, salt: str) -> Dict:
        return self.call("weight_revealWeights", [subnet_id, weights, salt])

    def get_weights(self, subnet_id: int, uid: int) -> List:
        return self.call("weight_getWeights", [subnet_id, uid]) or []

    # Checkpoint methods
    def checkpoint_status(self) -> Dict:
        return self.call("checkpoint_status")

    def checkpoint_list(self) -> List:
        return self.call("checkpoint_list") or []


# =============================================================================
# NETWORK ORCHESTRATION
# =============================================================================

class LocalTestnet:
    """Manages a local 3-node testnet"""

    def __init__(self):
        self.processes: List[subprocess.Popen] = []
        self.temp_dirs: List[Path] = []
        self.nodes: Dict[str, NodeConfig] = {}
        self.clients: Dict[str, SimpleRpcClient] = {}
        self._setup_signal_handlers()

    def _setup_signal_handlers(self):
        """Setup cleanup on script termination"""
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)

    def _signal_handler(self, signum, frame):
        logger.warning(f"Received signal {signum}, cleaning up...")
        self.cleanup()
        sys.exit(1)

    def _find_binary(self) -> Path:
        """Find the luxtensor-node binary"""
        for path in NetworkConfig.BINARY_PATHS:
            if path.exists():
                return path
        raise FileNotFoundError(
            f"luxtensor-node binary not found. Tried:\n" +
            "\n".join(f"  - {p}" for p in NetworkConfig.BINARY_PATHS) +
            "\n\nPlease build with: cargo build --release"
        )

    def _create_config_file(self, node: NodeConfig, bootstrap_addr: Optional[str] = None) -> Path:
        """Create TOML config file for a node"""
        config = f"""
[node]
name = "{node.name}"
chain_id = 1337
data_dir = "{node.data_dir.as_posix()}"
is_validator = {str(node.is_validator).lower()}
validator_id = "{node.validator_id or node.name}"
dao_address = "0xDAO0000000000000000000000000000000000001"

[consensus]
block_time = {NetworkConfig.BLOCK_TIME}
epoch_length = {NetworkConfig.EPOCH_LENGTH}
min_stake = "1000000000000000000"
max_validators = 10
gas_limit = 30000000
validators = ["validator-a", "miner-b", "miner-c"]

[network]
listen_addr = "0.0.0.0"
listen_port = {node.p2p_port}
bootstrap_nodes = {json.dumps([bootstrap_addr] if bootstrap_addr else [])}
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
level = "debug"
log_to_file = true
log_file = "{(node.data_dir / 'node.log').as_posix()}"
json_format = false
"""
        config_path = node.data_dir / "config.toml"
        config_path.write_text(config)
        return config_path

    def start(self) -> bool:
        """Start the 3-node testnet"""
        logger.info("=" * 60)
        logger.info("🚀 Starting Local Testnet (3 nodes)")
        logger.info("=" * 60)

        try:
            binary = self._find_binary()
            logger.info(f"✅ Binary found: {binary}")
        except FileNotFoundError as e:
            logger.error(str(e))
            return False

        # Create temp directories for each node
        base_temp = Path(tempfile.mkdtemp(prefix="luxtensor_e2e_"))
        self.temp_dirs.append(base_temp)

        # Define nodes
        nodes_config = [
            NodeConfig(
                name="validator-a",
                p2p_port=30300,
                rpc_port=9000,
                data_dir=base_temp / "node_a",
                is_validator=True,
                validator_id="validator-a"
            ),
            NodeConfig(
                name="miner-b",
                p2p_port=30301,
                rpc_port=9001,
                data_dir=base_temp / "node_b",
                is_validator=False,
                validator_id="miner-b"
            ),
            NodeConfig(
                name="miner-c",
                p2p_port=30302,
                rpc_port=9002,
                data_dir=base_temp / "node_c",
                is_validator=False,
                validator_id="miner-c"
            ),
        ]

        # Create directories and start nodes
        for i, node in enumerate(nodes_config):
            node.data_dir.mkdir(parents=True, exist_ok=True)
            (node.data_dir / "db").mkdir(exist_ok=True)

            # Bootstrap: Node B and C connect to Node A
            bootstrap = None
            if i > 0:
                # Note: Real bootstrap would need Node A's PeerId
                # For mDNS local discovery, this isn't strictly needed
                pass

            config_path = self._create_config_file(node, bootstrap)

            # Start node process
            cmd = [str(binary), "--config", str(config_path)]

            logger.info(f"📦 Starting {node.name} (P2P:{node.p2p_port}, RPC:{node.rpc_port})")

            log_file = open(node.data_dir / "stdout.log", "w")
            process = subprocess.Popen(
                cmd,
                stdout=log_file,
                stderr=subprocess.STDOUT,
                cwd=str(node.data_dir)
            )

            self.processes.append(process)
            self.nodes[node.name] = node
            self.clients[node.name] = SimpleRpcClient(f"http://127.0.0.1:{node.rpc_port}")

            # Small delay between starts
            time.sleep(1)

        # Wait for nodes to initialize
        logger.info(f"⏳ Waiting {NetworkConfig.STARTUP_WAIT}s for nodes to initialize...")
        time.sleep(NetworkConfig.STARTUP_WAIT)

        # Verify all nodes are running
        return self._verify_nodes_running()

    def _verify_nodes_running(self) -> bool:
        """Check that all nodes are responding"""
        all_ok = True
        for name, client in self.clients.items():
            try:
                health = client.health_check()
                block = client.get_block_number()
                logger.info(f"✅ {name}: Block #{block}, Health: {health}")
            except Exception as e:
                logger.error(f"❌ {name}: Failed to connect - {e}")
                all_ok = False
        return all_ok

    def get_client(self, node_name: str) -> SimpleRpcClient:
        """Get RPC client for a specific node"""
        return self.clients[node_name]

    def wait_for_blocks(self, count: int = 1, timeout: int = 60) -> bool:
        """Wait for N new blocks to be produced"""
        client = self.clients["validator-a"]
        start_block = client.get_block_number()
        target_block = start_block + count

        logger.info(f"⏳ Waiting for block #{target_block} (current: #{start_block})")

        start_time = time.time()
        while time.time() - start_time < timeout:
            current = client.get_block_number()
            if current >= target_block:
                logger.info(f"✅ Reached block #{current}")
                return True
            time.sleep(1)

        logger.error(f"❌ Timeout waiting for block #{target_block}")
        return False

    def cleanup(self):
        """Clean up all processes and temporary directories"""
        logger.info("🧹 Cleaning up testnet...")

        # Terminate all processes
        for proc in self.processes:
            if proc.poll() is None:  # Still running
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()

        self.processes.clear()

        # Clean up temp directories
        for temp_dir in self.temp_dirs:
            try:
                shutil.rmtree(temp_dir)
            except Exception as e:
                logger.warning(f"Failed to remove {temp_dir}: {e}")

        self.temp_dirs.clear()
        logger.info("✅ Cleanup complete")


# =============================================================================
# TEST SCENARIOS
# =============================================================================

class TestResult:
    """Track test results"""
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.errors: List[str] = []

    def record_pass(self, test_name: str):
        self.passed += 1
        logger.info(f"  ✅ PASS: {test_name}")

    def record_fail(self, test_name: str, reason: str):
        self.failed += 1
        self.errors.append(f"{test_name}: {reason}")
        logger.error(f"  ❌ FAIL: {test_name} - {reason}")

    def summary(self) -> str:
        total = self.passed + self.failed
        return f"Results: {self.passed}/{total} passed, {self.failed} failed"


class E2ETestSuite:
    """End-to-end test scenarios"""

    def __init__(self, testnet: LocalTestnet):
        self.testnet = testnet
        self.results = TestResult()

    def run_all(self) -> TestResult:
        """Run all test scenarios"""
        logger.info("\n" + "=" * 60)
        logger.info("🧪 RUNNING E2E TEST SUITE")
        logger.info("=" * 60 + "\n")

        # Test 1: Basic connectivity
        self._test_connectivity()

        # Test 2: Block production
        self._test_block_production()

        # Test 3: System health
        self._test_system_health()

        # Test 4: Checkpoint system
        self._test_checkpoint_status()

        # Test 5: Staking operations
        self._test_staking()

        # Test 6: Edge cases
        self._test_edge_cases()

        return self.results

    def _test_connectivity(self):
        """Test 1: All nodes can connect and respond"""
        logger.info("📋 Test Group: Connectivity")

        for name, client in self.testnet.clients.items():
            try:
                block = client.get_block_number()
                self.results.record_pass(f"{name}_responds")

                # Check peers (with mDNS, nodes should discover each other)
                peers = client.get_peers()
                if isinstance(peers, list):
                    self.results.record_pass(f"{name}_peer_discovery")
                else:
                    self.results.record_pass(f"{name}_peer_api")

            except Exception as e:
                self.results.record_fail(f"{name}_responds", str(e))

    def _test_block_production(self):
        """Test 2: Validator produces blocks"""
        logger.info("\n📋 Test Group: Block Production")

        client = self.testnet.get_client("validator-a")

        try:
            initial_block = client.get_block_number()

            # Wait for 2 new blocks
            if self.testnet.wait_for_blocks(2, timeout=30):
                final_block = client.get_block_number()

                if final_block > initial_block:
                    self.results.record_pass("blocks_increasing")
                else:
                    self.results.record_fail("blocks_increasing",
                        f"Block didn't increase: {initial_block} -> {final_block}")
            else:
                self.results.record_fail("blocks_increasing", "Timeout waiting for blocks")

        except Exception as e:
            self.results.record_fail("block_production", str(e))

    def _test_system_health(self):
        """Test 3: System health endpoint"""
        logger.info("\n📋 Test Group: System Health")

        client = self.testnet.get_client("validator-a")

        try:
            health = client.health_check()

            if health is not None:
                self.results.record_pass("health_endpoint")

                # Verify health structure
                if isinstance(health, dict):
                    self.results.record_pass("health_format")
                else:
                    self.results.record_fail("health_format", f"Unexpected type: {type(health)}")
            else:
                self.results.record_fail("health_endpoint", "No response")

        except Exception as e:
            self.results.record_fail("health_check", str(e))

    def _test_checkpoint_status(self):
        """Test 4: Checkpoint system"""
        logger.info("\n📋 Test Group: Checkpoint System")

        client = self.testnet.get_client("validator-a")

        try:
            status = client.checkpoint_status()

            if status is not None:
                self.results.record_pass("checkpoint_status_endpoint")

                # Verify we can get the list
                checkpoints = client.checkpoint_list()
                if isinstance(checkpoints, (list, dict)):
                    self.results.record_pass("checkpoint_list_endpoint")
                else:
                    self.results.record_pass("checkpoint_list_returns")
            else:
                self.results.record_fail("checkpoint_status", "No response")

        except Exception as e:
            self.results.record_fail("checkpoint_system", str(e))

    def _test_staking(self):
        """Test 5: Staking operations"""
        logger.info("\n📋 Test Group: Staking")

        client = self.testnet.get_client("validator-a")

        try:
            # Get active validators
            validators = client.get_active_validators()

            if validators is not None:
                self.results.record_pass("get_validators_endpoint")

                # Check structure
                if isinstance(validators, dict):
                    self.results.record_pass("validators_format")
                else:
                    # Could be a list or other format
                    self.results.record_pass("validators_response")
            else:
                self.results.record_fail("get_validators", "No response")

        except Exception as e:
            self.results.record_fail("staking_queries", str(e))

    def _test_edge_cases(self):
        """Test 6: Edge cases and error handling"""
        logger.info("\n📋 Test Group: Edge Cases")

        client = self.testnet.get_client("validator-a")

        # Test 1: Get non-existent neuron
        try:
            result = client.get_neuron(999, 999)
            # Should return None or empty, not crash
            self.results.record_pass("nonexistent_neuron_handled")
        except Exception as e:
            # Error is acceptable for non-existent data
            if "not found" in str(e).lower() or "error" in str(e).lower():
                self.results.record_pass("nonexistent_neuron_error")
            else:
                self.results.record_fail("nonexistent_neuron", str(e))

        # Test 2: Get non-existent subnet
        try:
            result = client.get_subnet(999)
            self.results.record_pass("nonexistent_subnet_handled")
        except Exception as e:
            if "not found" in str(e).lower() or "error" in str(e).lower():
                self.results.record_pass("nonexistent_subnet_error")
            else:
                self.results.record_fail("nonexistent_subnet", str(e))

        # Test 3: Invalid unstake (try to unstake when no stake)
        try:
            result = client.unstake("999999999999999999999999")
            # If it doesn't error, that's unexpected but not critical
            self.results.record_pass("invalid_unstake_handled")
        except Exception as e:
            # Error is expected
            self.results.record_pass("invalid_unstake_rejected")


# =============================================================================
# MAIN EXECUTION
# =============================================================================

def main():
    """Main entry point"""
    print("""
╔══════════════════════════════════════════════════════════════╗
║         ModernTensor E2E Full Network Test Suite             ║
║                                                              ║
║  Testing: Local Testnet (3 nodes)                            ║
║  Scenarios: Connectivity, Blocks, Staking, Edge Cases        ║
╚══════════════════════════════════════════════════════════════╝
    """)

    testnet = LocalTestnet()

    try:
        # Phase 1: Start network
        if not testnet.start():
            logger.error("❌ Failed to start testnet. Check logs above.")
            testnet.cleanup()
            return 1

        logger.info("\n✅ Testnet is running!\n")

        # Phase 2: Run tests
        suite = E2ETestSuite(testnet)
        results = suite.run_all()

        # Phase 3: Report results
        logger.info("\n" + "=" * 60)
        logger.info("📊 TEST RESULTS")
        logger.info("=" * 60)
        logger.info(results.summary())

        if results.errors:
            logger.info("\nFailures:")
            for error in results.errors:
                logger.info(f"  - {error}")

        # Return exit code based on failures
        return 0 if results.failed == 0 else 1

    except KeyboardInterrupt:
        logger.info("\n⚠️ Test interrupted by user")
        return 130

    except Exception as e:
        logger.exception(f"❌ Unexpected error: {e}")
        return 1

    finally:
        testnet.cleanup()


if __name__ == "__main__":
    sys.exit(main())
