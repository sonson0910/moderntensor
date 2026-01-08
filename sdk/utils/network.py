"""
Network Utilities Module

Provides utilities for network operations, connection health checks,
endpoint discovery, and retry mechanisms.
"""

import asyncio
import time
import socket
import logging
from typing import Optional, List, Dict, Callable, Any, Tuple
from urllib.parse import urlparse
import aiohttp
from dataclasses import dataclass
from enum import Enum


logger = logging.getLogger(__name__)


class NetworkError(Exception):
    """Exception raised for network-related errors."""
    pass


class EndpointStatus(Enum):
    """Status of a network endpoint."""
    HEALTHY = "healthy"
    UNHEALTHY = "unhealthy"
    DEGRADED = "degraded"
    UNKNOWN = "unknown"


@dataclass
class EndpointInfo:
    """Information about a network endpoint."""
    url: str
    status: EndpointStatus
    latency_ms: Optional[float] = None
    last_check: Optional[float] = None
    error_message: Optional[str] = None
    
    def is_healthy(self) -> bool:
        """Check if endpoint is healthy."""
        return self.status == EndpointStatus.HEALTHY


async def check_endpoint_health(
    url: str,
    timeout: float = 5.0,
    method: str = "GET",
    expected_status: int = 200
) -> EndpointInfo:
    """
    Check health of a network endpoint.
    
    Args:
        url: URL to check
        timeout: Request timeout in seconds
        method: HTTP method to use
        expected_status: Expected HTTP status code
        
    Returns:
        EndpointInfo with health status
        
    Examples:
        >>> import asyncio
        >>> info = asyncio.run(check_endpoint_health("http://localhost:8080/health"))
        >>> print(info.status)
        EndpointStatus.HEALTHY
        >>> print(f"Latency: {info.latency_ms:.2f}ms")
        Latency: 15.32ms
    """
    start_time = time.time()
    
    try:
        async with aiohttp.ClientSession() as session:
            async with session.request(
                method, 
                url, 
                timeout=aiohttp.ClientTimeout(total=timeout)
            ) as response:
                latency = (time.time() - start_time) * 1000  # Convert to ms
                
                if response.status == expected_status:
                    status = EndpointStatus.HEALTHY
                    error_msg = None
                else:
                    status = EndpointStatus.DEGRADED
                    error_msg = f"Unexpected status: {response.status}"
                
                return EndpointInfo(
                    url=url,
                    status=status,
                    latency_ms=latency,
                    last_check=time.time(),
                    error_message=error_msg
                )
    
    except asyncio.TimeoutError:
        latency = (time.time() - start_time) * 1000
        return EndpointInfo(
            url=url,
            status=EndpointStatus.UNHEALTHY,
            latency_ms=latency,
            last_check=time.time(),
            error_message="Request timeout"
        )
    
    except Exception as e:
        latency = (time.time() - start_time) * 1000
        return EndpointInfo(
            url=url,
            status=EndpointStatus.UNHEALTHY,
            latency_ms=latency,
            last_check=time.time(),
            error_message=str(e)
        )


async def check_multiple_endpoints(
    urls: List[str],
    timeout: float = 5.0,
    max_concurrent: int = 10
) -> Dict[str, EndpointInfo]:
    """
    Check health of multiple endpoints concurrently.
    
    Args:
        urls: List of URLs to check
        timeout: Request timeout per endpoint
        max_concurrent: Maximum concurrent checks
        
    Returns:
        Dictionary mapping URLs to EndpointInfo
        
    Examples:
        >>> urls = ["http://node1:8080", "http://node2:8080", "http://node3:8080"]
        >>> results = asyncio.run(check_multiple_endpoints(urls))
        >>> healthy = [url for url, info in results.items() if info.is_healthy()]
        >>> print(f"Healthy nodes: {len(healthy)}/{len(urls)}")
    """
    semaphore = asyncio.Semaphore(max_concurrent)
    
    async def check_with_semaphore(url: str) -> Tuple[str, EndpointInfo]:
        async with semaphore:
            info = await check_endpoint_health(url, timeout=timeout)
            return url, info
    
    tasks = [check_with_semaphore(url) for url in urls]
    results = await asyncio.gather(*tasks)
    
    return dict(results)


def is_port_open(host: str, port: int, timeout: float = 5.0) -> bool:
    """
    Check if a port is open on a host.
    
    Args:
        host: Hostname or IP address
        port: Port number
        timeout: Connection timeout in seconds
        
    Returns:
        True if port is open, False otherwise
        
    Examples:
        >>> is_port_open("localhost", 8080)
        True
        >>> is_port_open("localhost", 9999)
        False
    """
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(timeout)
    
    try:
        result = sock.connect_ex((host, port))
        sock.close()
        return result == 0
    except socket.error as e:
        logger.debug(f"Port check failed for {host}:{port}: {e}")
        return False


