"""
Tests for Month 2 Implementation: Performance Optimization and Security

This test suite covers:
- Performance optimization (Week 1-2)
- Security hardening (Week 3-4)
"""

import pytest
import asyncio
import time
from typing import Dict, Any

# Performance optimization tests
from sdk.tokenomics.performance_optimizer import (
    TTLCache,
    CacheConfig,
    PerformanceOptimizer,
    BatchOperationOptimizer,
    calculate_stake_weight,
    calculate_performance_score,
    MemoryOptimizer
)

# Security tests
from sdk.tokenomics.security import (
    RateLimiter,
    RateLimitConfig,
    InputValidator,
    TransactionValidator,
    SecurityMonitor,
    SecurityLevel,
    SlashingValidator,
    AuditLogger
)


# ============================================================================
# Performance Optimization Tests
# ============================================================================

class TestTTLCache:
    """Test TTL cache functionality."""
    
    @pytest.mark.asyncio
    async def test_cache_set_and_get(self):
        """Test basic cache operations."""
        cache = TTLCache(CacheConfig())
        
        await cache.set("key1", "value1")
        result = await cache.get("key1")
        
        assert result == "value1"
    
    @pytest.mark.asyncio
    async def test_cache_expiry(self):
        """Test cache expiration."""
        cache = TTLCache(CacheConfig(ttl_seconds=1))
        
        await cache.set("key1", "value1")
        await asyncio.sleep(1.5)
        result = await cache.get("key1")
        
        assert result is None
    
    @pytest.mark.asyncio
    async def test_cache_eviction(self):
        """Test LRU eviction."""
        cache = TTLCache(CacheConfig(max_size=2))
        
        await cache.set("key1", "value1")
        await cache.set("key2", "value2")
        await cache.set("key3", "value3")  # Should evict key1
        
        assert await cache.get("key1") is None
        assert await cache.get("key2") == "value2"
        assert await cache.get("key3") == "value3"
    
    @pytest.mark.asyncio
    async def test_cache_metrics(self):
        """Test cache metrics tracking."""
        cache = TTLCache(CacheConfig())
        
        await cache.set("key1", "value1")
        await cache.get("key1")  # Hit
        await cache.get("key2")  # Miss
        
        metrics = cache.get_metrics()
        assert metrics.hits == 1
        assert metrics.misses == 1
        assert metrics.hit_rate == 50.0


class TestPerformanceOptimizer:
    """Test performance optimizer."""
    
    @pytest.mark.asyncio
    async def test_utility_cache(self):
        """Test utility score caching."""
        optimizer = PerformanceOptimizer()
        
        # Cache a score
        await optimizer.cache_utility_score(
            task_volume=100,
            avg_task_difficulty=0.8,
            validator_participation=0.9,
            score=0.85
        )
        
        # Retrieve cached score
        cached = await optimizer.get_cached_utility_score(
            task_volume=100,
            avg_task_difficulty=0.8,
            validator_participation=0.9
        )
        
        assert cached == 0.85
    
    @pytest.mark.asyncio
    async def test_distribution_cache(self):
        """Test distribution caching."""
        optimizer = PerformanceOptimizer()
        
        distribution = {
            'miner_rewards': {'0x123': 100},
            'validator_rewards': {'0x456': 200}
        }
        
        await optimizer.cache_distribution(
            epoch=1,
            total_emission=300,
            distribution=distribution
        )
        
        cached = await optimizer.get_cached_distribution(
            epoch=1,
            total_emission=300
        )
        
        assert cached == distribution
    
    @pytest.mark.asyncio
    async def test_operation_profiling(self):
        """Test operation profiling."""
        optimizer = PerformanceOptimizer()
        
        @optimizer.profile_operation('test_op')
        async def test_operation():
            await asyncio.sleep(0.1)
            return "result"
        
        result = await test_operation()
        stats = optimizer.get_performance_stats()
        
        assert result == "result"
        assert 'test_op' in stats['operation_timings']
        assert stats['operation_timings']['test_op']['count'] == 1


class TestBatchOperationOptimizer:
    """Test batch operations."""
    
    @pytest.mark.asyncio
    async def test_batch_processing(self):
        """Test parallel batch processing."""
        optimizer = BatchOperationOptimizer(max_batch_size=5)
        
        items = list(range(10))
        
        async def double(x):
            await asyncio.sleep(0.01)
            return x * 2
        
        results = await optimizer.batch_process(
            items,
            double,
            max_concurrent=3
        )
        
        assert len(results) == 10
        assert results[0] == 0
        assert results[5] == 10
    
    def test_chunk_rewards(self):
        """Test reward chunking."""
        optimizer = BatchOperationOptimizer()
        
        rewards = {f'0x{i:040x}': i * 100 for i in range(10)}
        chunks = optimizer.chunk_rewards(rewards, chunk_size=3)
        
        assert len(chunks) == 4  # 10 items / 3 = 4 chunks
        assert len(chunks[0]) == 3
        assert len(chunks[-1]) == 1


