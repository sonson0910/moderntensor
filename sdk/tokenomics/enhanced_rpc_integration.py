"""
Enhanced RPC Integration for Tokenomics (Month 1 Implementation)

This module provides production-ready RPC integration between the Python SDK
and Luxtensor blockchain with:
- Comprehensive error handling
- Automatic retry mechanisms with exponential backoff
- Connection pooling for performance
- Request batching capabilities
- Health monitoring and circuit breaker pattern
- Detailed logging and metrics

Implementation Date: January 2026 (Month 1 Roadmap)
"""

import asyncio
import logging
import time
from typing import Dict, List, Optional, Any
from dataclasses import dataclass
from enum import Enum
import aiohttp
from aiohttp import ClientSession, ClientTimeout, TCPConnector


logger = logging.getLogger(__name__)


class ConnectionState(Enum):
    """Connection state for circuit breaker pattern."""
    CLOSED = "closed"  # Normal operation
    OPEN = "open"      # Circuit open, rejecting requests
    HALF_OPEN = "half_open"  # Testing if service recovered


@dataclass
class RPCConfig:
    """Configuration for RPC integration."""
    # Connection settings
    url: str = "http://localhost:8545"
    timeout: int = 30
    max_connections: int = 100
    connection_pool_size: int = 50

    # Retry settings
    max_retry_attempts: int = 3
    retry_delay: float = 1.0
    retry_exponential_base: float = 2.0
    max_retry_delay: float = 30.0

    # Circuit breaker settings
    failure_threshold: int = 5
    recovery_timeout: int = 60
    half_open_max_calls: int = 3

    # Performance settings
    batch_size: int = 50
    batch_timeout: float = 0.1

    # Health check settings
    health_check_interval: int = 30
    health_check_timeout: int = 5


@dataclass
class RPCMetrics:
    """Metrics for RPC operations."""
    total_requests: int = 0
    successful_requests: int = 0
    failed_requests: int = 0
    retry_count: int = 0
    average_response_time: float = 0.0
    last_error: Optional[str] = None
    last_error_time: Optional[float] = None
    circuit_breaker_trips: int = 0

    def record_request(self, success: bool, response_time: float, error: Optional[str] = None):
        """Record a request metric."""
        self.total_requests += 1
        if success:
            self.successful_requests += 1
        else:
            self.failed_requests += 1
            self.last_error = error
            self.last_error_time = time.time()

        # Update rolling average
        alpha = 0.1  # Smoothing factor
        self.average_response_time = (
            alpha * response_time +
            (1 - alpha) * self.average_response_time
        )


class CircuitBreaker:
    """
    Circuit breaker pattern implementation.

    Prevents cascading failures by temporarily blocking requests
    when the backend service is unavailable.
    """

    def __init__(self, config: RPCConfig):
        self.config = config
        self.state = ConnectionState.CLOSED
        self.failure_count = 0
        self.last_failure_time: Optional[float] = None
        self.half_open_calls = 0

    def can_execute(self) -> bool:
        """Check if request can be executed."""
        if self.state == ConnectionState.CLOSED:
            return True

        if self.state == ConnectionState.OPEN:
            # Check if recovery timeout has passed
            if (time.time() - self.last_failure_time) >= self.config.recovery_timeout:
                self.state = ConnectionState.HALF_OPEN
                self.half_open_calls = 0
                logger.info("Circuit breaker entering HALF_OPEN state")
                return True
            return False

        if self.state == ConnectionState.HALF_OPEN:
            return self.half_open_calls < self.config.half_open_max_calls

        return False

    def record_success(self):
        """Record successful request."""
        if self.state == ConnectionState.HALF_OPEN:
            self.half_open_calls += 1
            if self.half_open_calls >= self.config.half_open_max_calls:
                self.state = ConnectionState.CLOSED
                self.failure_count = 0
                logger.info("Circuit breaker CLOSED - service recovered")
        elif self.state == ConnectionState.CLOSED:
            self.failure_count = max(0, self.failure_count - 1)

    def record_failure(self):
        """Record failed request."""
        self.failure_count += 1
        self.last_failure_time = time.time()

        if self.state == ConnectionState.HALF_OPEN:
            self.state = ConnectionState.OPEN
            logger.warning("Circuit breaker OPEN - service still unhealthy")
        elif self.failure_count >= self.config.failure_threshold:
            self.state = ConnectionState.OPEN
            logger.warning("Circuit breaker OPEN - %s failures", self.failure_count)


