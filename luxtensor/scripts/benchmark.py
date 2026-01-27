#!/usr/bin/env python3
"""
Luxtensor Performance Benchmark Suite

Measures key performance metrics for the blockchain.

Usage:
    python benchmark.py --rpc http://localhost:8545
    python benchmark.py --mode block_time
    python benchmark.py --mode state_ops
    python benchmark.py --mode all
"""

import argparse
import asyncio
import aiohttp
import json
import time
import statistics
from dataclasses import dataclass, field
from typing import List, Dict, Optional

# Configuration
DEFAULT_RPC = "http://localhost:8545"


@dataclass
class BenchmarkResult:
    """Result of a benchmark"""
    name: str
    metric: str
    target: str
    measured: str
    samples: List[float] = field(default_factory=list)
    passed: bool = False

    @property
    def avg(self) -> float:
        return statistics.mean(self.samples) if self.samples else 0

    @property
    def median(self) -> float:
        return statistics.median(self.samples) if self.samples else 0

    @property
    def p99(self) -> float:
        if not self.samples:
            return 0
        sorted_samples = sorted(self.samples)
        idx = int(len(sorted_samples) * 0.99)
        return sorted_samples[idx]


class Benchmarker:
    """Run performance benchmarks"""

    def __init__(self, rpc_url: str = DEFAULT_RPC):
        self.rpc_url = rpc_url

    async def _rpc_call(self, session: aiohttp.ClientSession, method: str, params: List = None):
        """Make RPC call and measure latency"""
        start = time.perf_counter()
        try:
            async with session.post(
                self.rpc_url,
                json={"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1},
                timeout=aiohttp.ClientTimeout(total=30),
            ) as response:
                result = await response.json()
                latency_ms = (time.perf_counter() - start) * 1000
                return (True, result.get("result"), latency_ms)
        except Exception as e:
            return (False, str(e), 0)

    async def benchmark_block_time(self, sample_count: int = 20) -> BenchmarkResult:
        """Measure actual block time"""
        print(f"\n⏱️  Benchmarking Block Time ({sample_count} samples)...")

        result = BenchmarkResult(
            name="Block Time",
            metric="Time between blocks",
            target="3 seconds",
            measured="",
        )

        async with aiohttp.ClientSession() as session:
            last_block_num = None
            last_block_time = None
            block_times = []

            for i in range(sample_count + 1):
                success, block_num_hex, _ = await self._rpc_call(session, "eth_blockNumber")

                if success and block_num_hex:
                    current_block_num = int(block_num_hex, 16)
                    current_time = time.time()

                    if last_block_num is not None and current_block_num > last_block_num:
                        block_time = current_time - last_block_time
                        block_times.append(block_time)
                        print(f"  Block {current_block_num}: {block_time:.2f}s")

                    last_block_num = current_block_num
                    last_block_time = current_time

                await asyncio.sleep(1)

        result.samples = block_times
        result.measured = f"{result.avg:.2f}s (avg), {result.median:.2f}s (median)"
        result.passed = 2.5 <= result.avg <= 3.5  # Within ±0.5s of target

        return result

    async def benchmark_finality_time(self, sample_count: int = 10) -> BenchmarkResult:
        """Measure time from TX submission to finality"""
        print(f"\n🔒 Benchmarking Finality Time ({sample_count} samples)...")

        result = BenchmarkResult(
            name="Finality Time",
            metric="TX submission to finality",
            target="~30 seconds",
            measured="",
        )

        async with aiohttp.ClientSession() as session:
            for i in range(sample_count):
                # Submit a transaction and measure time to finality
                tx = {
                    "from": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
                    "to": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
                    "value": hex(1000),
                    "gas": hex(21000),
                    "gasPrice": hex(20000000000),
                }

                start = time.perf_counter()
                success, tx_hash, _ = await self._rpc_call(session, "eth_sendTransaction", [tx])

                if success and tx_hash:
                    # Wait for finality (poll for receipt and finalized status)
                    finalized = False
                    while not finalized and (time.perf_counter() - start) < 120:
                        await asyncio.sleep(1)
                        success2, receipt, _ = await self._rpc_call(
                            session, "eth_getTransactionReceipt", [tx_hash]
                        )
                        if receipt:
                            # Check if block is finalized
                            block_num = receipt.get("blockNumber")
                            if block_num:
                                success3, finalized_block, _ = await self._rpc_call(
                                    session, "eth_getBlockByNumber", ["finalized", False]
                                )
                                if finalized_block:
                                    finalized_num = int(finalized_block.get("number", "0x0"), 16)
                                    if int(block_num, 16) <= finalized_num:
                                        finalized = True

                    finality_time = time.perf_counter() - start
                    result.samples.append(finality_time)
                    print(f"  TX {i+1}: {finality_time:.2f}s")

        result.measured = f"{result.avg:.2f}s (avg)"
        result.passed = result.avg <= 60  # Should finalize within 60s

        return result

    async def benchmark_rpc_latency(self, iterations: int = 100) -> BenchmarkResult:
        """Measure RPC call latency"""
        print(f"\n📡 Benchmarking RPC Latency ({iterations} calls)...")

        result = BenchmarkResult(
            name="RPC Latency",
            metric="eth_blockNumber call",
            target="< 10ms",
            measured="",
        )

        async with aiohttp.ClientSession() as session:
            for i in range(iterations):
                _, _, latency = await self._rpc_call(session, "eth_blockNumber")
                if latency > 0:
                    result.samples.append(latency)

        result.measured = f"{result.avg:.2f}ms (avg), {result.p99:.2f}ms (p99)"
        result.passed = result.avg < 10

        return result

    async def benchmark_state_read(self, iterations: int = 100) -> BenchmarkResult:
        """Measure state read latency"""
        print(f"\n📖 Benchmarking State Read ({iterations} calls)...")

        result = BenchmarkResult(
            name="State Read",
            metric="eth_getBalance call",
            target="< 1ms",
            measured="",
        )

        test_address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

        async with aiohttp.ClientSession() as session:
            for i in range(iterations):
                _, _, latency = await self._rpc_call(
                    session, "eth_getBalance", [test_address, "latest"]
                )
                if latency > 0:
                    result.samples.append(latency)

        result.measured = f"{result.avg:.2f}ms (avg), {result.p99:.2f}ms (p99)"
        result.passed = result.avg < 1

        return result

    async def benchmark_state_root_calculation(self) -> BenchmarkResult:
        """Estimate state root calculation time from block processing"""
        print("\n🌳 Benchmarking State Root Calculation...")

        result = BenchmarkResult(
            name="State Root",
            metric="State root calculation",
            target="< 100ms",
            measured="",
        )

        # We estimate this by measuring block retrieval time differences
        # which includes state root access
        async with aiohttp.ClientSession() as session:
            for _ in range(20):
                start = time.perf_counter()
                success, block, _ = await self._rpc_call(
                    session, "eth_getBlockByNumber", ["latest", True]
                )
                latency_ms = (time.perf_counter() - start) * 1000

                if success and block:
                    result.samples.append(latency_ms)

        result.measured = f"{result.avg:.2f}ms (block retrieval)"
        result.passed = result.avg < 100

        return result


async def main():
    parser = argparse.ArgumentParser(description="Luxtensor Performance Benchmark")
    parser.add_argument(
        "--rpc",
        default=DEFAULT_RPC,
        help=f"RPC endpoint (default: {DEFAULT_RPC})",
    )
    parser.add_argument(
        "--mode",
        choices=["block_time", "finality", "rpc", "state", "all"],
        default="all",
        help="Benchmark to run",
    )

    args = parser.parse_args()

    print("=" * 60)
    print("       LUXTENSOR PERFORMANCE BENCHMARK")
    print("=" * 60)
    print(f"RPC: {args.rpc}")
    print("=" * 60)

    benchmarker = Benchmarker(args.rpc)
    results: List[BenchmarkResult] = []

    if args.mode in ["block_time", "all"]:
        results.append(await benchmarker.benchmark_block_time())

    if args.mode in ["finality", "all"]:
        results.append(await benchmarker.benchmark_finality_time())

    if args.mode in ["rpc", "all"]:
        results.append(await benchmarker.benchmark_rpc_latency())

    if args.mode in ["state", "all"]:
        results.append(await benchmarker.benchmark_state_read())
        results.append(await benchmarker.benchmark_state_root_calculation())

    # Print summary
    print("\n" + "=" * 60)
    print("                 BENCHMARK RESULTS")
    print("=" * 60)
    print(f"{'Metric':<25} {'Target':<15} {'Measured':<25} {'Status'}")
    print("-" * 75)

    all_passed = True
    for r in results:
        status = "✅ PASS" if r.passed else "❌ FAIL"
        print(f"{r.name:<25} {r.target:<15} {r.measured:<25} {status}")
        all_passed = all_passed and r.passed

    print("=" * 60)
    overall = "✅ ALL BENCHMARKS PASSED" if all_passed else "⚠️ SOME BENCHMARKS FAILED"
    print(f"  {overall}")

    return 0 if all_passed else 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    exit(exit_code)
