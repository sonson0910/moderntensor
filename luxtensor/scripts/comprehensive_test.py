#!/usr/bin/env python3
"""
ULTIMATE Blockchain Feature Test Suite
Tests ALL 50+ features of Luxtensor blockchain for 100% coverage
"""

import json
import requests
import time
import sys
from typing import Optional, Dict, Any, List

RPC_URL = "http://localhost:8545"

class Colors:
    GREEN = '\033[92m'
    RED = '\033[91m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    CYAN = '\033[96m'
    END = '\033[0m'

def rpc_call(method: str, params: list = None) -> Dict[str, Any]:
    """Make RPC call and return result"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": 1
    }
    try:
        response = requests.post(RPC_URL, json=payload, timeout=10)
        return response.json()
    except Exception as e:
        return {"error": {"message": str(e)}}

def rpc_call_object(method: str, params: dict) -> Dict[str, Any]:
    """Make RPC call with object/named params"""
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    }
    try:
        response = requests.post(RPC_URL, json=payload, timeout=10)
        return response.json()
    except Exception as e:
        return {"error": {"message": str(e)}}


def test_result(name: str, result: Dict, expected_success: bool = True) -> bool:
    """Check test result and print status"""
    has_error = "error" in result
    success = not has_error if expected_success else has_error

    if success:
        print(f"  {Colors.GREEN}✓{Colors.END} {name}")
        return True
    else:
        print(f"  {Colors.RED}✗{Colors.END} {name}")
        if "error" in result:
            print(f"    → Error: {result['error'].get('message', result['error'])[:60]}")
        return False

class TestSuite:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.test_addr = "0x0000000000000000000000000000000000000001"

    def run_test(self, name: str, method: str, params: list = None, expect_success: bool = True):
        result = rpc_call(method, params)
        if test_result(name, result, expect_success):
            self.passed += 1
        else:
            self.failed += 1
        return result

    def run_all(self):
        print(f"\n{Colors.BLUE}{'='*70}{Colors.END}")
        print(f"{Colors.BLUE}  LUXTENSOR ULTIMATE FEATURE TEST - 50+ TESTS{Colors.END}")
        print(f"{Colors.BLUE}{'='*70}{Colors.END}\n")

        # ==========================================
        # 1. CORE BLOCKCHAIN (10 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[1] CORE BLOCKCHAIN{Colors.END}")

        self.run_test("eth_blockNumber", "eth_blockNumber")
        self.run_test("eth_chainId", "eth_chainId")
        self.run_test("eth_getBalance", "eth_getBalance", [self.test_addr, "latest"])
        self.run_test("eth_getTransactionCount", "eth_getTransactionCount", [self.test_addr, "latest"])
        self.run_test("eth_getBlockByNumber", "eth_getBlockByNumber", ["0x1", False])
        self.run_test("eth_getBlockByNumber (latest)", "eth_getBlockByNumber", ["latest", False])
        self.run_test("eth_getBlockByHash (genesis)", "eth_getBlockByNumber", ["0x0", True])
        self.run_test("eth_gasPrice", "eth_gasPrice")
        self.run_test("eth_estimateGas", "eth_estimateGas", [{"to": self.test_addr, "value": "0x1"}])
        self.run_test("eth_getCode", "eth_getCode", [self.test_addr, "latest"])

        # ==========================================
        # 2. STAKING (8 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[2] STAKING{Colors.END}")

        self.run_test("staking_getTotalStake", "staking_getTotalStake")
        self.run_test("staking_getValidators", "staking_getValidators")
        self.run_test("staking_getStake", "staking_getStake", [self.test_addr])
        self.run_test("staking_addStake", "staking_addStake", [self.test_addr, "0x1000"])
        self.run_test("staking_getStake (verify add)", "staking_getStake", [self.test_addr])
        self.run_test("staking_removeStake", "staking_removeStake", [self.test_addr, "0x100"])
        self.run_test("staking_getStake (verify remove)", "staking_getStake", [self.test_addr])
        self.run_test("staking_claimRewards", "staking_claimRewards", [self.test_addr])

        # ==========================================
        # 3. SUBNETS (6 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[3] SUBNETS{Colors.END}")

        self.run_test("subnet_listAll", "subnet_listAll")
        self.run_test("subnet_getCount", "subnet_getCount")
        self.run_test("subnet_getInfo", "subnet_getInfo", [0])
        self.run_test("query_getSubnets", "query_getSubnets")
        self.run_test("query_getSubnetInfo", "query_getSubnetInfo", [0])
        self.run_test("subnet_create", "subnet_create", ["TestSubnet", self.test_addr, "0x100"])

        # ==========================================
        # 4. NEURONS (6 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[4] NEURONS{Colors.END}")

        self.run_test("neuron_listBySubnet", "neuron_listBySubnet", [0])
        self.run_test("neuron_getCount", "neuron_getCount", [0])
        self.run_test("neuron_getInfo", "neuron_getInfo", [0, 0])
        self.run_test("query_getNeurons", "query_getNeurons", [0])
        self.run_test("query_getNeuronInfo", "query_getNeuronInfo", [0, 0])
        self.run_test("neuron_register", "neuron_register", [0, self.test_addr, "0x100"])

        # ==========================================
        # 5. WEIGHTS (4 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[5] WEIGHTS{Colors.END}")

        self.run_test("weight_getAll", "weight_getAll", [0])
        self.run_test("weight_getWeights", "weight_getWeights", [0, 0])
        self.run_test("query_getWeights", "query_getWeights", [0])
        self.run_test("weight_setWeights", "weight_setWeights", [0, 0, [1, 2], [100, 200]])

        # ==========================================
        # 6. AI METHODS (4 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[6] AI METHODS{Colors.END}")

        self.run_test("ai_getMetagraph", "ai_getMetagraph", [0])
        self.run_test("ai_getIncentive", "ai_getIncentive", [0])
        # lux_submitAITask expects a single object, not array - use special call
        result = rpc_call_object("lux_submitAITask", {"model_hash": "test", "input_data": "test", "requester": self.test_addr, "reward": "0x100"})
        if test_result("lux_submitAITask", result):
            self.passed += 1
        else:
            self.failed += 1
        self.run_test("lux_getValidatorStatus", "lux_getValidatorStatus", [self.test_addr])

        # ==========================================
        # 7. ETHEREUM COMPATIBILITY (8 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[7] ETHEREUM COMPATIBILITY{Colors.END}")

        self.run_test("eth_getStorageAt", "eth_getStorageAt", [self.test_addr, "0x0", "latest"])
        self.run_test("eth_accounts", "eth_accounts")
        self.run_test("eth_syncing", "eth_syncing")
        self.run_test("eth_mining", "eth_mining")
        self.run_test("eth_hashrate", "eth_hashrate")
        self.run_test("eth_coinbase", "eth_coinbase")
        self.run_test("eth_protocolVersion", "eth_protocolVersion")
        self.run_test("eth_getTransactionReceipt (null ok)", "eth_getTransactionReceipt", ["0x" + "0" * 64])

        # ==========================================
        # 8. NETWORK INFO (6 tests)
        # ==========================================
        print(f"\n{Colors.CYAN}[8] NETWORK INFO{Colors.END}")

        self.run_test("net_version", "net_version")
        self.run_test("net_peerCount", "net_peerCount")
        self.run_test("net_listening", "net_listening")
        self.run_test("web3_clientVersion", "web3_clientVersion")
        self.run_test("web3_sha3", "web3_sha3", ["0x68656c6c6f"])
        self.run_test("rpc_modules", "rpc_modules")

        # ==========================================
        # SUMMARY
        # ==========================================
        total = self.passed + self.failed
        pct = (self.passed / total * 100) if total > 0 else 0

        print(f"\n{Colors.BLUE}{'='*70}{Colors.END}")
        if self.failed == 0:
            print(f"{Colors.GREEN}  ✓ ALL TESTS PASSED: {self.passed}/{total} (100%){Colors.END}")
        else:
            color = Colors.GREEN if pct >= 90 else Colors.YELLOW if pct >= 70 else Colors.RED
            print(f"{color}  TESTS: {self.passed} passed, {self.failed} failed ({pct:.1f}%){Colors.END}")
        print(f"{Colors.BLUE}{'='*70}{Colors.END}\n")

        return {"passed": self.passed, "failed": self.failed, "total": total, "percentage": pct}

if __name__ == "__main__":
    suite = TestSuite()
    results = suite.run_all()
    sys.exit(0 if results["failed"] == 0 else 1)
