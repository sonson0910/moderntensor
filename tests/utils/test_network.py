"""
Tests for Network Utilities

Comprehensive test suite for network operations and utilities.
"""

import pytest
import asyncio
import time
from unittest.mock import Mock, patch, AsyncMock
from sdk.utils.network import (
    NetworkError,
    EndpointStatus,
    EndpointInfo,
    check_endpoint_health,
    check_multiple_endpoints,
    is_port_open,
    parse_endpoint,
    retry_async,
    retry_sync,
    CircuitBreaker,
    wait_for_service,
    get_local_ip,
    format_url,
)


class TestEndpointInfo:
    """Test EndpointInfo dataclass."""
    
    def test_endpoint_info_creation(self):
        """Test creating EndpointInfo."""
        info = EndpointInfo(
            url="http://localhost:8080",
            status=EndpointStatus.HEALTHY,
            latency_ms=15.5
        )
        assert info.url == "http://localhost:8080"
        assert info.status == EndpointStatus.HEALTHY
        assert info.latency_ms == 15.5
    
    def test_endpoint_is_healthy(self):
        """Test is_healthy method."""
        healthy = EndpointInfo(
            url="http://test", 
            status=EndpointStatus.HEALTHY
        )
        assert healthy.is_healthy()
        
        unhealthy = EndpointInfo(
            url="http://test",
            status=EndpointStatus.UNHEALTHY
        )
        assert not unhealthy.is_healthy()


class TestParseEndpoint:
    """Test endpoint parsing."""
    
    def test_parse_http_endpoint(self):
        """Test parsing HTTP endpoint."""
        host, port, scheme = parse_endpoint("http://localhost:8080/api")
        assert host == "localhost"
        assert port == 8080
        assert scheme == "http"
    
    def test_parse_https_endpoint(self):
        """Test parsing HTTPS endpoint."""
        host, port, scheme = parse_endpoint("https://example.com:443")
        assert host == "example.com"
        assert port == 443
        assert scheme == "https"
    
    def test_parse_endpoint_default_port(self):
        """Test parsing endpoint with default port."""
        host, port, scheme = parse_endpoint("http://localhost")
        assert host == "localhost"
        assert port == 80
        assert scheme == "http"
        
        host, port, scheme = parse_endpoint("https://example.com")
        assert host == "example.com"
        assert port == 443
        assert scheme == "https"
    
    def test_parse_ip_endpoint(self):
        """Test parsing IP address endpoint."""
        host, port, scheme = parse_endpoint("http://192.168.1.1:8080")
        assert host == "192.168.1.1"
        assert port == 8080


class TestFormatUrl:
    """Test URL formatting."""
    
    def test_format_basic_url(self):
        """Test formatting basic URL."""
        url = format_url("localhost", 8080, "http")
        assert url == "http://localhost:8080"
    
    def test_format_url_with_path(self):
        """Test formatting URL with path."""
        url = format_url("localhost", 8080, "http", "/api/v1")
        assert url == "http://localhost:8080/api/v1"
    
    def test_format_url_path_without_slash(self):
        """Test formatting URL with path without leading slash."""
        url = format_url("localhost", 8080, "http", "api/v1")
        assert url == "http://localhost:8080/api/v1"
    
    def test_format_https_url(self):
        """Test formatting HTTPS URL."""
        url = format_url("example.com", 443, "https")
        assert url == "https://example.com:443"


class TestRetrySync:
    """Test synchronous retry mechanism."""
    
    def test_retry_sync_success_first_try(self):
        """Test successful call on first try."""
        mock_func = Mock(return_value="success")
        result = retry_sync(mock_func, max_retries=3)
        assert result == "success"
        assert mock_func.call_count == 1
    
    def test_retry_sync_success_after_failures(self):
        """Test successful call after some failures."""
        mock_func = Mock(side_effect=[Exception("fail"), Exception("fail"), "success"])
        result = retry_sync(mock_func, max_retries=3, initial_delay=0.01)
        assert result == "success"
        assert mock_func.call_count == 3
    
    def test_retry_sync_all_failures(self):
        """Test all retries fail."""
        mock_func = Mock(side_effect=Exception("always fails"))
        with pytest.raises(Exception) as exc_info:
            retry_sync(mock_func, max_retries=2, initial_delay=0.01)
        assert "always fails" in str(exc_info.value)
        assert mock_func.call_count == 3  # Initial + 2 retries
    
    def test_retry_sync_with_specific_exception(self):
        """Test retry only on specific exceptions."""
        mock_func = Mock(side_effect=ValueError("specific error"))
        
        # Should retry on ValueError
        with pytest.raises(ValueError):
            retry_sync(mock_func, max_retries=2, retry_on=[ValueError], initial_delay=0.01)
        assert mock_func.call_count == 3
        
        # Should not retry on TypeError
        mock_func.reset_mock()
        mock_func.side_effect = TypeError("type error")
        with pytest.raises(TypeError):
            retry_sync(mock_func, max_retries=2, retry_on=[ValueError], initial_delay=0.01)
        assert mock_func.call_count == 1  # No retries