def parse_endpoint(url: str) -> Tuple[str, int, str]:
    """
    Parse endpoint URL into components.
    
    Args:
        url: URL to parse
        
    Returns:
        Tuple of (host, port, scheme)
        
    Examples:
        >>> parse_endpoint("http://localhost:8080/api")
        ('localhost', 8080, 'http')
        >>> parse_endpoint("https://node.example.com:443")
        ('node.example.com', 443, 'https')
    """
    parsed = urlparse(url)
    
    host = parsed.hostname or "localhost"
    scheme = parsed.scheme or "http"
    
    # Determine port
    if parsed.port:
        port = parsed.port
    else:
        port = 443 if scheme == "https" else 80
    
    return host, port, scheme


async def retry_async(
    func: Callable,
    *args,
    max_retries: int = 3,
    initial_delay: float = 1.0,
    backoff_factor: float = 2.0,
    max_delay: float = 60.0,
    retry_on: Optional[List[type]] = None,
    **kwargs
) -> Any:
    """
    Retry an async function with exponential backoff.
    
    Args:
        func: Async function to retry
        *args: Positional arguments for func
        max_retries: Maximum number of retry attempts
        initial_delay: Initial delay between retries in seconds
        backoff_factor: Multiplier for delay after each retry
        max_delay: Maximum delay between retries
        retry_on: List of exception types to retry on (None = all exceptions)
        **kwargs: Keyword arguments for func
        
    Returns:
        Result from successful function call
        
    Raises:
        Last exception if all retries fail
        
    Examples:
        >>> async def flaky_request():
        ...     # Simulate flaky network request
        ...     if random.random() < 0.5:
        ...         raise NetworkError("Connection failed")
        ...     return "Success"
        >>> result = await retry_async(flaky_request, max_retries=5)
        >>> print(result)
        Success
    """
    delay = initial_delay
    last_exception = None
    
    for attempt in range(max_retries + 1):
        try:
            return await func(*args, **kwargs)
        
        except Exception as e:
            last_exception = e
            
            # Check if we should retry on this exception
            if retry_on and not any(isinstance(e, exc_type) for exc_type in retry_on):
                raise
            
            # Don't retry on last attempt
            if attempt >= max_retries:
                break
            
            logger.debug(
                f"Retry attempt {attempt + 1}/{max_retries} after {delay:.2f}s "
                f"due to: {e}"
            )
            
            await asyncio.sleep(delay)
            
            # Increase delay with backoff
            delay = min(delay * backoff_factor, max_delay)
    
    # All retries failed
    if last_exception:
        raise last_exception


def retry_sync(
    func: Callable,
    *args,
    max_retries: int = 3,
    initial_delay: float = 1.0,
    backoff_factor: float = 2.0,
    max_delay: float = 60.0,
    retry_on: Optional[List[type]] = None,
    **kwargs
) -> Any:
    """
    Retry a synchronous function with exponential backoff.
    
    Args:
        func: Function to retry
        *args: Positional arguments for func
        max_retries: Maximum number of retry attempts
        initial_delay: Initial delay between retries in seconds
        backoff_factor: Multiplier for delay after each retry
        max_delay: Maximum delay between retries
        retry_on: List of exception types to retry on (None = all exceptions)
        **kwargs: Keyword arguments for func
        
    Returns:
        Result from successful function call
        
    Raises:
        Last exception if all retries fail
    """
    delay = initial_delay
    last_exception = None
    
    for attempt in range(max_retries + 1):
        try:
            return func(*args, **kwargs)
        
        except Exception as e:
            last_exception = e
            
            # Check if we should retry on this exception
            if retry_on and not any(isinstance(e, exc_type) for exc_type in retry_on):
                raise
            
            # Don't retry on last attempt
            if attempt >= max_retries:
                break
            
            logger.debug(
                f"Retry attempt {attempt + 1}/{max_retries} after {delay:.2f}s "
                f"due to: {e}"
            )
            
            time.sleep(delay)
            
            # Increase delay with backoff
            delay = min(delay * backoff_factor, max_delay)
    
    # All retries failed
    if last_exception:
        raise last_exception


