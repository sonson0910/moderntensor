"""
Connection pool management for Dendrite.

Manages HTTP connections with pooling, keep-alive, and limits.
"""

import httpx
import asyncio
from typing import Optional, Dict
from datetime import datetime, timedelta
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
            follow_redirects=True,
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
        logger.warning(f"Connection error for {host}, total errors: {self.connection_errors[host]}")
    
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
            logger.info(f"Cleaned up idle connections for {len(hosts_to_remove)} hosts")
    
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
            logger.info(f"Circuit closed for {host} after successful recovery")
        
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
            logger.warning(f"Circuit opened again for {host} after half-open failure")
        
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
