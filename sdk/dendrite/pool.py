"""
Connection pool management for Dendrite.

Manages HTTP connections with pooling, keep-alive, and limits.
"""

import httpx
import asyncio
from typing import Optional, Dict
from datetime import datetime, timedelta
from collections import defaultdict
import logging

from .config import DendriteConfig

logger = logging.getLogger(__name__)


class ConnectionPool:
    """Manages HTTP connection pooling for Dendrite."""

    def __init__(self, config: DendriteConfig):
        """
        Initialize connection pool.

        Args:
            config: Dendrite configuration
        """
        self.config = config

        # Create httpx client with connection pooling
        limits = httpx.Limits(
            max_connections=config.max_connections,
            max_keepalive_connections=config.max_connections_per_host,
            keepalive_expiry=config.keepalive_expiry,
        )

        timeout = httpx.Timeout(
            timeout=config.timeout,
            connect=config.connect_timeout,
            read=config.read_timeout,
        )

        self.client = httpx.AsyncClient(
            limits=limits,
            timeout=timeout,
            headers=config.default_headers,
            follow_redirects=False,
        )

        # Connection tracking
        self.active_connections: Dict[str, int] = {}
        self.connection_errors: Dict[str, int] = {}
        self.last_used: Dict[str, datetime] = {}

        logger.info(
            f"Connection pool initialized: "
            f"max_connections={config.max_connections}, "
            f"max_per_host={config.max_connections_per_host}"
        )

    async def get_client(self) -> httpx.AsyncClient:
        """Get the HTTP client."""
        return self.client

    def track_connection(self, host: str):
        """Track an active connection to a host."""
        self.active_connections[host] = self.active_connections.get(host, 0) + 1
        self.last_used[host] = datetime.now()

    def release_connection(self, host: str):
        """Release a connection from a host."""
        if host in self.active_connections:
            self.active_connections[host] = max(0, self.active_connections[host] - 1)

    def record_error(self, host: str):
        """Record a connection error for a host."""
        self.connection_errors[host] = self.connection_errors.get(host, 0) + 1
        logger.warning("Connection error for %s, total errors: %s", host, self.connection_errors[host])

    def reset_errors(self, host: str):
        """Reset error count for a host."""
        if host in self.connection_errors:
            self.connection_errors[host] = 0

    def get_connection_count(self, host: str) -> int:
        """Get active connection count for a host."""
        return self.active_connections.get(host, 0)

    def get_error_count(self, host: str) -> int:
        """Get error count for a host."""
        return self.connection_errors.get(host, 0)

    def is_available(self, host: str) -> bool:
        """Check if connections are available for a host."""
        return self.get_connection_count(host) < self.config.max_connections_per_host

    async def cleanup_idle_connections(self, idle_timeout: float = 300.0):
        """
        Clean up idle connections.

        Args:
            idle_timeout: Timeout in seconds for idle connections
        """
        now = datetime.now()
        cutoff = now - timedelta(seconds=idle_timeout)

        hosts_to_remove = []
        for host, last_time in self.last_used.items():
            if last_time < cutoff and self.active_connections.get(host, 0) == 0:
                hosts_to_remove.append(host)

        for host in hosts_to_remove:
            if host in self.active_connections:
                del self.active_connections[host]
            if host in self.last_used:
                del self.last_used[host]
            if host in self.connection_errors:
                del self.connection_errors[host]

        if hosts_to_remove:
            logger.info("Cleaned up idle connections for %s hosts", len(hosts_to_remove))

    async def close(self):
        """Close the connection pool and all connections."""
        await self.client.aclose()
        logger.info("Connection pool closed")