class TestRetryAsync:
    """Test asynchronous retry mechanism."""
    
    @pytest.mark.asyncio
    async def test_retry_async_success_first_try(self):
        """Test successful async call on first try."""
        mock_func = AsyncMock(return_value="success")
        result = await retry_async(mock_func, max_retries=3)
        assert result == "success"
        assert mock_func.call_count == 1
    
    @pytest.mark.asyncio
    async def test_retry_async_success_after_failures(self):
        """Test successful async call after failures."""
        mock_func = AsyncMock(side_effect=[Exception("fail"), Exception("fail"), "success"])
        result = await retry_async(mock_func, max_retries=3, initial_delay=0.01)
        assert result == "success"
        assert mock_func.call_count == 3
    
    @pytest.mark.asyncio
    async def test_retry_async_exponential_backoff(self):
        """Test exponential backoff delays."""
        mock_func = AsyncMock(side_effect=Exception("fail"))
        start_time = time.time()
        
        with pytest.raises(Exception):
            await retry_async(
                mock_func,
                max_retries=2,
                initial_delay=0.1,
                backoff_factor=2.0
            )
        
        elapsed = time.time() - start_time
        # Should have delays of ~0.1 and ~0.2 seconds
        assert elapsed >= 0.3  # At least 0.1 + 0.2
    
    @pytest.mark.asyncio
    async def test_retry_async_max_delay(self):
        """Test maximum delay cap."""
        mock_func = AsyncMock(side_effect=Exception("fail"))
        start_time = time.time()
        
        with pytest.raises(Exception):
            await retry_async(
                mock_func,
                max_retries=3,
                initial_delay=0.1,
                backoff_factor=10.0,  # Would grow quickly
                max_delay=0.15  # But capped at 0.15
            )
        
        elapsed = time.time() - start_time
        # Max delay is 0.15, so should not exceed 0.15 * 3 + buffer
        assert elapsed < 0.6


class TestCircuitBreaker:
    """Test circuit breaker pattern."""
    
    @pytest.mark.asyncio
    async def test_circuit_breaker_closed_state(self):
        """Test circuit breaker in closed state."""
        breaker = CircuitBreaker(failure_threshold=3, timeout=1.0)
        
        # Should allow requests
        async with breaker:
            pass
        
        assert breaker.get_state() == "closed"
    
    @pytest.mark.asyncio
    async def test_circuit_breaker_opens_after_failures(self):
        """Test circuit breaker opens after threshold failures."""
        breaker = CircuitBreaker(failure_threshold=3, timeout=1.0)
        
        # Trigger failures
        for _ in range(3):
            try:
                async with breaker:
                    raise Exception("simulated failure")
            except Exception:
                pass
        
        assert breaker.get_state() == "open"
    
    @pytest.mark.asyncio
    async def test_circuit_breaker_rejects_when_open(self):
        """Test circuit breaker rejects requests when open."""
        breaker = CircuitBreaker(failure_threshold=2, timeout=1.0)
        
        # Trigger failures to open circuit
        for _ in range(2):
            try:
                async with breaker:
                    raise Exception("failure")
            except Exception:
                pass
        
        # Should now reject requests
        with pytest.raises(NetworkError) as exc_info:
            async with breaker:
                pass
        assert "OPEN" in str(exc_info.value)
    
    @pytest.mark.asyncio
    async def test_circuit_breaker_half_open_recovery(self):
        """Test circuit breaker recovery through half-open state."""
        breaker = CircuitBreaker(
            failure_threshold=2,
            timeout=0.1,  # Short timeout for testing
            success_threshold=2
        )
        
        # Open the circuit
        for _ in range(2):
            try:
                async with breaker:
                    raise Exception("failure")
            except Exception:
                pass
        
        assert breaker.get_state() == "open"
        
        # Wait for timeout
        await asyncio.sleep(0.15)
        
        # Next request should move to half-open
        async with breaker:
            pass  # Success
        
        assert breaker.get_state() == "half_open"
        
        # Another success should close the circuit
        async with breaker:
            pass  # Success
        
        assert breaker.get_state() == "closed"