class CircuitBreaker:
    """
    Circuit breaker pattern implementation for network resilience.
    
    Prevents repeated calls to a failing service by "opening" the circuit
    after a threshold of failures.
    
    States:
    - CLOSED: Normal operation, requests pass through
    - OPEN: Too many failures, requests fail immediately
    - HALF_OPEN: Testing if service recovered, limited requests pass through
    
    Examples:
        >>> breaker = CircuitBreaker(failure_threshold=5, timeout=60)
        >>> 
        >>> async def call_service():
        ...     async with breaker:
        ...         return await make_network_request()
    """
    
    def __init__(
        self,
        failure_threshold: int = 5,
        timeout: float = 60.0,
        success_threshold: int = 2
    ):
        """
        Initialize circuit breaker.
        
        Args:
            failure_threshold: Number of failures before opening circuit
            timeout: Seconds to wait before trying again (half-open state)
            success_threshold: Successful calls needed in half-open to close
        """
        self.failure_threshold = failure_threshold
        self.timeout = timeout
        self.success_threshold = success_threshold
        
        self.failure_count = 0
        self.success_count = 0
        self.last_failure_time: Optional[float] = None
        self.state = "closed"  # closed, open, half_open
        
        self._lock = asyncio.Lock()
    
    async def __aenter__(self):
        """Enter async context."""
        async with self._lock:
            if self.state == "open":
                # Check if timeout has passed
                if (time.time() - self.last_failure_time) > self.timeout:
                    logger.info("Circuit breaker entering half-open state")
                    self.state = "half_open"
                    self.success_count = 0
                else:
                    raise NetworkError("Circuit breaker is OPEN")
        
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Exit async context."""
        async with self._lock:
            if exc_type is None:
                # Success
                self._record_success()
            else:
                # Failure
                self._record_failure()
        
        return False  # Don't suppress exceptions
    
    def _record_success(self):
        """Record a successful call."""
        if self.state == "half_open":
            self.success_count += 1
            if self.success_count >= self.success_threshold:
                logger.info("Circuit breaker closing after successful recovery")
                self.state = "closed"
                self.failure_count = 0
        elif self.state == "closed":
            # Reset failure count on success
            self.failure_count = 0
    
    def _record_failure(self):
        """Record a failed call."""
        self.failure_count += 1
        self.last_failure_time = time.time()
        
        if self.state == "half_open":
            # Failed during recovery, go back to open
            logger.warning("Circuit breaker reopening after failed recovery attempt")
            self.state = "open"
        elif self.state == "closed":
            if self.failure_count >= self.failure_threshold:
                logger.warning(
                    f"Circuit breaker opening after {self.failure_count} failures"
                )
                self.state = "open"
    
    def get_state(self) -> str:
        """Get current circuit breaker state."""
        return self.state


async def wait_for_service(
    check_func: Callable[[], bool],
    timeout: float = 60.0,
    check_interval: float = 1.0
) -> bool:
    """
    Wait for a service to become available.
    
    Args:
        check_func: Function that returns True when service is ready
        timeout: Maximum time to wait in seconds
        check_interval: Time between checks in seconds
        
    Returns:
        True if service became available, False if timeout
        
    Examples:
        >>> async def check_db():
        ...     return is_port_open("localhost", 5432)
        >>> 
        >>> ready = await wait_for_service(check_db, timeout=30)
        >>> if ready:
        ...     print("Database is ready")
    """
    start_time = time.time()
    
    while (time.time() - start_time) < timeout:
        try:
            if check_func():
                return True
        except Exception as e:
            logger.debug(f"Service check failed: {e}")
        
        await asyncio.sleep(check_interval)
    
    return False


def get_local_ip() -> str:
    """
    Get local IP address.
    
    Returns:
        Local IP address as string
        
    Examples:
        >>> ip = get_local_ip()
        >>> print(ip)
        192.168.1.100
    """
    try:
        # Create a socket to determine local IP
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        # Connect to a public DNS server (doesn't actually send data)
        s.connect(("8.8.8.8", 80))
        local_ip = s.getsockname()[0]
        s.close()
        return local_ip
    except Exception:
        # Fallback to localhost
        return "127.0.0.1"


def format_url(host: str, port: int, scheme: str = "http", path: str = "") -> str:
    """
    Format a URL from components.
    
    Args:
        host: Hostname or IP
        port: Port number
        scheme: URL scheme (http/https)
        path: Optional path component
        
    Returns:
        Formatted URL string
        
    Examples:
        >>> format_url("localhost", 8080, "http", "/api/v1")
        'http://localhost:8080/api/v1'
        >>> format_url("192.168.1.1", 443, "https")
        'https://192.168.1.1:443'
    """
    url = f"{scheme}://{host}:{port}"
    if path:
        if not path.startswith("/"):
            path = "/" + path
        url += path
    return url