class CircuitBreaker:
    """
    Circuit breaker pattern implementation.

    Prevents cascading failures by stopping requests to failing services.
    """

    class State:
        CLOSED = "closed"      # Normal operation
        OPEN = "open"          # Failing, rejecting requests
        HALF_OPEN = "half_open"  # Testing if service recovered

    def __init__(
        self,
        threshold: int = 5,
        timeout: float = 60.0,
        half_open_max_calls: int = 3,
    ):
        """
        Initialize circuit breaker.

        Args:
            threshold: Number of failures before opening circuit
            timeout: Time in seconds before attempting to close circuit
            half_open_max_calls: Max calls allowed in half-open state
        """
        self.threshold = threshold
        self.timeout = timeout
        self.half_open_max_calls = half_open_max_calls

        # State tracking per host
        self.state: Dict[str, str] = {}
        self.failure_count: Dict[str, int] = {}
        self.last_failure_time: Dict[str, datetime] = {}
        self.half_open_calls: Dict[str, int] = {}

        logger.info(
            f"Circuit breaker initialized: threshold={threshold}, "
            f"timeout={timeout}s"
        )

    def is_open(self, host: str) -> bool:
        """Check if circuit is open for a host."""
        state = self._get_state(host)
        return state == self.State.OPEN

    def is_closed(self, host: str) -> bool:
        """Check if circuit is closed for a host."""
        state = self._get_state(host)
        return state == self.State.CLOSED

    def is_half_open(self, host: str) -> bool:
        """Check if circuit is half-open for a host."""
        state = self._get_state(host)
        return state == self.State.HALF_OPEN

    def can_attempt(self, host: str) -> bool:
        """Check if a request can be attempted."""
        state = self._get_state(host)

        if state == self.State.CLOSED:
            return True

        if state == self.State.OPEN:
            # Check if timeout has passed
            if self._should_attempt_reset(host):
                self._transition_to_half_open(host)
                return True
            return False

        if state == self.State.HALF_OPEN:
            # Allow limited calls in half-open state
            calls = self.half_open_calls.get(host, 0)
            return calls < self.half_open_max_calls

        return False

    def record_success(self, host: str):
        """Record a successful request."""
        state = self._get_state(host)

        if state == self.State.HALF_OPEN:
            # Successful call in half-open, close the circuit
            self._transition_to_closed(host)
            logger.info("Circuit closed for %s after successful recovery", host)

        # Reset failure count on success
        self.failure_count[host] = 0

    def record_failure(self, host: str):
        """Record a failed request."""
        state = self._get_state(host)

        self.failure_count[host] = self.failure_count.get(host, 0) + 1
        self.last_failure_time[host] = datetime.now()

        if state == self.State.HALF_OPEN:
            # Failed in half-open, open circuit again
            self._transition_to_open(host)
            logger.warning("Circuit opened again for %s after half-open failure", host)

        elif state == self.State.CLOSED:
            # Check if threshold exceeded
            if self.failure_count[host] >= self.threshold:
                self._transition_to_open(host)
                logger.warning(
                    f"Circuit opened for {host} after {self.failure_count[host]} failures"
                )

    def _get_state(self, host: str) -> str:
        """Get current state for a host."""
        return self.state.get(host, self.State.CLOSED)

    def _should_attempt_reset(self, host: str) -> bool:
        """Check if enough time has passed to attempt reset."""
        if host not in self.last_failure_time:
            return True

        time_since_failure = (datetime.now() - self.last_failure_time[host]).total_seconds()
        return time_since_failure >= self.timeout

    def _transition_to_closed(self, host: str):
        """Transition to closed state."""
        self.state[host] = self.State.CLOSED
        self.failure_count[host] = 0
        self.half_open_calls[host] = 0

    def _transition_to_open(self, host: str):
        """Transition to open state."""
        self.state[host] = self.State.OPEN
        self.half_open_calls[host] = 0

    def _transition_to_half_open(self, host: str):
        """Transition to half-open state."""
        self.state[host] = self.State.HALF_OPEN
        self.half_open_calls[host] = 0

    def get_stats(self, host: str) -> dict:
        """Get circuit breaker statistics for a host."""
        return {
            "state": self._get_state(host),
            "failure_count": self.failure_count.get(host, 0),
            "last_failure": self.last_failure_time.get(host),
            "half_open_calls": self.half_open_calls.get(host, 0),
        }


# =============================================================================
# Advanced Optimization Features (Phase 2 Enhancements)
# =============================================================================

