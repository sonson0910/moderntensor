"""
Tests for the Dendrite client implementation.

Tests cover configuration, connection pool, circuit breaker, and query functionality.
"""

import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

import pytest
import asyncio
from datetime import datetime, timedelta

from sdk.dendrite.config import DendriteConfig, LoadBalancingStrategy, RetryStrategy
from sdk.dendrite.pool import ConnectionPool, CircuitBreaker
from sdk.dendrite.aggregator import ResponseAggregator


class TestDendriteConfig:
    """Test Dendrite configuration."""
    
    def test_default_config(self):
        """Test default configuration values."""
        config = DendriteConfig()
        assert config.timeout == 30.0
        assert config.max_retries == 3
        assert config.parallel_queries is True
        assert config.circuit_breaker_enabled is True
        print("✓ Default config test passed")
    
    def test_custom_config(self):
        """Test custom configuration."""
        config = DendriteConfig(
            timeout=60.0,
            max_retries=5,
            cache_enabled=True,
            load_balancing_strategy=LoadBalancingStrategy.RANDOM,
        )
        assert config.timeout == 60.0
        assert config.max_retries == 5
        assert config.cache_enabled is True
        assert config.load_balancing_strategy == LoadBalancingStrategy.RANDOM
        print("✓ Custom config test passed")
    
    def test_validation(self):
        """Test configuration validation."""
        # Test max_retry_delay validation
        try:
            DendriteConfig(retry_delay=10.0, max_retry_delay=5.0)
            assert False, "Should have raised ValueError"
        except ValueError:
            print("✓ Config validation test passed")


class TestConnectionPool:
    """Test ConnectionPool functionality."""
    
    def test_initialization(self):
        """Test connection pool initialization."""
        config = DendriteConfig()
        pool = ConnectionPool(config)
        
        assert pool.config == config
        assert pool.client is not None
        print("✓ Connection pool initialization test passed")
    
    def test_connection_tracking(self):
        """Test connection tracking."""
        config = DendriteConfig()
        pool = ConnectionPool(config)
        
        # Track connections
        pool.track_connection("host1")
        pool.track_connection("host1")
        assert pool.get_connection_count("host1") == 2
        
        # Release connection
        pool.release_connection("host1")
        assert pool.get_connection_count("host1") == 1
        
        print("✓ Connection tracking test passed")
    
    def test_error_tracking(self):
        """Test error tracking."""
        config = DendriteConfig()
        pool = ConnectionPool(config)
        
        # Record errors
        pool.record_error("host1")
        pool.record_error("host1")
        assert pool.get_error_count("host1") == 2
        
        # Reset errors
        pool.reset_errors("host1")
        assert pool.get_error_count("host1") == 0
        
        print("✓ Error tracking test passed")
    
    def test_availability_check(self):
        """Test connection availability check."""
        config = DendriteConfig(max_connections_per_host=2)
        pool = ConnectionPool(config)
        
        # Initially available
        assert pool.is_available("host1")
        
        # Track connections
        pool.track_connection("host1")
        assert pool.is_available("host1")
        
        pool.track_connection("host1")
        assert not pool.is_available("host1")  # At limit
        
        print("✓ Availability check test passed")


class TestCircuitBreaker:
    """Test CircuitBreaker functionality."""
    
    def test_initialization(self):
        """Test circuit breaker initialization."""
        breaker = CircuitBreaker(threshold=5, timeout=60.0)
        
        assert breaker.threshold == 5
        assert breaker.timeout == 60.0
        print("✓ Circuit breaker initialization test passed")
    
    def test_closed_state(self):
        """Test circuit breaker in closed state."""
        breaker = CircuitBreaker(threshold=3)
        
        # Initially closed
        assert breaker.is_closed("host1")
        assert breaker.can_attempt("host1")
        
        # Record failures below threshold
        breaker.record_failure("host1")
        breaker.record_failure("host1")
        assert breaker.is_closed("host1")
        
        print("✓ Closed state test passed")
    
    def test_open_state(self):
        """Test circuit breaker opening."""
        breaker = CircuitBreaker(threshold=3, timeout=1.0)
        
        # Record failures to open circuit
        for _ in range(3):
            breaker.record_failure("host1")
        
        assert breaker.is_open("host1")
        assert not breaker.can_attempt("host1")
        
        print("✓ Open state test passed")
    
    def test_half_open_state(self):
        """Test circuit breaker half-open state."""
        import time
        
        breaker = CircuitBreaker(threshold=2, timeout=0.1, half_open_max_calls=2)
        
        # Open the circuit
        breaker.record_failure("host1")
        breaker.record_failure("host1")
        assert breaker.is_open("host1")
        
        # Wait for timeout
        time.sleep(0.2)
        
        # Should transition to half-open
        assert breaker.can_attempt("host1")
        
        print("✓ Half-open state test passed")
    
    def test_recovery(self):
        """Test circuit breaker recovery."""
        breaker = CircuitBreaker(threshold=2)
        
        # Open the circuit
        breaker.record_failure("host1")
        breaker.record_failure("host1")
        
        # Transition to half-open (simulate)
        breaker._transition_to_half_open("host1")
        
        # Record success to close
        breaker.record_success("host1")
        assert breaker.is_closed("host1")
        
        print("✓ Recovery test passed")


