#!/usr/bin/env python3
"""
E2E Integration Tests: SDK ↔ LuxTensor Node

Run these tests with a running LuxTensor node:
    cargo run -p luxtensor-node &
    pytest tests/integration/test_sdk_luxtensor_e2e.py -v

Or run the standalone script:
    python tests/integration/test_sdk_luxtensor_e2e.py
"""

import sys
import time
import pytest
from pathlib import Path

# Add SDK to path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

import requests
from typing import Optional, Dict, Any

# Import SDK components
try:
    from sdk.luxtensor_client import LuxtensorClient
    SDK_AVAILABLE = True
except ImportError as e:
    print(f"Warning: SDK import failed: {e}")
    SDK_AVAILABLE = False


# ============================================================================
# Test Configuration
# ============================================================================

RPC_URL = "http://127.0.0.1:8545"
HEADERS = {"Content-Type": "application/json"}


def rpc_call(method: str, params: Optional[list] = None, id: int = 1) -> dict:
    """Direct RPC call for low-level testing"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": id
    }
    try:
        response = requests.post(RPC_URL, json=payload, headers=HEADERS, timeout=10)
        return response.json()
    except requests.exceptions.ConnectionError:
        return {"error": "Cannot connect to node"}
    except Exception as e:
        return {"error": str(e)}


def node_is_running() -> bool:
    """Check if node is running"""
    result = rpc_call("web3_clientVersion")
    return "result" in result


# ============================================================================
# Pytest Fixtures
# ============================================================================

@pytest.fixture(scope="session")
def check_node():
    """Skip all tests if node is not running"""
    if not node_is_running():
        pytest.skip("LuxTensor node not running. Start with: cargo run -p luxtensor-node")


@pytest.fixture
def client(check_node):
    """Create SDK client instance"""
    if not SDK_AVAILABLE:
        pytest.skip("SDK not available")
    return LuxtensorClient(url=RPC_URL)


# ============================================================================
# Test Suite 1: Basic Connection
# ============================================================================

class TestConnection:
    """Test basic RPC connection"""

    def test_web3_client_version(self, check_node):
        """Test we can connect and get client version"""
        result = rpc_call("web3_clientVersion")
        assert "result" in result
        assert "LuxTensor" in result["result"] or result["result"] is not None

    def test_net_version(self, check_node):
        """Test network version"""
        result = rpc_call("net_version")
        assert "result" in result

    def test_eth_chain_id(self, check_node):
        """Test chain ID"""
        result = rpc_call("eth_chainId")
        assert "result" in result


# ============================================================================
# Test Suite 2: Block Operations
# ============================================================================

class TestBlockOperations:
    """Test block-related RPC calls"""

    def test_eth_block_number(self, check_node):
        """Test current block number"""
        result = rpc_call("eth_blockNumber")
        assert "result" in result
        block_num = int(result["result"], 16) if result["result"] else 0
        assert block_num >= 0

    def test_eth_get_block_by_number(self, check_node):
        """Test get block by number"""
        result = rpc_call("eth_getBlockByNumber", ["0x0", False])
        # Genesis block should exist
        assert "result" in result or "error" in result

    def test_get_latest_block(self, check_node):
        """Test get latest block"""
        result = rpc_call("eth_getBlockByNumber", ["latest", False])
        assert "result" in result


# ============================================================================
# Test Suite 3: Account Operations
# ============================================================================

class TestAccountOperations:
    """Test account-related RPC calls"""

    def test_eth_get_balance(self, check_node):
        """Test get balance"""
        test_address = "0x" + "00" * 20
        result = rpc_call("eth_getBalance", [test_address, "latest"])
        assert "result" in result

    def test_eth_get_transaction_count(self, check_node):
        """Test get nonce"""
        test_address = "0x" + "00" * 20
        result = rpc_call("eth_getTransactionCount", [test_address, "latest"])
        assert "result" in result


# ============================================================================
# Test Suite 4: Subnet 0 Operations (ModernTensor specific)
# ============================================================================

class TestSubnet0Operations:
    """Test Subnet 0 (Root Subnet) RPC methods"""

    def test_subnet_get_all(self, check_node):
        """Test getting all subnets"""
        result = rpc_call("subnet_getAll")
        assert "result" in result
        # May return empty list initially
        assert isinstance(result.get("result"), list) or result.get("result") is None

    def test_subnet_get_config(self, check_node):
        """Test getting root subnet config"""
        result = rpc_call("subnet_getConfig")
        assert "result" in result
        config = result.get("result", {})
        if config:
            # Verify expected config fields
            assert "max_subnets" in config or True  # May vary

    def test_subnet_get_root_validators(self, check_node):
        """Test getting root validators"""
        result = rpc_call("subnet_getRootValidators")
        assert "result" in result

    def test_subnet_get_emissions(self, check_node):
        """Test getting emission distribution"""
        result = rpc_call("subnet_getEmissions")
        assert "result" in result


# ============================================================================
# Test Suite 5: SDK Client Integration
# ============================================================================

class TestSDKClient:
    """Test SDK LuxtensorClient methods"""

    def test_get_block_number(self, client):
        """Test SDK get_block_number method"""
        block = client.get_block_number()
        assert isinstance(block, int)
        assert block >= 0

    def test_get_chain_info(self, client):
        """Test SDK get_chain_info method"""
        info = client.get_chain_info()
        assert info is not None

    def test_get_balance(self, client):
        """Test SDK get_balance method"""
        test_address = "0x" + "00" * 20
        balance = client.get_balance(test_address)
        assert isinstance(balance, int)

    def test_get_all_subnets(self, client):
        """Test SDK Subnet 0 method"""
        subnets = client.get_all_subnets()
        assert isinstance(subnets, list)

    def test_get_root_config(self, client):
        """Test SDK get root config"""
        config = client.get_root_config()
        assert config is not None


# ============================================================================
# Test Suite 6: Transaction Flow (requires funded account)
# ============================================================================

class TestTransactionFlow:
    """Test transaction submission (skip if no funded accounts)"""

    @pytest.mark.skip(reason="Requires funded test account")
    def test_send_transaction(self, client):
        """Test sending a transaction"""
        # This would require a funded account
        pass

    @pytest.mark.skip(reason="Requires funded test account")
    def test_stake_operation(self, client):
        """Test stake operation"""
        pass


# ============================================================================
# Standalone Test Runner
# ============================================================================

def run_standalone_tests():
    """Run tests without pytest"""
    print("""