class ConnectionLoadBalancer:
    """
    Load balancer for distributing requests across multiple endpoints.

    Supports:
    - Round-robin distribution
    - Least connections strategy
    - Weighted distribution
    - Health-based routing
    """

    def __init__(self, endpoints: list):
        """
        Initialize load balancer.

        Args:
            endpoints: List of endpoint URLs
        """
        self.endpoints = endpoints
        self.current_index = 0
        self.connection_counts = {ep: 0 for ep in endpoints}
        self.health_status = {ep: True for ep in endpoints}
        self.weights = {ep: 1.0 for ep in endpoints}
        self.lock = asyncio.Lock()

    async def get_next_endpoint(self, strategy: str = "round_robin") -> Optional[str]:
        """
        Get next endpoint based on strategy.

        Args:
            strategy: Load balancing strategy
                - round_robin: Simple round-robin
                - least_connections: Endpoint with fewest connections
                - weighted: Weighted random selection
                - health_aware: Only healthy endpoints with round-robin

        Returns:
            Selected endpoint URL or None if all unhealthy
        """
        async with self.lock:
            healthy_endpoints = [
                ep for ep in self.endpoints if self.health_status.get(ep, True)
            ]

            if not healthy_endpoints:
                logger.error("No healthy endpoints available")
                return None

            if strategy == "round_robin":
                endpoint = healthy_endpoints[self.current_index % len(healthy_endpoints)]
                self.current_index += 1
                return endpoint

            elif strategy == "least_connections":
                endpoint = min(
                    healthy_endpoints,
                    key=lambda ep: self.connection_counts.get(ep, 0)
                )
                return endpoint

            elif strategy == "weighted":
                import random
                total_weight = sum(self.weights.get(ep, 1.0) for ep in healthy_endpoints)
                rand_val = random.uniform(0, total_weight)

                cumulative = 0
                for ep in healthy_endpoints:
                    cumulative += self.weights.get(ep, 1.0)
                    if rand_val <= cumulative:
                        return ep

                return healthy_endpoints[0]

            else:  # Default to round-robin
                return healthy_endpoints[0]

    async def mark_unhealthy(self, endpoint: str):
        """Mark an endpoint as unhealthy."""
        async with self.lock:
            self.health_status[endpoint] = False
            logger.warning("Marked endpoint %s as unhealthy", endpoint)

    async def mark_healthy(self, endpoint: str):
        """Mark an endpoint as healthy."""
        async with self.lock:
            self.health_status[endpoint] = True
            logger.info("Marked endpoint %s as healthy", endpoint)

    async def increment_connections(self, endpoint: str):
        """Increment connection count for endpoint."""
        async with self.lock:
            self.connection_counts[endpoint] += 1

    async def decrement_connections(self, endpoint: str):
        """Decrement connection count for endpoint."""
        async with self.lock:
            if self.connection_counts[endpoint] > 0:
                self.connection_counts[endpoint] -= 1

    def set_weight(self, endpoint: str, weight: float):
        """Set weight for an endpoint."""
        self.weights[endpoint] = weight

    def get_stats(self) -> dict:
        """Get load balancer statistics."""
        return {
            "endpoints": len(self.endpoints),
            "healthy": sum(1 for h in self.health_status.values() if h),
            "connections": dict(self.connection_counts),
            "health": dict(self.health_status),
        }


class ResponseCache:
    """
    Response caching for Dendrite with TTL and size limits.

    Features:
    - TTL-based expiration
    - LRU eviction
    - Size-based limits
    - Cache statistics
    """

    def __init__(self, max_size: int = 1000, default_ttl: int = 60):
        """
        Initialize response cache.

        Args:
            max_size: Maximum number of cached entries
            default_ttl: Default TTL in seconds
        """
        self.max_size = max_size
        self.default_ttl = default_ttl

        # Cache storage: key -> (value, expiry_time, access_count)
        self.cache: Dict[str, tuple] = {}
        self.access_times: Dict[str, datetime] = {}
        self.lock = asyncio.Lock()

        # Stats
        self.hits = 0
        self.misses = 0

    async def get(self, key: str) -> Optional[any]:
        """
        Get value from cache.

        Args:
            key: Cache key

        Returns:
            Cached value or None if not found/expired
        """
        async with self.lock:
            if key not in self.cache:
                self.misses += 1
                return None

            value, expiry, access_count = self.cache[key]

            # Check if expired
            if datetime.now() > expiry:
                del self.cache[key]
                del self.access_times[key]
                self.misses += 1
                return None

            # Update access info
            self.access_times[key] = datetime.now()
            self.cache[key] = (value, expiry, access_count + 1)
            self.hits += 1

            return value

    async def set(self, key: str, value: any, ttl: Optional[int] = None):
        """
        Set value in cache.

        Args:
            key: Cache key
            value: Value to cache
            ttl: Time to live in seconds (uses default if None)
        """
        async with self.lock:
            # Evict if at capacity
            if len(self.cache) >= self.max_size and key not in self.cache:
                await self._evict_lru()

            ttl = ttl or self.default_ttl
            expiry = datetime.now() + timedelta(seconds=ttl)

            self.cache[key] = (value, expiry, 0)
            self.access_times[key] = datetime.now()

    async def _evict_lru(self):
        """Evict least recently used entry."""
        if not self.access_times:
            return

        lru_key = min(self.access_times.items(), key=lambda x: x[1])[0]
        del self.cache[lru_key]
        del self.access_times[lru_key]
        logger.debug("Evicted LRU cache entry: %s", lru_key)

    async def invalidate(self, key: str):
        """Invalidate a cache entry."""
        async with self.lock:
            if key in self.cache:
                del self.cache[key]
                del self.access_times[key]

    async def clear(self):
        """Clear all cache entries."""
        async with self.lock:
            self.cache.clear()
            self.access_times.clear()
            logger.info("Cache cleared")

    def get_stats(self) -> dict:
        """Get cache statistics."""
        total_requests = self.hits + self.misses
        hit_rate = self.hits / total_requests if total_requests > 0 else 0

        return {
            "size": len(self.cache),
            "max_size": self.max_size,
            "hits": self.hits,
            "misses": self.misses,
            "hit_rate": hit_rate,
            "utilization": len(self.cache) / self.max_size,
        }