class TestWaitForService:
    """Test service availability waiting."""
    
    @pytest.mark.asyncio
    async def test_wait_for_service_immediate(self):
        """Test waiting for immediately available service."""
        check_func = Mock(return_value=True)
        ready = await wait_for_service(check_func, timeout=1.0, check_interval=0.1)
        assert ready
        assert check_func.call_count >= 1
    
    @pytest.mark.asyncio
    async def test_wait_for_service_eventual(self):
        """Test waiting for eventually available service."""
        check_func = Mock(side_effect=[False, False, True])
        ready = await wait_for_service(check_func, timeout=1.0, check_interval=0.1)
        assert ready
        assert check_func.call_count == 3
    
    @pytest.mark.asyncio
    async def test_wait_for_service_timeout(self):
        """Test timeout when service never becomes available."""
        check_func = Mock(return_value=False)
        ready = await wait_for_service(check_func, timeout=0.2, check_interval=0.1)
        assert not ready


class TestIsPortOpen:
    """Test port availability checking."""
    
    def test_is_port_open_localhost_closed(self):
        """Test checking closed port on localhost."""
        # Port 65000 should not be open
        assert not is_port_open("localhost", 65000, timeout=0.5)
    
    def test_is_port_open_invalid_host(self):
        """Test checking port on invalid host."""
        # Non-existent host should return False
        assert not is_port_open("invalid-host-that-does-not-exist-12345", 80, timeout=0.5)


class TestGetLocalIp:
    """Test local IP retrieval."""
    
    def test_get_local_ip(self):
        """Test getting local IP address."""
        ip = get_local_ip()
        assert ip is not None
        assert len(ip) > 0
        # Should be valid IP format
        parts = ip.split('.')
        assert len(parts) == 4
        for part in parts:
            assert 0 <= int(part) <= 255


class TestCheckEndpointHealth:
    """Test endpoint health checking."""
    
    @pytest.mark.asyncio
    async def test_check_endpoint_timeout(self):
        """Test endpoint health check with timeout."""
        # Use a non-routable IP to force timeout
        info = await check_endpoint_health(
            "http://192.0.2.1:80",  # TEST-NET-1, should timeout
            timeout=0.5
        )
        assert info.status == EndpointStatus.UNHEALTHY
        assert "timeout" in info.error_message.lower() or info.error_message is not None


class TestCheckMultipleEndpoints:
    """Test multiple endpoint checking."""
    
    @pytest.mark.asyncio
    async def test_check_multiple_endpoints(self):
        """Test checking multiple endpoints concurrently."""
        urls = [
            "http://192.0.2.1:80",  # Should timeout
            "http://192.0.2.2:80",  # Should timeout
        ]
        results = await check_multiple_endpoints(urls, timeout=0.5, max_concurrent=2)
        assert len(results) == 2
        assert all(isinstance(info, EndpointInfo) for info in results.values())


class TestEdgeCases:
    """Test edge cases and error conditions."""
    
    def test_retry_sync_zero_retries(self):
        """Test retry with zero retries."""
        mock_func = Mock(side_effect=Exception("fail"))
        with pytest.raises(Exception):
            retry_sync(mock_func, max_retries=0, initial_delay=0.01)
        assert mock_func.call_count == 1  # Only initial attempt
    
    @pytest.mark.asyncio
    async def test_retry_async_zero_retries(self):
        """Test async retry with zero retries."""
        mock_func = AsyncMock(side_effect=Exception("fail"))
        with pytest.raises(Exception):
            await retry_async(mock_func, max_retries=0, initial_delay=0.01)
        assert mock_func.call_count == 1


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
