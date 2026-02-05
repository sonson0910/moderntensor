#!/usr/bin/env python3
"""
High-performance TPS benchmark for Luxtensor RPC
Uses concurrent requests to measure maximum throughput
"""

import sys
import time
import json
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import Tuple

try:
    import requests
except ImportError:
    print("Install: pip install requests")
    sys.exit(1)


def make_request(session: requests.Session, url: str, method: str, params: list = None) -> Tuple[bool, float]:
    """Make a single RPC request and return (success, latency)"""
    start = time.perf_counter()
    try:
        payload = {"jsonrpc": "2.0", "method": method, "params": params or [], "id": 1}
        resp = session.post(url, json=payload, timeout=5)
        data = resp.json()
        elapsed = time.perf_counter() - start
        return "error" not in data, elapsed
    except Exception as e:
        return False, time.perf_counter() - start


def benchmark(url: str, method: str, params: list = None, requests_count: int = 1000, concurrency: int = 50):
    """Run benchmark with concurrent requests"""
    print(f"\n{'='*60}")
    print(f"  BENCHMARK: {method}")
    print(f"  Requests: {requests_count}, Concurrency: {concurrency}")
    print(f"{'='*60}")

    # Create session with connection pooling
    session = requests.Session()
    adapter = requests.adapters.HTTPAdapter(
        pool_connections=concurrency,
        pool_maxsize=concurrency,
        max_retries=0
    )
    session.mount('http://', adapter)

    start = time.perf_counter()
    success_count = 0
    latencies = []

    with ThreadPoolExecutor(max_workers=concurrency) as executor:
        futures = [
            executor.submit(make_request, session, url, method, params)
            for _ in range(requests_count)
        ]

        for future in as_completed(futures):
            success, latency = future.result()
            if success:
                success_count += 1
                latencies.append(latency)

    total_time = time.perf_counter() - start
    tps = requests_count / total_time

    # Calculate statistics
    if latencies:
        avg_latency = sum(latencies) / len(latencies) * 1000  # ms
        min_latency = min(latencies) * 1000
        max_latency = max(latencies) * 1000
        p99_latency = sorted(latencies)[int(len(latencies) * 0.99)] * 1000
    else:
        avg_latency = min_latency = max_latency = p99_latency = 0

    print(f"\n  ✅ Success: {success_count}/{requests_count}")
    print(f"  ⏱️  Total time: {total_time:.2f}s")
    print(f"  🚀 TPS: {tps:.2f} requests/second")
    print(f"\n  Latency (ms):")
    print(f"     Avg: {avg_latency:.2f}")
    print(f"     Min: {min_latency:.2f}")
    print(f"     Max: {max_latency:.2f}")
    print(f"     P99: {p99_latency:.2f}")
    print(f"{'='*60}\n")

    return tps


def main():
    url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8545"

    print("\n" + "="*60)
    print("  🔥 LUXTENSOR HIGH-PERFORMANCE BENCHMARK")
    print("="*60)

    # Test connection
    try:
        session = requests.Session()
        resp = session.post(url, json={"jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 1}, timeout=5)
        if resp.status_code != 200:
            print(f"❌ Failed to connect to {url}")
            return
        print(f"  ✅ Connected to {url}")
    except Exception as e:
        print(f"❌ Connection error: {e}")
        return

    # Run benchmarks
    results = {}

    # Warm up
    print("\n  🔥 Warming up...")
    benchmark(url, "eth_blockNumber", requests_count=100, concurrency=10)

    # Read operations
    results["eth_blockNumber"] = benchmark(url, "eth_blockNumber", requests_count=5000, concurrency=200)
    results["eth_getBalance"] = benchmark(url, "eth_getBalance",
        params=["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", "latest"],
        requests_count=5000, concurrency=200)
    results["eth_chainId"] = benchmark(url, "eth_chainId", requests_count=5000, concurrency=200)

    # Summary
    print("\n" + "="*60)
    print("  📊 BENCHMARK SUMMARY")
    print("="*60)
    for method, tps in results.items():
        emoji = "🚀" if tps >= 1000 else "⚡" if tps >= 500 else "💨" if tps >= 100 else "🐢"
        print(f"  {emoji} {method}: {tps:.2f} TPS")

    avg_tps = sum(results.values()) / len(results)
    print(f"\n  📈 Average TPS: {avg_tps:.2f}")
    print("="*60 + "\n")


if __name__ == "__main__":
    main()