class EnhancedRPCIntegration:
    """
    Production-ready RPC integration for tokenomics.

    Features:
    - Automatic retry with exponential backoff
    - Connection pooling
    - Circuit breaker pattern
    - Request batching
    - Health monitoring
    - Comprehensive error handling
    """

    def __init__(self, config: Optional[RPCConfig] = None):
        """
        Initialize enhanced RPC integration.

        Args:
            config: RPC configuration (uses defaults if not provided)
        """
        self.config = config or RPCConfig()
        self.metrics = RPCMetrics()
        self.circuit_breaker = CircuitBreaker(self.config)

        self._session: Optional[ClientSession] = None
        self._is_healthy = False
        self._health_check_task: Optional[asyncio.Task] = None
        self._batch_queue: List[Dict[str, Any]] = []
        self._batch_lock = asyncio.Lock()

        logger.info("EnhancedRPCIntegration initialized with URL: %s", self.config.url)

    async def connect(self):
        """Establish connection pool."""
        if self._session is None:
            connector = TCPConnector(
                limit=self.config.max_connections,
                limit_per_host=self.config.connection_pool_size,
                ttl_dns_cache=300,
            )

            self._session = ClientSession(
                connector=connector,
                timeout=ClientTimeout(total=self.config.timeout),
            )

            # Start health check
            self._health_check_task = asyncio.create_task(self._health_check_loop())

            logger.info("Connection pool established")

    async def close(self):
        """Close connection pool."""
        if self._health_check_task:
            self._health_check_task.cancel()
            try:
                await self._health_check_task
            except asyncio.CancelledError:
                pass

        if self._session:
            await self._session.close()
            self._session = None
            logger.info("Connection pool closed")

    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()

    async def _health_check_loop(self):
        """Periodically check blockchain health."""
        while True:
            try:
                await asyncio.sleep(self.config.health_check_interval)
                self._is_healthy = await self._check_health()

                if not self._is_healthy:
                    logger.warning("Health check failed - blockchain may be unavailable")
                    self.circuit_breaker.record_failure()
                else:
                    logger.debug("Health check passed")

            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error("Health check error: %s", e)

    async def _check_health(self) -> bool:
        """Check if blockchain is healthy."""
        try:
            # Try to get block number as health check
            result = await self._execute_request(
                method="eth_blockNumber",
                params=[],
                skip_circuit_breaker=True,
                timeout=self.config.health_check_timeout
            )
            return result is not None
        except Exception:
            return False

    async def _execute_request(
        self,
        method: str,
        params: List[Any],
        skip_circuit_breaker: bool = False,
        timeout: Optional[int] = None
    ) -> Any:
        """
        Execute a single RPC request with retry logic.

        Args:
            method: RPC method name
            params: Method parameters
            skip_circuit_breaker: If True, bypass circuit breaker
            timeout: Optional timeout override

        Returns:
            RPC response result

        Raises:
            Exception: If all retry attempts fail
        """
        if not skip_circuit_breaker and not self.circuit_breaker.can_execute():
            raise Exception("Circuit breaker is OPEN - service unavailable")

        request_payload = {
            "jsonrpc": "2.0",
            "id": self.metrics.total_requests + 1,
            "method": method,
            "params": params
        }

        last_error = None
        retry_delay = self.config.retry_delay

        for attempt in range(self.config.max_retry_attempts):
            start_time = time.time()

            try:
                if self._session is None:
                    await self.connect()

                async with self._session.post(
                    self.config.url,
                    json=request_payload,
                    timeout=ClientTimeout(total=timeout or self.config.timeout)
                ) as response:
                    response.raise_for_status()
                    data = await response.json()

                    response_time = time.time() - start_time

                    if "error" in data:
                        raise Exception(f"RPC error: {data['error']}")

                    # Success
                    self.metrics.record_request(True, response_time)
                    self.circuit_breaker.record_success()

                    return data.get("result")

            except asyncio.TimeoutError:
                last_error = f"Timeout after {timeout or self.config.timeout}s"
                logger.warning("Request timeout (attempt %s): %s", attempt + 1, method)

            except aiohttp.ClientError as e:
                last_error = f"Client error: {str(e)}"
                logger.warning("Client error (attempt %s): %s", attempt + 1, e)

            except Exception as e:
                last_error = str(e)
                logger.warning("Request failed (attempt %s): %s", attempt + 1, e)

            # Record failure
            response_time = time.time() - start_time
            self.metrics.record_request(False, response_time, last_error)
            self.metrics.retry_count += 1
            self.circuit_breaker.record_failure()

            # Wait before retry (with exponential backoff)
            if attempt < self.config.max_retry_attempts - 1:
                await asyncio.sleep(min(retry_delay, self.config.max_retry_delay))
                retry_delay *= self.config.retry_exponential_base

        # All attempts failed
        raise Exception(f"All {self.config.max_retry_attempts} attempts failed. Last error: {last_error}")

    async def execute_rpc_call(
        self,
        method: str,
        params: List[Any] = None,
        timeout: Optional[int] = None
    ) -> Any:
        """
        Execute an RPC call with full error handling and retry logic.

        Args:
            method: RPC method name
            params: Method parameters
            timeout: Optional timeout override

        Returns:
            RPC response result
        """
        return await self._execute_request(
            method=method,
            params=params or [],
            timeout=timeout
        )

    async def batch_execute(
        self,
        requests: List[Dict[str, Any]]
    ) -> List[Any]:
        """
        Execute multiple RPC calls in a batch.

        Args:
            requests: List of {"method": str, "params": List} dicts

        Returns:
            List of results in same order
        """
        if not requests:
            return []

        # Build batch request
        batch_payload = [
            {
                "jsonrpc": "2.0",
                "id": i,
                "method": req["method"],
                "params": req.get("params", [])
            }
            for i, req in enumerate(requests)
        ]

        start_time = time.time()

        try:
            if self._session is None:
                await self.connect()

            async with self._session.post(
                self.config.url,
                json=batch_payload,
                timeout=ClientTimeout(total=self.config.timeout * 2)  # Longer timeout for batch
            ) as response:
                response.raise_for_status()
                data = await response.json()

                response_time = time.time() - start_time
                self.metrics.record_request(True, response_time)

                # Sort by ID and extract results
                sorted_data = sorted(data, key=lambda x: x["id"])
                return [item.get("result") for item in sorted_data]

        except Exception as e:
            response_time = time.time() - start_time
            self.metrics.record_request(False, response_time, str(e))
            logger.error("Batch execution failed: %s", e)
            raise

    def get_metrics(self) -> Dict[str, Any]:
        """Get current metrics."""
        return {
            "total_requests": self.metrics.total_requests,
            "successful_requests": self.metrics.successful_requests,
            "failed_requests": self.metrics.failed_requests,
            "success_rate": (
                self.metrics.successful_requests / self.metrics.total_requests * 100
                if self.metrics.total_requests > 0 else 0
            ),
            "retry_count": self.metrics.retry_count,
            "average_response_time_ms": self.metrics.average_response_time * 1000,
            "circuit_breaker_state": self.circuit_breaker.state.value,
            "circuit_breaker_trips": self.metrics.circuit_breaker_trips,
            "is_healthy": self._is_healthy,
            "last_error": self.metrics.last_error,
            "last_error_time": self.metrics.last_error_time
        }

    def is_healthy(self) -> bool:
        """Check if connection is healthy."""
        return self._is_healthy and self.circuit_breaker.state != ConnectionState.OPEN
