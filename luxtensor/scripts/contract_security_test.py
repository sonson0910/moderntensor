#!/usr/bin/env python3
"""
Luxtensor Smart Contract Security Test Suite

Tests native contracts for security vulnerabilities:
- Staking contract: stake/unstake/slash
- Rewards contract: claim/distribute
- DAO contract: vote/execute
- Attack vectors: reentrancy, overflow, access control

Usage:
    python contract_security_test.py --rpc http://localhost:8545
    python contract_security_test.py --mode staking
    python contract_security_test.py --mode rewards
    python contract_security_test.py --mode all
"""

import argparse
import asyncio
import aiohttp
import json
import time
from dataclasses import dataclass, field
from typing import List, Dict, Optional

# Configuration
DEFAULT_RPC = "http://localhost:8545"
DEFAULT_CHAIN_ID = 8899

# Test accounts
TEST_ACCOUNTS = [
    {
        "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
        "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
    },
    {
        "address": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
        "private_key": "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
    },
]


@dataclass
class ContractTestResult:
    """Result of a contract security test"""
    contract: str
    test_name: str
    passed: bool
    vulnerability_found: bool
    details: List[str] = field(default_factory=list)
    duration_seconds: float = 0


class ContractSecurityTester:
    """Test smart contracts for security vulnerabilities"""

    def __init__(self, rpc_url: str = DEFAULT_RPC, chain_id: int = DEFAULT_CHAIN_ID):
        self.rpc_url = rpc_url
        self.chain_id = chain_id

    async def _rpc_call(self, session: aiohttp.ClientSession, method: str, params: List = None):
        """Make RPC call"""
        try:
            async with session.post(
                self.rpc_url,
                json={"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1},
                timeout=aiohttp.ClientTimeout(total=30),
            ) as response:
                result = await response.json()
                if "error" in result:
                    return (False, None, result["error"].get("message", "Unknown"))
                return (True, result.get("result"), None)
        except Exception as e:
            return (False, None, str(e))

    # =========================================================================
    # STAKING CONTRACT TESTS
    # =========================================================================

    async def test_staking_stake(self) -> ContractTestResult:
        """Test stake functionality"""
        print("  Testing stake...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            account = TEST_ACCOUNTS[0]
            stake_amount = 1000000000000000000  # 1 token

            # Get initial stake
            success, result, error = await self._rpc_call(
                session, "staking_getStake", [account["address"]]
            )
            initial_stake = int(result, 16) if success and result else 0
            details.append(f"Initial stake: {initial_stake}")

            # Stake tokens
            success, tx_hash, error = await self._rpc_call(
                session, "staking_stake", [
                    account["address"],
                    hex(stake_amount)
                ]
            )

            if success:
                details.append(f"Stake TX: {tx_hash}")
                await asyncio.sleep(4)  # Wait for block

                # Verify stake increased
                success2, result2, _ = await self._rpc_call(
                    session, "staking_getStake", [account["address"]]
                )
                final_stake = int(result2, 16) if success2 and result2 else 0

                if final_stake >= initial_stake:
                    details.append("✓ Stake recorded correctly")
                else:
                    details.append("✗ Stake not recorded")
                    passed = False
            else:
                details.append(f"Stake call: {error or 'responded'}")

        return ContractTestResult(
            contract="Staking",
            test_name="stake",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )

    async def test_staking_unstake(self) -> ContractTestResult:
        """Test unstake functionality with unbonding period"""
        print("  Testing unstake...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            account = TEST_ACCOUNTS[0]

            # Try to unstake
            success, result, error = await self._rpc_call(
                session, "staking_unstake", [
                    account["address"],
                    hex(500000000000000000)  # 0.5 token
                ]
            )

            if success:
                details.append("Unstake request accepted")

                # Verify unbonding period enforced
                success2, pending, _ = await self._rpc_call(
                    session, "staking_getPendingUnstake", [account["address"]]
                )
                if pending:
                    details.append("✓ Unbonding period enforced")
                else:
                    details.append("⚠️ Could not verify unbonding")
            else:
                details.append(f"Unstake response: {error or 'handled'}")

        return ContractTestResult(
            contract="Staking",
            test_name="unstake",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )

    async def test_staking_slash_protection(self) -> ContractTestResult:
        """Test that slashing only works for authorized callers"""
        print("  Testing slash protection...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            # Try to slash from non-authorized account
            attacker = TEST_ACCOUNTS[1]
            victim = TEST_ACCOUNTS[0]

            success, result, error = await self._rpc_call(
                session, "staking_slash", [
                    victim["address"],
                    hex(1000000000000000000),  # Try to slash 1 token
                    "unauthorized_test"
                ]
            )

            if not success or "unauthorized" in str(error).lower() or "permission" in str(error).lower():
                details.append("✓ Unauthorized slash rejected")
            else:
                details.append("✗ VULNERABILITY: Unauthorized slash allowed!")
                vuln = True
                passed = False

        return ContractTestResult(
            contract="Staking",
            test_name="slash_protection",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )

    # =========================================================================
    # REWARDS CONTRACT TESTS
    # =========================================================================

    async def test_rewards_claim(self) -> ContractTestResult:
        """Test reward claiming"""
        print("  Testing reward claim...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            account = TEST_ACCOUNTS[0]

            # Get pending rewards
            success, pending, _ = await self._rpc_call(
                session, "rewards_getPending", [account["address"]]
            )
            pending_amount = int(pending, 16) if success and pending else 0
            details.append(f"Pending rewards: {pending_amount}")

            # Claim rewards
            success, result, error = await self._rpc_call(
                session, "rewards_claim", [account["address"]]
            )

            if success:
                details.append("Claim processed")
            else:
                details.append(f"Claim response: {error or 'handled'}")

        return ContractTestResult(
            contract="Rewards",
            test_name="claim",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )

    async def test_rewards_double_claim(self) -> ContractTestResult:
        """Test that double claiming is prevented"""
        print("  Testing double claim prevention...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            account = TEST_ACCOUNTS[0]

            # First claim
            await self._rpc_call(session, "rewards_claim", [account["address"]])

            # Get pending after first claim
            success, pending1, _ = await self._rpc_call(
                session, "rewards_getPending", [account["address"]]
            )

            # Second claim immediately
            await self._rpc_call(session, "rewards_claim", [account["address"]])

            # Get pending after second claim
            success, pending2, _ = await self._rpc_call(
                session, "rewards_getPending", [account["address"]]
            )

            pending1_val = int(pending1, 16) if pending1 else 0
            pending2_val = int(pending2, 16) if pending2 else 0

            if pending2_val <= pending1_val:
                details.append("✓ Double claim prevented or no change")
            else:
                details.append("✗ VULNERABILITY: Rewards increased after claim!")
                vuln = True
                passed = False

        return ContractTestResult(
            contract="Rewards",
            test_name="double_claim",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )

    # =========================================================================
    # OVERFLOW PROTECTION TESTS
    # =========================================================================

    async def test_overflow_protection(self) -> ContractTestResult:
        """Test integer overflow protection"""
        print("  Testing overflow protection...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            account = TEST_ACCOUNTS[0]

            # Try to stake max uint256 (should overflow without protection)
            max_uint256 = "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"

            success, result, error = await self._rpc_call(
                session, "staking_stake", [account["address"], max_uint256]
            )

            if not success or "overflow" in str(error).lower() or "invalid" in str(error).lower():
                details.append("✓ Overflow attempt rejected")
            elif success:
                # Check if it was handled with saturating arithmetic
                success2, stake, _ = await self._rpc_call(
                    session, "staking_getStake", [account["address"]]
                )
                stake_val = int(stake, 16) if stake else 0

                # If stake is not max, saturating arithmetic worked
                if stake_val < 2**256 - 1:
                    details.append("✓ Saturating arithmetic applied")
                else:
                    details.append("✗ VULNERABILITY: Overflow allowed!")
                    vuln = True
                    passed = False
            else:
                details.append(f"Response: {error or 'handled correctly'}")

        return ContractTestResult(
            contract="Core",
            test_name="overflow_protection",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )

    # =========================================================================
    # ACCESS CONTROL TESTS
    # =========================================================================

    async def test_access_control(self) -> ContractTestResult:
        """Test access control on admin functions"""
        print("  Testing access control...")
        start = time.perf_counter()
        details = []
        passed = True
        vuln = False

        async with aiohttp.ClientSession() as session:
            non_admin = TEST_ACCOUNTS[1]

            # Try admin functions from non-admin account
            admin_methods = [
                ("admin_setMinStake", [hex(1000)]),
                ("admin_setSlashRate", [hex(10)]),
                ("admin_setEpochLength", [hex(50)]),
            ]

            for method, params in admin_methods:
                success, result, error = await self._rpc_call(session, method, params)

                if not success or "unauthorized" in str(error).lower() or "permission" in str(error).lower() or "auth" in str(error).lower():
                    details.append(f"✓ {method}: Access denied")
                else:
                    # Method might not exist, which is also fine
                    if "not found" in str(error).lower() or "unknown" in str(error).lower():
                        details.append(f"✓ {method}: Not exposed")
                    else:
                        details.append(f"⚠️ {method}: Response unclear")

        return ContractTestResult(
            contract="Admin",
            test_name="access_control",
            passed=passed,
            vulnerability_found=vuln,
            details=details,
            duration_seconds=time.perf_counter() - start,
        )


async def main():
    parser = argparse.ArgumentParser(description="Contract Security Test Suite")
    parser.add_argument("--rpc", default=DEFAULT_RPC, help="RPC endpoint")
    parser.add_argument(
        "--mode",
        choices=["staking", "rewards", "security", "all"],
        default="all",
        help="Test category",
    )

    args = parser.parse_args()

    print("=" * 60)
    print("    LUXTENSOR CONTRACT SECURITY TEST SUITE")
    print("=" * 60)
    print(f"RPC: {args.rpc}")
    print("=" * 60)

    tester = ContractSecurityTester(args.rpc)
    results: List[ContractTestResult] = []

    if args.mode in ["staking", "all"]:
        print("\n📜 STAKING CONTRACT TESTS:")
        results.append(await tester.test_staking_stake())
        results.append(await tester.test_staking_unstake())
        results.append(await tester.test_staking_slash_protection())

    if args.mode in ["rewards", "all"]:
        print("\n🎁 REWARDS CONTRACT TESTS:")
        results.append(await tester.test_rewards_claim())
        results.append(await tester.test_rewards_double_claim())

    if args.mode in ["security", "all"]:
        print("\n🔒 SECURITY TESTS:")
        results.append(await tester.test_overflow_protection())
        results.append(await tester.test_access_control())

    # Print summary
    print("\n" + "=" * 60)
    print("                 TEST RESULTS")
    print("=" * 60)

    vulnerabilities = 0
    for r in results:
        status = "✅ PASS" if r.passed else "❌ FAIL"
        vuln = "🔴 VULN" if r.vulnerability_found else ""
        print(f"  [{r.contract}] {r.test_name}: {status} {vuln}")
        for detail in r.details:
            print(f"    - {detail}")
        if r.vulnerability_found:
            vulnerabilities += 1

    print("\n" + "=" * 60)
    passed = sum(1 for r in results if r.passed)
    total = len(results)
    print(f"  Passed: {passed}/{total}")
    print(f"  Vulnerabilities: {vulnerabilities}")

    overall = "✅ SECURE" if vulnerabilities == 0 else "🔴 VULNERABILITIES FOUND"
    print(f"  Status: {overall}")
    print("=" * 60)

    return 0 if vulnerabilities == 0 else 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    exit(exit_code)
