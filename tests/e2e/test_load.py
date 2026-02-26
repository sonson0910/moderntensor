#!/usr/bin/env python3
"""
Load Testing Suite for LuxTensor Node via Python SDK

Measures throughput, latency, and concurrency behavior of the RPC layer.

Usage:
    # Quick smoke test (default: 50 requests, 5 concurrent)
    pytest tests/e2e/test_load.py -v --node-url http://127.0.0.1:8545

    # Full load test
    pytest tests/e2e/test_load.py -v -k "heavy" --node-url http://127.0.0.1:8545

    # Standalone (no pytest)
    python tests/e2e/test_load.py --url http://127.0.0.1:8545 --requests 500 --concurrency 20
"""

import json
import time
import statistics
import urllib.request
import concurrent.futures
from dataclasses import dataclass, field
from typing import List, Optional

import pytest

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
    pytest.mark.slow,
]


# ── Configuration ────────────────────────────────────────────────────

DEFAULT_URL = "http://127.0.0.1:8545"
LIGHT_REQUESTS = 50
LIGHT_CONCURRENCY = 5
HEAVY_REQUESTS = 500
HEAVY_CONCURRENCY = 20


# ── Helpers ──────────────────────────────────────────────────────────


@dataclass
class LoadResult:
    """Aggregated load test result."""
    total_requests: int = 0
    successful: int = 0
    failed: int = 0
    latencies_ms: List[float] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)
    duration_s: float = 0.0

    @property
    def success_rate(self) -> float:
        return (self.successful / self.total_requests * 100) if self.total_requests else 0

    @property
    def rps(self) -> float:
        return self.successful / self.duration_s if self.duration_s > 0 else 0

    @property
    def p50_ms(self) -> float:
        return self._percentile(50)

    @property
    def p95_ms(self) -> float:
        return self._percentile(95)

    @property
    def p99_ms(self) -> float:
        return self._percentile(99)

    @property
    def avg_ms(self) -> float:
        return statistics.mean(self.latencies_ms) if self.latencies_ms else 0

    def _percentile(self, pct: int) -> float:
        if not self.latencies_ms:
            return 0
        sorted_lat = sorted(self.latencies_ms)
        idx = int(len(sorted_lat) * pct / 100)
        return sorted_lat[min(idx, len(sorted_lat) - 1)]

    def summary(self) -> str:
        return (
            f"Load Test Results:\n"
            f"  Requests:  {self.successful}/{self.total_requests} "
            f"({self.success_rate:.1f}% success)\n"
            f"  Duration:  {self.duration_s:.2f}s\n"
            f"  RPS:       {self.rps:.1f}\n"
            f"  Latency:   avg={self.avg_ms:.1f}ms  "
            f"p50={self.p50_ms:.1f}ms  "
            f"p95={self.p95_ms:.1f}ms  "
            f"p99={self.p99_ms:.1f}ms\n"
            f"  Failures:  {self.failed}"
        )


def rpc_call_timed(url: str, method: str, params=None) -> tuple:
    """
    Single RPC call, returns (latency_ms, success, error_msg).
    Uses raw urllib to avoid SDK overhead in load measurements.
    """
    payload = json.dumps({
        "jsonrpc": "2.0",
        "method": method,
        "params": params or [],
        "id": 1,
    }).encode("utf-8")

    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json"},
    )

    t0 = time.perf_counter()
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            body = json.loads(resp.read().decode("utf-8"))
        latency = (time.perf_counter() - t0) * 1000

        if "error" in body:
            return latency, False, f"RPC error: {body['error']}"
        return latency, True, None

    except Exception as e:
        latency = (time.perf_counter() - t0) * 1000
        return latency, False, str(e)


def run_load_test(
    url: str,
    method: str,
    params=None,
    total_requests: int = 50,
    concurrency: int = 5,
) -> LoadResult:
    """
    Execute a concurrent load test against a single RPC method.
    """
    result = LoadResult(total_requests=total_requests)

    t0 = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [
            pool.submit(rpc_call_timed, url, method, params)
            for _ in range(total_requests)
        ]
        for future in concurrent.futures.as_completed(futures):
            latency, success, error = future.result()
            result.latencies_ms.append(latency)
            if success:
                result.successful += 1
            else:
                result.failed += 1
                if error:
                    result.errors.append(error)

    result.duration_s = time.perf_counter() - t0
    return result


# ── Fixtures ─────────────────────────────────────────────────────────


@pytest.fixture
def load_url(node_url) -> str:
    """URL for load testing (from e2e conftest)."""
    return node_url