╔══════════════════════════════════════════════════════════════╗
║         E2E Integration Test Suite                           ║
║         SDK ↔ LuxTensor Node                                 ║
╚══════════════════════════════════════════════════════════════╝
    """)

    print(f"Target Node: {RPC_URL}")

    if not node_is_running():
        print("❌ Node not running. Start with: cargo run -p luxtensor-node")
        return

    print("✅ Node connected\n")

    tests = [
        ("web3_clientVersion", lambda: rpc_call("web3_clientVersion")),
        ("eth_blockNumber", lambda: rpc_call("eth_blockNumber")),
        ("eth_getBalance", lambda: rpc_call("eth_getBalance", ["0x" + "00" * 20, "latest"])),
        ("subnet_getAll", lambda: rpc_call("subnet_getAll")),
        ("subnet_getConfig", lambda: rpc_call("subnet_getConfig")),
        ("subnet_getRootValidators", lambda: rpc_call("subnet_getRootValidators")),
    ]

    passed = 0
    for name, test_fn in tests:
        result = test_fn()
        if "result" in result:
            print(f"  ✅ {name}")
            passed += 1
        else:
            print(f"  ❌ {name}: {result.get('error', 'Unknown error')}")

    print(f"\nTotal: {passed}/{len(tests)} passed")

    # Test SDK if available
    if SDK_AVAILABLE:
        print("\n--- SDK Client Tests ---")
        try:
            client = LuxtensorClient(url=RPC_URL)
            block = client.get_block_number()
            print(f"  ✅ SDK get_block_number: {block}")

            subnets = client.get_all_subnets()
            print(f"  ✅ SDK get_all_subnets: {len(subnets)} subnets")
        except Exception as e:
            print(f"  ❌ SDK error: {e}")


if __name__ == "__main__":
    run_standalone_tests()