class TestResponseAggregator:
    """Test ResponseAggregator functionality."""
    
    def test_majority_vote(self):
        """Test majority voting aggregation."""
        responses = [
            {"result": "A"},
            {"result": "A"},
            {"result": "B"},
        ]
        
        result = ResponseAggregator.majority_vote(responses)
        assert result == "A"
        print("✓ Majority vote test passed")
    
    def test_average(self):
        """Test average aggregation."""
        responses = [
            {"result": 10},
            {"result": 20},
            {"result": 30},
        ]
        
        result = ResponseAggregator.average(responses)
        assert result == 20.0
        print("✓ Average test passed")
    
    def test_median(self):
        """Test median aggregation."""
        responses = [
            {"result": 10},
            {"result": 20},
            {"result": 30},
            {"result": 40},
        ]
        
        result = ResponseAggregator.median(responses)
        assert result == 25.0
        print("✓ Median test passed")
    
    def test_weighted_average(self):
        """Test weighted average aggregation."""
        responses = [
            {"result": 10},
            {"result": 20},
            {"result": 30},
        ]
        weights = [0.5, 0.3, 0.2]
        
        result = ResponseAggregator.weighted_average(responses, weights)
        expected = (10 * 0.5 + 20 * 0.3 + 30 * 0.2)
        assert abs(result - expected) < 0.01
        print("✓ Weighted average test passed")
    
    def test_first_valid(self):
        """Test first valid response."""
        responses = [
            {"result": None},
            {"result": "valid"},
            {"result": "also valid"},
        ]
        
        result = ResponseAggregator.first_valid(responses)
        assert result == "valid"
        print("✓ First valid test passed")
    
    def test_consensus(self):
        """Test consensus aggregation."""
        responses = [
            {"result": "A"},
            {"result": "A"},
            {"result": "A"},
            {"result": "B"},
        ]
        
        # 75% agreement, threshold 0.66
        result = ResponseAggregator.consensus(responses, threshold=0.66)
        assert result == "A"
        
        # No consensus with higher threshold
        result = ResponseAggregator.consensus(responses, threshold=0.9)
        assert result is None
        
        print("✓ Consensus test passed")
    
    def test_aggregate_method(self):
        """Test aggregate method with different strategies."""
        responses = [
            {"result": "A"},
            {"result": "A"},
            {"result": "B"},
        ]
        
        # Test majority
        result = ResponseAggregator.aggregate(responses, strategy="majority")
        assert result == "A"
        
        # Test first
        result = ResponseAggregator.aggregate(responses, strategy="first")
        assert result == "A"
        
        print("✓ Aggregate method test passed")


def run_tests():
    """Run all tests."""
    print("\n" + "="*60)
    print("Running Dendrite Tests")
    print("="*60 + "\n")
    
    # Test DendriteConfig
    print("Testing DendriteConfig...")
    config_tests = TestDendriteConfig()
    config_tests.test_default_config()
    config_tests.test_custom_config()
    config_tests.test_validation()
    
    # Test ConnectionPool
    print("\nTesting ConnectionPool...")
    pool_tests = TestConnectionPool()
    pool_tests.test_initialization()
    pool_tests.test_connection_tracking()
    pool_tests.test_error_tracking()
    pool_tests.test_availability_check()
    
    # Test CircuitBreaker
    print("\nTesting CircuitBreaker...")
    breaker_tests = TestCircuitBreaker()
    breaker_tests.test_initialization()
    breaker_tests.test_closed_state()
    breaker_tests.test_open_state()
    breaker_tests.test_half_open_state()
    breaker_tests.test_recovery()
    
    # Test ResponseAggregator
    print("\nTesting ResponseAggregator...")
    aggregator_tests = TestResponseAggregator()
    aggregator_tests.test_majority_vote()
    aggregator_tests.test_average()
    aggregator_tests.test_median()
    aggregator_tests.test_weighted_average()
    aggregator_tests.test_first_valid()
    aggregator_tests.test_consensus()
    aggregator_tests.test_aggregate_method()
    
    print("\n" + "="*60)
    print("✅ All tests passed!")
    print("="*60 + "\n")


if __name__ == "__main__":
    run_tests()