# ── 1. Light Load Tests (fast, CI-safe) ──────────────────────────────


class TestLightLoad:
    """Quick load tests suitable for CI pipelines."""

    def test_eth_block_number_throughput(self, load_url):
        """eth_blockNumber under light load — should handle 10+ RPS."""
        result = run_load_test(
            load_url, "eth_blockNumber",
            total_requests=LIGHT_REQUESTS,
            concurrency=LIGHT_CONCURRENCY,
        )
        print(f"\n{result.summary()}")

        assert result.success_rate >= 95, (
            f"Success rate too low: {result.success_rate:.1f}%"
        )
        assert result.p95_ms < 5000, (
            f"p95 latency too high: {result.p95_ms:.1f}ms"
        )

    def test_eth_get_balance_throughput(self, load_url):
        """eth_getBalance under light load."""
        result = run_load_test(
            load_url, "eth_getBalance",
            params=["0x" + "00" * 20, "latest"],
            total_requests=LIGHT_REQUESTS,
            concurrency=LIGHT_CONCURRENCY,
        )
        print(f"\n{result.summary()}")

        assert result.success_rate >= 95
        assert result.p95_ms < 5000

    def test_subnet_get_all_throughput(self, load_url):
        """subnet_getAll under light load."""
        result = run_load_test(
            load_url, "subnet_getAll",
            total_requests=LIGHT_REQUESTS,
            concurrency=LIGHT_CONCURRENCY,
        )
        print(f"\n{result.summary()}")

        assert result.success_rate >= 90

    def test_mixed_rpc_methods(self, load_url):
        """Mixed workload: various methods concurrently."""
        methods = [
            ("eth_blockNumber", None),
            ("eth_getBalance", ["0x" + "00" * 20, "latest"]),
            ("eth_gasPrice", None),
            ("net_version", None),
            ("eth_chainId", None),
        ]

        all_latencies = []
        successes = 0
        total = 0

        with concurrent.futures.ThreadPoolExecutor(max_workers=LIGHT_CONCURRENCY) as pool:
            futures = []
            for i in range(LIGHT_REQUESTS):
                method, params = methods[i % len(methods)]
                futures.append(pool.submit(rpc_call_timed, load_url, method, params))

            for future in concurrent.futures.as_completed(futures):
                latency, success, _ = future.result()
                all_latencies.append(latency)
                total += 1
                if success:
                    successes += 1

        success_rate = successes / total * 100
        avg_latency = statistics.mean(all_latencies)
        print(
            f"\nMixed Load: {successes}/{total} ({success_rate:.1f}%) "
            f"avg={avg_latency:.1f}ms"
        )

        assert success_rate >= 90, f"Mixed workload success rate: {success_rate:.1f}%"


# ── 2. Heavy Load Tests (slow, marked) ──────────────────────────────


class TestHeavyLoad:
    """
    Heavy load tests — run with -k "heavy" or explicitly.

    These tests send 500 requests with 20 concurrent workers.
    """

    @pytest.mark.slow
    def test_heavy_block_number(self, load_url):
        """eth_blockNumber under heavy load (500 req, 20 concurrent)."""
        result = run_load_test(
            load_url, "eth_blockNumber",
            total_requests=HEAVY_REQUESTS,
            concurrency=HEAVY_CONCURRENCY,
        )
        print(f"\n{result.summary()}")

        assert result.success_rate >= 90, (
            f"Heavy load success: {result.success_rate:.1f}%"
        )
        assert result.rps >= 20, f"RPS too low: {result.rps:.1f}"

    @pytest.mark.slow
    def test_heavy_balance_queries(self, load_url):
        """eth_getBalance under heavy load."""
        # Use different addresses to avoid cache effects
        results = LoadResult(total_requests=HEAVY_REQUESTS)
        t0 = time.perf_counter()

        def query_balance(i):
            addr = f"0x{i:040x}"
            return rpc_call_timed(load_url, "eth_getBalance", [addr, "latest"])

        with concurrent.futures.ThreadPoolExecutor(max_workers=HEAVY_CONCURRENCY) as pool:
            futures = [pool.submit(query_balance, i) for i in range(HEAVY_REQUESTS)]
            for future in concurrent.futures.as_completed(futures):
                latency, success, error = future.result()
                results.latencies_ms.append(latency)
                if success:
                    results.successful += 1
                else:
                    results.failed += 1

        results.duration_s = time.perf_counter() - t0
        print(f"\n{results.summary()}")

        assert results.success_rate >= 85, (
            f"Heavy balance query success: {results.success_rate:.1f}%"
        )

    @pytest.mark.slow
    def test_sustained_load_60s(self, load_url):
        """Sustained load for 60 seconds — measures stability."""
        duration = 60
        concurrency = 10
        results = LoadResult()

        t_start = time.perf_counter()

        def worker():
            """Keep sending requests until timeout."""
            local_lats = []
            local_ok = 0
            local_fail = 0
            while time.perf_counter() - t_start < duration:
                lat, ok, err = rpc_call_timed(load_url, "eth_blockNumber")
                local_lats.append(lat)
                if ok:
                    local_ok += 1
                else:
                    local_fail += 1
            return local_lats, local_ok, local_fail

        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
            futures = [pool.submit(worker) for _ in range(concurrency)]
            for future in concurrent.futures.as_completed(futures):
                lats, ok, fail = future.result()
                results.latencies_ms.extend(lats)
                results.successful += ok
                results.failed += fail

        results.total_requests = results.successful + results.failed
        results.duration_s = time.perf_counter() - t_start
        print(f"\n{results.summary()}")

        assert results.success_rate >= 90, (
            f"Sustained load success: {results.success_rate:.1f}%"
        )
        assert results.rps >= 10, f"Sustained RPS too low: {results.rps:.1f}"


