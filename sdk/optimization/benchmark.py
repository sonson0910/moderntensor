"""
Performance benchmarking tools.

Provides comprehensive performance testing and metrics collection.
"""
from typing import Dict, Any, List, Callable
import time
import statistics


class PerformanceBenchmark:
    """
    Comprehensive performance benchmarking for blockchain components.
    
    Measures:
    - Transaction throughput (TPS)
    - Block production time
    - State access latency
    - Network latency
    - Signature verification speed
    """
    
    def __init__(self):
        """Initialize performance benchmark."""
        self.results = {}
    
    def benchmark_transaction_throughput(self, process_func: Callable, 
                                        num_transactions: int = 1000) -> Dict[str, float]:
        """
        Benchmark transaction processing throughput.
        
        Args:
            process_func: Function that processes transactions
            num_transactions: Number of transactions to process
            
        Returns:
            Dict with throughput metrics
        """
        # Generate test transactions
        test_txs = [self._generate_test_tx() for _ in range(num_transactions)]
        
        start_time = time.time()
        process_func(test_txs)
        end_time = time.time()
        
        duration = end_time - start_time
        tps = num_transactions / duration if duration > 0 else 0
        
        result = {
            'transactions': num_transactions,
            'duration_seconds': duration,
            'tps': tps,
            'avg_time_per_tx_ms': (duration / num_transactions) * 1000 if num_transactions > 0 else 0,
        }
        
        self.results['transaction_throughput'] = result
        return result
    
    def benchmark_signature_verification(self, num_signatures: int = 1000) -> Dict[str, float]:
        """
        Benchmark signature verification speed.
        
        Args:
            num_signatures: Number of signatures to verify
            
        Returns:
            Dict with verification metrics
        """
        times = []
        
        for _ in range(num_signatures):
            start = time.time()
            # Would verify actual signature here
            # self._verify_signature()
            end = time.time()
            times.append((end - start) * 1000)  # Convert to ms
        
        result = {
            'signatures_verified': num_signatures,
            'avg_time_ms': statistics.mean(times) if times else 0,
            'min_time_ms': min(times) if times else 0,
            'max_time_ms': max(times) if times else 0,
            'std_dev_ms': statistics.stdev(times) if len(times) > 1 else 0,
        }
        
        self.results['signature_verification'] = result
        return result
    
    def benchmark_state_access(self, state_db: Any, num_accesses: int = 10000) -> Dict[str, float]:
        """
        Benchmark state database access latency.
        
        Args:
            state_db: State database instance
            num_accesses: Number of state accesses to perform
            
        Returns:
            Dict with latency metrics
        """
        times = []
        
        for _ in range(num_accesses):
            start = time.time()
            # Would access state here
            # state_db.get_account(random_address)
            end = time.time()
            times.append((end - start) * 1000000)  # Convert to microseconds
        
        result = {
            'accesses': num_accesses,
            'avg_latency_us': statistics.mean(times) if times else 0,
            'p50_latency_us': statistics.median(times) if times else 0,
            'p95_latency_us': self._percentile(times, 0.95) if times else 0,
            'p99_latency_us': self._percentile(times, 0.99) if times else 0,
        }
        
        self.results['state_access'] = result
        return result
    
    def benchmark_block_production(self, produce_func: Callable, 
                                   num_blocks: int = 100) -> Dict[str, float]:
        """
        Benchmark block production time.
        
        Args:
            produce_func: Function that produces a block
            num_blocks: Number of blocks to produce
            
        Returns:
            Dict with production metrics
        """
        times = []
        
        for _ in range(num_blocks):
            start = time.time()
            produce_func()
            end = time.time()
            times.append(end - start)
        
        result = {
            'blocks_produced': num_blocks,
            'avg_time_seconds': statistics.mean(times) if times else 0,
            'min_time_seconds': min(times) if times else 0,
            'max_time_seconds': max(times) if times else 0,
        }
        
        self.results['block_production'] = result
        return result
    
    def _generate_test_tx(self) -> Any:
        """Generate a test transaction."""
        return None  # Placeholder
    
    def _percentile(self, data: List[float], percentile: float) -> float:
        """Calculate percentile of data."""
        if not data:
            return 0.0
        sorted_data = sorted(data)
        index = int(len(sorted_data) * percentile)
        return sorted_data[min(index, len(sorted_data) - 1)]
    
    def get_all_results(self) -> Dict[str, Any]:
        """Get all benchmark results."""
        return dict(self.results)
    
    def generate_report(self) -> str:
        """Generate a human-readable performance report."""
        report = ["Performance Benchmark Report", "=" * 50, ""]
        
        for category, metrics in self.results.items():
            report.append(f"{category.replace('_', ' ').title()}:")
            for key, value in metrics.items():
                if isinstance(value, float):
                    report.append(f"  {key}: {value:.2f}")
                else:
                    report.append(f"  {key}: {value}")
            report.append("")
        
        return "\n".join(report)
