"""
Dendrite client implementation.

The main Dendrite client class for querying multiple miners with
resilience, load balancing, and response aggregation.
"""

from typing import List, Dict, Any, Optional, Tuple
import asyncio
import httpx
import time
import random
import logging
from datetime import datetime
from urllib.parse import urlparse
import hashlib

from .config import DendriteConfig, DendriteMetrics, RetryStrategy
from .pool import ConnectionPool, CircuitBreaker
from .aggregator import ResponseAggregator

logger = logging.getLogger(__name__)


class Dendrite:
    """
    Dendrite client for querying ModernTensor miners.

    Provides async HTTP client with connection pooling, retry logic,
    circuit breakers, load balancing, and response aggregation.
    """

    def __init__(self, config: Optional[DendriteConfig] = None):
        """
        Initialize Dendrite client.

        Args:
            config: Dendrite configuration (uses defaults if not provided)
        """
        self.config = config or DendriteConfig()

        # Initialize components
        self.pool = ConnectionPool(self.config)
        self.circuit_breaker = CircuitBreaker(
            threshold=self.config.circuit_breaker_threshold,
            timeout=self.config.circuit_breaker_timeout,
            half_open_max_calls=self.config.circuit_breaker_half_open_max_calls,
        ) if self.config.circuit_breaker_enabled else None

        self.aggregator = ResponseAggregator()

        # Metrics tracking
        self.metrics = DendriteMetrics()
        self.start_time = time.time()

        # Query cache {hash: (response, timestamp)}
        self.cache: Dict[str, Tuple[Any, datetime]] = {}

        # Request deduplication {hash: Future}
        self.in_flight: Dict[str, asyncio.Future] = {}

        # Load balancing state
        self.round_robin_index: Dict[str, int] = {}

        logger.info("Dendrite client initialized")

    async def query(
        self,
        endpoints: List[str],
        data: Dict[str, Any],
        headers: Optional[Dict[str, str]] = None,
        timeout: Optional[float] = None,
        aggregate: bool = True,
        aggregation_strategy: Optional[str] = None,
    ) -> Any:
        """
        Query multiple endpoints and aggregate responses.

        Args:
            endpoints: List of miner endpoint URLs
            data: Request payload
            headers: Optional request headers
            timeout: Optional timeout override
            aggregate: Whether to aggregate responses
            aggregation_strategy: Strategy to use for aggregation

        Returns:
            Aggregated response or list of responses
        """
        if not endpoints:
            logger.warning("No endpoints provided for query")
            return None

        # Check cache
        if self.config.cache_enabled:
            cache_key = self._get_cache_key(endpoints, data)
            cached = self._get_from_cache(cache_key)
            if cached is not None:
                self.metrics.cached_responses += 1
                logger.debug("Cache hit for query")
                return cached

        # Execute queries
        start_time = time.time()

        if self.config.parallel_queries:
            responses = await self._query_parallel(endpoints, data, headers, timeout)
        else:
            responses = await self._query_sequential(endpoints, data, headers, timeout)

        duration = time.time() - start_time

        # Update metrics
        self.metrics.total_queries += 1
        successful = [r for r in responses if r is not None]
        self.metrics.successful_queries += len(successful)
        self.metrics.failed_queries += len(responses) - len(successful)

        # Update average response time
        if self.metrics.total_queries > 0:
            self.metrics.average_response_time = (
                (self.metrics.average_response_time * (self.metrics.total_queries - 1) + duration)
                / self.metrics.total_queries
            )

        logger.info(
            f"Query completed: {len(successful)}/{len(responses)} successful "
            f"in {duration:.3f}s"
        )

        # Aggregate responses
        if aggregate and successful:
            strategy = aggregation_strategy or self.config.aggregation_strategy
            result = self.aggregator.aggregate(successful, strategy=strategy)

            # Cache result
            if self.config.cache_enabled and result is not None:
                self._add_to_cache(cache_key, result)

            return result

        return successful if aggregate else responses

    async def query_single(
        self,
        endpoint: str,
        data: Dict[str, Any],
        headers: Optional[Dict[str, str]] = None,
        timeout: Optional[float] = None,
        retry: bool = True,
    ) -> Optional[Dict[str, Any]]:
        """
        Query a single endpoint with retry logic.

        Args:
            endpoint: Miner endpoint URL
            data: Request payload
            headers: Optional request headers
            timeout: Optional timeout override
            retry: Whether to retry on failure

        Returns:
            Response dictionary or None on failure
        """
        host = urlparse(endpoint).netloc

        # Check circuit breaker
        if self.circuit_breaker and not self.circuit_breaker.can_attempt(host):
            logger.warning("Circuit breaker open for %s, skipping request", host)
            return None

        # Check deduplication
        if self.config.deduplication_enabled:
            dedup_key = self._get_dedup_key(endpoint, data)
            if dedup_key in self.in_flight:
                logger.debug("Request already in flight, waiting for result")
                try:
                    return await asyncio.wait_for(
                        self.in_flight[dedup_key],
                        timeout=timeout or self.config.query_timeout
                    )
                except asyncio.TimeoutError:
                    pass

        # Create future for deduplication
        future = asyncio.Future()
        if self.config.deduplication_enabled:
            self.in_flight[dedup_key] = future

        try:
            # Execute request with retries
            if retry and self.config.max_retries > 0:
                response = await self._query_with_retry(endpoint, data, headers, timeout)
            else:
                response = await self._execute_request(endpoint, data, headers, timeout)

            # Record success with circuit breaker
            if self.circuit_breaker and response is not None:
                self.circuit_breaker.record_success(host)

            # Set future result
            if not future.done():
                future.set_result(response)

            return response

        except Exception as e:
            logger.error("Query failed for %s: %s", endpoint, e)

            # Record failure with circuit breaker
            if self.circuit_breaker:
                self.circuit_breaker.record_failure(host)

            # Set future exception
            if not future.done():
                future.set_exception(e)

            return None

        finally:
            # Cleanup deduplication
            if self.config.deduplication_enabled and dedup_key in self.in_flight:
                del self.in_flight[dedup_key]

    async def _query_parallel(
        self,
        endpoints: List[str],
        data: Dict[str, Any],
        headers: Optional[Dict[str, str]],
        timeout: Optional[float],
    ) -> List[Optional[Dict[str, Any]]]:
        """Execute queries in parallel."""
        # Limit parallelism
        semaphore = asyncio.Semaphore(self.config.max_parallel_queries)

        async def bounded_query(endpoint):
            async with semaphore:
                return await self.query_single(endpoint, data, headers, timeout)

        tasks = [bounded_query(ep) for ep in endpoints]
        return await asyncio.gather(*tasks, return_exceptions=False)

    async def _query_sequential(
        self,
        endpoints: List[str],
        data: Dict[str, Any],
        headers: Optional[Dict[str, str]],
        timeout: Optional[float],
    ) -> List[Optional[Dict[str, Any]]]:
        """Execute queries sequentially."""
        responses = []
        for endpoint in endpoints:
            response = await self.query_single(endpoint, data, headers, timeout)
            responses.append(response)

            # Early exit if we have enough responses
            successful = [r for r in responses if r is not None]
            if len(successful) >= self.config.min_responses:
                break

        return responses

    async def _query_with_retry(
        self,
        endpoint: str,
        data: Dict[str, Any],
        headers: Optional[Dict[str, str]],
        timeout: Optional[float],
    ) -> Optional[Dict[str, Any]]:
        """Execute request with retry logic."""
        last_error = None

        for attempt in range(self.config.max_retries + 1):
            try:
                response = await self._execute_request(endpoint, data, headers, timeout)

                if response is not None:
                    if attempt > 0:
                        self.metrics.retried_queries += 1
                        logger.info("Query succeeded on attempt %s", attempt + 1)
                    return response

            except Exception as e:
                _ = e  # Capture exception for logging context
                logger.warning("Query attempt %s failed: %s", attempt + 1, e)

            # Don't sleep after last attempt
            if attempt < self.config.max_retries:
                delay = self._calculate_retry_delay(attempt)
                logger.debug("Retrying in %ss...", delay:.2f)
                await asyncio.sleep(delay)

        logger.error("Query failed after %s attempts", self.config.max_retries + 1)
        return None

    def _calculate_retry_delay(self, attempt: int) -> float:
        """Calculate delay before next retry."""
        if self.config.retry_strategy == RetryStrategy.EXPONENTIAL_BACKOFF:
            delay = self.config.retry_delay * (2 ** attempt)
        elif self.config.retry_strategy == RetryStrategy.LINEAR_BACKOFF:
            delay = self.config.retry_delay * (attempt + 1)
        else:  # FIXED_DELAY
            delay = self.config.retry_delay

        # Cap at max delay
        delay = min(delay, self.config.max_retry_delay)

        # Add jitter (±20%)
        jitter = delay * 0.2 * (2 * random.random() - 1)
        return delay + jitter

    async def _execute_request(
        self,
        endpoint: str,
        data: Dict[str, Any],
        headers: Optional[Dict[str, str]],
        timeout: Optional[float],
    ) -> Optional[Dict[str, Any]]:
        """Execute HTTP request."""
        client = await self.pool.get_client()
        host = urlparse(endpoint).netloc

        # Track connection
        self.pool.track_connection(host)

        try:
            # Merge headers
            request_headers = {**self.config.default_headers}
            if headers:
                request_headers.update(headers)

            # Execute request
            response = await client.post(
                endpoint,
                json=data,
                headers=request_headers,
                timeout=timeout or self.config.query_timeout,
            )

            response.raise_for_status()

            # Reset errors on success
            self.pool.reset_errors(host)

            return response.json()

        except httpx.HTTPError as e:
            logger.error("HTTP error for %s: %s", endpoint, e)
            self.pool.record_error(host)
            return None

        finally:
            self.pool.release_connection(host)

    def _get_cache_key(self, endpoints: List[str], data: Dict[str, Any]) -> str:
        """Generate cache key from endpoints and data."""
        key_str = f"{sorted(endpoints)}:{str(data)}"
        return hashlib.sha256(key_str.encode()).hexdigest()

    def _get_dedup_key(self, endpoint: str, data: Dict[str, Any]) -> str:
        """Generate deduplication key."""
        key_str = f"{endpoint}:{str(data)}"
        return hashlib.sha256(key_str.encode()).hexdigest()

    def _get_from_cache(self, key: str) -> Optional[Any]:
        """Get response from cache if not expired."""
        if key in self.cache:
            response, timestamp = self.cache[key]
            age = (datetime.now() - timestamp).total_seconds()

            if age < self.config.cache_ttl:
                return response
            else:
                # Remove expired entry
                del self.cache[key]

        return None

    def _add_to_cache(self, key: str, response: Any):
        """Add response to cache."""
        # Check cache size limit
        if len(self.cache) >= self.config.cache_max_size:
            # Remove oldest entry
            oldest_key = min(self.cache.keys(), key=lambda k: self.cache[k][1])
            del self.cache[oldest_key]

        self.cache[key] = (response, datetime.now())

    def get_metrics(self) -> DendriteMetrics:
        """Get current client metrics."""
        self.metrics.active_connections = sum(self.pool.active_connections.values())
        return self.metrics

    async def close(self):
        """Close the Dendrite client and cleanup resources."""
        await self.pool.close()
        self.cache.clear()
        self.in_flight.clear()
        logger.info("Dendrite client closed")


# Convenience function to create a Dendrite client
def create_dendrite(config: Optional[DendriteConfig] = None) -> Dendrite:
    """
    Create a Dendrite client instance.

    Args:
        config: Dendrite configuration

    Returns:
        Configured Dendrite instance
    """
    return Dendrite(config=config)