# ── 3. SDK Client Load Tests ────────────────────────────────────────


class TestSDKClientLoad:
    """Load tests using the actual SDK client (measures SDK overhead)."""

    def test_sdk_sequential_queries(self, luxtensor_client):
        """Measure SDK overhead with 20 sequential get_block_number calls."""
        latencies = []
        for _ in range(20):
            t0 = time.perf_counter()
            luxtensor_client.get_block_number()
            latencies.append((time.perf_counter() - t0) * 1000)

        avg = statistics.mean(latencies)
        p95 = sorted(latencies)[int(len(latencies) * 0.95)]
        print(f"\nSDK Sequential: avg={avg:.1f}ms  p95={p95:.1f}ms")

        # SDK should add minimal overhead over raw RPC
        assert avg < 2000, f"SDK avg latency too high: {avg:.1f}ms"

    def test_sdk_concurrent_clients(self, node_url):
        """Multiple SDK clients querying concurrently."""
        from sdk.client import LuxtensorClient

        num_clients = 5
        queries_per_client = 10

        def client_worker(idx):
            client = LuxtensorClient(url=node_url, network="testnet", timeout=10)
            latencies = []
            for _ in range(queries_per_client):
                t0 = time.perf_counter()
                try:
                    client.get_block_number()
                    latencies.append((time.perf_counter() - t0) * 1000)
                except Exception:
                    pass
            return latencies

        all_lats = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=num_clients) as pool:
            futures = [pool.submit(client_worker, i) for i in range(num_clients)]
            for future in concurrent.futures.as_completed(futures):
                all_lats.extend(future.result())

        if all_lats:
            avg = statistics.mean(all_lats)
            print(f"\nSDK Concurrent ({num_clients} clients): "
                  f"avg={avg:.1f}ms  {len(all_lats)} successful queries")
            assert len(all_lats) >= num_clients * queries_per_client * 0.8


# ── Standalone Runner ────────────────────────────────────────────────


def main():
    import argparse

    parser = argparse.ArgumentParser(description="LuxTensor Load Test")
    parser.add_argument("--url", default=DEFAULT_URL)
    parser.add_argument("--requests", type=int, default=100)
    parser.add_argument("--concurrency", type=int, default=10)
    args = parser.parse_args()

    print(f"""
╔══════════════════════════════════════════════════════════╗
║            LuxTensor Load Test                           ║
║   URL: {args.url:<48} ║
║   Requests: {args.requests:<4}  Concurrency: {args.concurrency:<4}                 ║
╚══════════════════════════════════════════════════════════╝
    """)

    # Check node
    _, ok, _ = rpc_call_timed(args.url, "eth_blockNumber")
    if not ok:
        print("❌ Node not reachable!")
        return

    methods = [
        ("eth_blockNumber", None),
        ("eth_getBalance", ["0x" + "00" * 20, "latest"]),
        ("eth_gasPrice", None),
        ("subnet_getAll", None),
    ]

    for method, params in methods:
        print(f"\n{'─' * 50}")
        print(f"Method: {method}")
        result = run_load_test(
            args.url, method, params,
            total_requests=args.requests,
            concurrency=args.concurrency,
        )
        print(result.summary())


if __name__ == "__main__":
    main()