class TestCachedFunctions:
    """Test cached utility functions."""
    
    def test_stake_weight_calculation(self):
        """Test cached stake weight calculation."""
        weight1 = calculate_stake_weight(100, 1000)
        weight2 = calculate_stake_weight(100, 1000)  # Should hit cache
        
        assert weight1 == 0.1
        assert weight2 == 0.1
    
    def test_performance_score_calculation(self):
        """Test cached performance score."""
        score1 = calculate_performance_score(0.9, 100, 0.95)
        score2 = calculate_performance_score(0.9, 100, 0.95)  # Cache hit
        
        assert 0.0 <= score1 <= 1.0
        assert score1 == score2


class TestMemoryOptimizer:
    """Test memory optimization."""
    
    def test_reward_compression(self):
        """Test reward data compression."""
        rewards = {f'0x{i:040x}': i * 1000 for i in range(100)}
        
        compressed = MemoryOptimizer.compress_reward_data(rewards)
        decompressed = MemoryOptimizer.decompress_reward_data(compressed)
        
        assert decompressed == rewards
        # Compression should reduce size
        import pickle
        original_size = len(pickle.dumps(rewards))
        assert len(compressed) < original_size


# ============================================================================
# Security Tests
# ============================================================================

class TestRateLimiter:
    """Test rate limiting."""
    
    def test_rate_limit_allows_requests(self):
        """Test that requests within limit are allowed."""
        limiter = RateLimiter(RateLimitConfig(max_requests=5, window_seconds=60))
        
        address = "0x1234567890123456789012345678901234567890"
        
        for _ in range(5):
            assert limiter.check_rate_limit(address) is True
    
    def test_rate_limit_blocks_excess(self):
        """Test that excess requests are blocked."""
        limiter = RateLimiter(RateLimitConfig(max_requests=3, window_seconds=60))
        
        address = "0x1234567890123456789012345678901234567890"
        
        # First 3 should pass
        for _ in range(3):
            assert limiter.check_rate_limit(address) is True
        
        # 4th should fail
        assert limiter.check_rate_limit(address) is False
    
    def test_rate_limit_window_reset(self):
        """Test that rate limit resets after window."""
        limiter = RateLimiter(RateLimitConfig(max_requests=2, window_seconds=1))
        
        address = "0x1234567890123456789012345678901234567890"
        
        # Use up limit
        limiter.check_rate_limit(address)
        limiter.check_rate_limit(address)
        assert limiter.check_rate_limit(address) is False
        
        # Wait for window to reset
        time.sleep(1.5)
        
        # Should work again
        assert limiter.check_rate_limit(address) is True


class TestInputValidator:
    """Test input validation."""
    
    def test_valid_address(self):
        """Test valid address validation."""
        address = "0x1234567890123456789012345678901234567890"
        assert InputValidator.validate_address(address) is True
    
    def test_invalid_address_format(self):
        """Test invalid address format."""
        with pytest.raises(ValueError, match="Invalid address format"):
            InputValidator.validate_address("invalid")
    
    def test_invalid_address_type(self):
        """Test invalid address type."""
        with pytest.raises(ValueError, match="must be a string"):
            InputValidator.validate_address(123)
    
    def test_valid_amount(self):
        """Test valid amount validation."""
        assert InputValidator.validate_amount(1000) is True
        assert InputValidator.validate_amount(0) is True
    
    def test_negative_amount(self):
        """Test negative amount rejection."""
        with pytest.raises(ValueError, match="must be >="):
            InputValidator.validate_amount(-100, min_val=0)
    
    def test_non_integer_amount(self):
        """Test non-integer amount rejection."""
        with pytest.raises(ValueError, match="must be an integer"):
            InputValidator.validate_amount(10.5)
    
    def test_valid_score(self):
        """Test valid score validation."""
        assert InputValidator.validate_score(0.5) is True
        assert InputValidator.validate_score(0.0) is True
        assert InputValidator.validate_score(1.0) is True
    
    def test_out_of_range_score(self):
        """Test out of range score."""
        with pytest.raises(ValueError, match="must be between"):
            InputValidator.validate_score(1.5)
    
    def test_string_sanitization(self):
        """Test string sanitization."""
        dangerous = "<script>alert('xss')</script>"
        sanitized = InputValidator.sanitize_string(dangerous)
        
        assert '<' not in sanitized
        assert '>' not in sanitized
        assert 'script' in sanitized  # Letters remain