class RequestRetryStrategy:
    """
    Advanced retry strategy with exponential backoff and jitter.

    Features:
    - Exponential backoff
    - Jitter for thundering herd prevention
    - Per-error-type retry policies
    - Circuit breaker integration
    """

    def __init__(
        self,
        max_retries: int = 3,
        base_delay: float = 1.0,
        max_delay: float = 60.0,
        exponential_base: float = 2.0,
        jitter: bool = True
    ):
        """
        Initialize retry strategy.

        Args:
            max_retries: Maximum number of retry attempts
            base_delay: Base delay in seconds
            max_delay: Maximum delay in seconds
            exponential_base: Base for exponential backoff
            jitter: Whether to add jitter to delays
        """
        self.max_retries = max_retries
        self.base_delay = base_delay
        self.max_delay = max_delay
        self.exponential_base = exponential_base
        self.jitter = jitter

        # Retryable error codes
        self.retryable_status_codes = {408, 429, 500, 502, 503, 504}

        # Stats
        self.retry_counts: Dict[str, int] = defaultdict(int)
        self.success_after_retry: Dict[str, int] = defaultdict(int)

    def calculate_delay(self, attempt: int) -> float:
        """
        Calculate delay for retry attempt.

        Args:
            attempt: Attempt number (0-based)

        Returns:
            Delay in seconds
        """
        delay = min(
            self.base_delay * (self.exponential_base ** attempt),
            self.max_delay
        )

        if self.jitter:
            import random
            delay = delay * (0.5 + random.random() * 0.5)

        return delay

    def should_retry(self, error: Exception, attempt: int) -> bool:
        """
        Determine if request should be retried.

        Args:
            error: Exception that occurred
            attempt: Current attempt number

        Returns:
            True if should retry, False otherwise
        """
        if attempt >= self.max_retries:
            return False

        # Check error type
        if isinstance(error, httpx.HTTPStatusError):
            return error.response.status_code in self.retryable_status_codes

        if isinstance(error, (httpx.ConnectError, httpx.TimeoutException)):
            return True

        return False

    async def execute_with_retry(self, func, *args, **kwargs):
        """
        Execute function with retry logic.

        Args:
            func: Async function to execute
            *args: Positional arguments
            **kwargs: Keyword arguments

        Returns:
            Function result

        Raises:
            Last exception if all retries exhausted
        """
        last_error = None

        for attempt in range(self.max_retries + 1):
            try:
                result = await func(*args, **kwargs)

                if attempt > 0:
                    func_name = func.__name__
                    self.success_after_retry[func_name] += 1
                    logger.info("Succeeded on retry attempt %s for %s", attempt, func_name)

                return result

            except Exception as e:
                last_error = e
                func_name = func.__name__

                if not self.should_retry(e, attempt):
                    raise e

                if attempt < self.max_retries:
                    delay = self.calculate_delay(attempt)
                    self.retry_counts[func_name] += 1
                    logger.warning(
                        f"Retry attempt {attempt + 1}/{self.max_retries} "
                        f"for {func_name} after {delay:.2f}s delay"
                    )
                    await asyncio.sleep(delay)

        raise last_error

    def get_stats(self) -> dict:
        """Get retry statistics."""
        return {
            "total_retries": sum(self.retry_counts.values()),
            "successes_after_retry": sum(self.success_after_retry.values()),
            "by_function": dict(self.retry_counts),
        }