class TestTransactionValidator:
    """Test transaction validation."""
    
    def test_valid_reward_transaction(self):
        """Test valid reward transaction."""
        validator = TransactionValidator()
        
        result = validator.validate_reward_transaction(
            from_address="0x1234567890123456789012345678901234567890",
            to_address="0x0987654321098765432109876543210987654321",
            amount=1000,
            balance=5000
        )
        
        assert result is True
    
    def test_insufficient_balance(self):
        """Test insufficient balance rejection."""
        validator = TransactionValidator()
        
        with pytest.raises(ValueError, match="Insufficient balance"):
            validator.validate_reward_transaction(
                from_address="0x1234567890123456789012345678901234567890",
                to_address="0x0987654321098765432109876543210987654321",
                amount=1000,
                balance=500
            )
    
    def test_self_transfer_prevention(self):
        """Test self-transfer prevention."""
        validator = TransactionValidator()
        
        address = "0x1234567890123456789012345678901234567890"
        
        with pytest.raises(ValueError, match="Cannot transfer to self"):
            validator.validate_reward_transaction(
                from_address=address,
                to_address=address,
                amount=1000,
                balance=5000
            )
    
    def test_double_claim_detection(self):
        """Test double-claim detection."""
        validator = TransactionValidator()
        
        tx_hash = "0xabcdef"
        
        assert validator.check_double_claim(tx_hash) is False
        
        validator.mark_processed(tx_hash)
        
        assert validator.check_double_claim(tx_hash) is True


class TestSecurityMonitor:
    """Test security monitoring."""
    
    def test_reward_anomaly_detection(self):
        """Test anomalous reward detection."""
        monitor = SecurityMonitor()
        
        alert = monitor.check_reward_anomaly(
            address="0x1234567890123456789012345678901234567890",
            reward=10000,
            avg_reward=1000,
            threshold=3.0
        )
        
        assert alert is not None
        assert alert.level == SecurityLevel.WARNING
    
    def test_normal_reward_no_alert(self):
        """Test normal reward doesn't trigger alert."""
        monitor = SecurityMonitor()
        
        alert = monitor.check_reward_anomaly(
            address="0x1234567890123456789012345678901234567890",
            reward=1200,
            avg_reward=1000,
            threshold=3.0
        )
        
        assert alert is None
    
    def test_suspicious_claim_pattern(self):
        """Test suspicious claim pattern detection."""
        monitor = SecurityMonitor()
        
        alert = monitor.check_claim_pattern(
            address="0x1234567890123456789012345678901234567890",
            claim_count=15,
            time_window=1800  # 30 minutes
        )
        
        assert alert is not None
        assert alert.level == SecurityLevel.WARNING


class TestSlashingValidator:
    """Test slashing validation."""
    
    def test_slash_amount_calculation(self):
        """Test slash amount calculation."""
        validator = SlashingValidator(slash_percentage=0.1)
        
        slash = validator.calculate_slash_amount(
            stake=10000,
            severity=1.0
        )
        
        assert slash == 1000  # 10% of 10000
    
    def test_slash_amount_with_severity(self):
        """Test slash amount with severity multiplier."""
        validator = SlashingValidator(slash_percentage=0.1)
        
        slash = validator.calculate_slash_amount(
            stake=10000,
            severity=0.5
        )
        
        assert slash == 500  # 10% * 0.5 * 10000
    
    def test_slash_cannot_exceed_stake(self):
        """Test that slash cannot exceed stake."""
        validator = SlashingValidator(slash_percentage=0.5)
        
        slash = validator.calculate_slash_amount(
            stake=1000,
            severity=3.0  # Would be 150% without cap
        )
        
        assert slash == 1000  # Capped at stake
    
    def test_slash_evidence_validation(self):
        """Test slash evidence validation."""
        validator = SlashingValidator()
        
        evidence = {
            'type': 'double_sign',
            'timestamp': time.time(),
            'proof': b'proof_data'
        }
        
        assert validator.validate_slash_evidence(
            validator="0x1234567890123456789012345678901234567890",
            evidence=evidence
        ) is True
    
    def test_invalid_evidence_type(self):
        """Test invalid evidence type."""
        validator = SlashingValidator()
        
        evidence = {
            'type': 'invalid_type',
            'timestamp': time.time(),
            'proof': b'proof_data'
        }
        
        with pytest.raises(ValueError, match="Invalid evidence type"):
            validator.validate_slash_evidence(
                validator="0x1234567890123456789012345678901234567890",
                evidence=evidence
            )


class TestAuditLogger:
    """Test audit logging."""
    
    def test_log_event(self):
        """Test event logging."""
        logger = AuditLogger()
        
        logger.log_event(
            event_type='reward_claim',
            actor='0x1234567890123456789012345678901234567890',
            action='claimed reward',
            details={'amount': 1000}
        )
        
        assert len(logger.audit_log) == 1
        assert logger.audit_log[0]['type'] == 'reward_claim'
    
    def test_event_filtering(self):
        """Test event filtering."""
        logger = AuditLogger()
        
        logger.log_event('type_a', '0x123', 'action1')
        logger.log_event('type_b', '0x456', 'action2')
        logger.log_event('type_a', '0x789', 'action3')
        
        events = logger.get_events(event_type='type_a')
        
        assert len(events) == 2
        assert all(e['type'] == 'type_a' for e in events)
    
    def test_integrity_verification(self):
        """Test audit log integrity."""
        logger = AuditLogger()
        
        logger.log_event('test', '0x123', 'action1')
        logger.log_event('test', '0x456', 'action2')
        
        assert logger.verify_integrity() is True


# ============================================================================
# Integration Tests
# ============================================================================

class TestIntegration:
    """Test integration of optimization and security."""
    
    @pytest.mark.asyncio
    async def test_optimized_secure_transaction(self):
        """Test optimized transaction with security checks."""
        # Setup
        optimizer = PerformanceOptimizer()
        validator = TransactionValidator()
        monitor = SecurityMonitor()
        
        # Simulate transaction
        from_addr = "0x1234567890123456789012345678901234567890"
        to_addr = "0x0987654321098765432109876543210987654321"
        amount = 1000
        balance = 5000
        
        # Validate
        assert validator.validate_reward_transaction(
            from_addr, to_addr, amount, balance
        )
        
        # Check for anomalies
        alert = monitor.check_reward_anomaly(
            to_addr, amount, avg_reward=900, threshold=3.0
        )
        
        assert alert is None  # Normal transaction
    
    @pytest.mark.asyncio
    async def test_cached_validation(self):
        """Test that validation can use cached data."""
        optimizer = PerformanceOptimizer()
        
        # Cache utility score
        await optimizer.cache_utility_score(
            task_volume=1000,
            avg_task_difficulty=0.8,
            validator_participation=0.9,
            score=0.85
        )
        
        # Retrieve for validation
        score = await optimizer.get_cached_utility_score(
            task_volume=1000,
            avg_task_difficulty=0.8,
            validator_participation=0.9
        )
        
        # Validate score
        InputValidator.validate_score(score)
        
        assert score == 0.85


# ============================================================================
# Stress Tests
# ============================================================================

@pytest.mark.slow
class TestStress:
    """Stress tests for performance and security."""
    
    @pytest.mark.asyncio
    async def test_high_cache_load(self):
        """Test cache under high load."""
        optimizer = PerformanceOptimizer()
        
        # Cache many items
        for i in range(1000):
            await optimizer.cache_utility_score(
                task_volume=i,
                avg_task_difficulty=0.8,
                validator_participation=0.9,
                score=0.85
            )
        
        # Verify cache works
        score = await optimizer.get_cached_utility_score(
            task_volume=500,
            avg_task_difficulty=0.8,
            validator_participation=0.9
        )
        
        assert score == 0.85
    
    def test_many_rate_limit_requests(self):
        """Test rate limiter under load."""
        limiter = RateLimiter(RateLimitConfig(max_requests=100, window_seconds=60))
        
        address = "0x1234567890123456789012345678901234567890"
        
        # Make many requests
        allowed_count = 0
        for _ in range(150):
            if limiter.check_rate_limit(address):
                allowed_count += 1
        
        assert allowed_count == 100  # Only 100 should be allowed
    
    def test_validation_performance(self):
        """Test validation performance."""
        validator = TransactionValidator()
        
        start = time.perf_counter()
        
        # Validate many transactions
        for i in range(1000):
            validator.validate_reward_transaction(
                from_address="0x1234567890123456789012345678901234567890",
                to_address=f"0x{i:040x}",
                amount=100,
                balance=10000
            )
        
        elapsed = time.perf_counter() - start
        
        # Should complete in reasonable time (< 1 second)
        assert elapsed < 1.0


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
