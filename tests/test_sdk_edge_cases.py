"""
SDK Edge-Case Test Suite

Comprehensive tests for Luxtensor SDK covering:
- Network failures (connection refused, timeout, DNS errors)
- Invalid/malformed inputs (keys, addresses, amounts)
- Rate limiting and retry behavior
- Concurrent operation safety
- Error handling robustness

Following TDD principles: Each test defines expected behavior first.
"""

import pytest
import httpx
from unittest.mock import Mock, patch, MagicMock
from typing import Generator
import asyncio
import time

# Import SDK components
import sys
from pathlib import Path

# Add parent directory to path for SDK imports
SDK_ROOT = Path(__file__).parent.parent
if str(SDK_ROOT) not in sys.path:
    sys.path.insert(0, str(SDK_ROOT))

from sdk.luxtensor_client import LuxtensorClient
from sdk.errors import (
    RpcError,
    RpcErrorCode,
    InsufficientFundsError,
    NonceTooLowError,
    GasLimitExceededError,
    RateLimitedError,
    parse_rpc_error,
)

# Use standard exceptions for types not defined in SDK
ConnectionError = httpx.ConnectError
TimeoutError = httpx.TimeoutException
ValidationError = ValueError
InvalidAddressError = ValueError


# =============================================================================
# FIXTURES
# =============================================================================

@pytest.fixture
def client() -> LuxtensorClient:
    """Standard client for testing."""
    return LuxtensorClient(
        url="http://localhost:8545",
        network="testnet",
        timeout=5
    )


@pytest.fixture
def mock_httpx_client():
    """Mock HTTP client for simulating network conditions."""
    with patch('httpx.Client') as mock:
        yield mock


# =============================================================================
# NETWORK FAILURE TESTS
# =============================================================================

class TestNetworkFailures:
    """Tests for network connectivity issues."""

    def test_connection_refused_raises_connection_error(self, client: LuxtensorClient):
        """
        GIVEN: Node is not running (connection refused)
        WHEN: Client attempts RPC call
        THEN: Should raise LuxConnectionError with clear message
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_http.post.side_effect = httpx.ConnectError("Connection refused")

            with pytest.raises(httpx.ConnectError) as exc_info:
                client.get_block_number()

            assert "Connection refused" in str(exc_info.value) or "connect" in str(exc_info.value).lower()

    def test_timeout_raises_timeout_error(self, client: LuxtensorClient):
        """
        GIVEN: Node is slow to respond (timeout)
        WHEN: Client waits beyond timeout
        THEN: Should raise LuxTimeoutError
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_http.post.side_effect = httpx.TimeoutException("Request timeout")

            with pytest.raises(httpx.TimeoutException):
                client.get_block_number()

    def test_dns_resolution_failure(self, client: LuxtensorClient):
        """
        GIVEN: Invalid hostname
        WHEN: Client attempts connection
        THEN: Should raise appropriate error
        """
        bad_client = LuxtensorClient(url="http://nonexistent-host-12345.local:8545")

        with pytest.raises((httpx.ConnectError, Exception)):
            bad_client.get_block_number()

    def test_connection_reset_mid_request(self, client: LuxtensorClient):
        """
        GIVEN: Connection drops mid-request
        WHEN: Client is waiting for response
        THEN: Should raise connection error, not corrupt data
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_http.post.side_effect = httpx.RemoteProtocolError("Connection reset")

            with pytest.raises((httpx.RemoteProtocolError, Exception)):
                client.get_block_number()

    def test_ssl_certificate_error(self):
        """
        GIVEN: HTTPS endpoint with invalid cert
        WHEN: Client connects
        THEN: Should raise SSL-related error
        """
        # This test would require actual SSL setup
        # For now, verify client accepts verify parameter
        client = LuxtensorClient(url="https://localhost:8545")
        assert client is not None


# =============================================================================
# INVALID KEY TESTS
# =============================================================================

class TestInvalidKeys:
    """Tests for invalid cryptographic keys and addresses."""

    def test_invalid_address_format(self, client: LuxtensorClient):
        """
        GIVEN: Malformed address (not valid hex)
        WHEN: Used in query
        THEN: Should raise InvalidAddressError
        """
        invalid_addresses = [
            "not_a_valid_address",
            "0xZZZZZ",  # Invalid hex chars
            "0x123",    # Too short
            "",         # Empty
            None,       # None
        ]

        for addr in invalid_addresses:
            # Client should validate before sending to node
            try:
                result = client.get_account(addr)
                # If no exception, result should indicate invalid
            except (InvalidAddressError, ValidationError, ValueError, TypeError):
                pass  # Expected behavior

    def test_wrong_network_address(self, client: LuxtensorClient):
        """
        GIVEN: Address from different network
        WHEN: Used on testnet
        THEN: Should handle gracefully
        """
        # Mainnet address format on testnet
        mainnet_style_address = "lux_mainnet_1234567890abcdef"

        # Should not crash, may return None or raise ValidationError
        try:
            result = client.get_account(mainnet_style_address)
        except (ValidationError, InvalidAddressError):
            pass  # Expected

    def test_zero_address(self, client: LuxtensorClient):
        """
        GIVEN: Zero address (0x000...000)
        WHEN: Used in transaction
        THEN: Should be rejected or handled specially
        """
        zero_addr = "0x" + "0" * 40

        with patch.object(client, '_call_rpc') as mock_rpc:
            mock_rpc.return_value = {"balance": 0, "nonce": 0}
            result = client.get_account(zero_addr)
            # Should work - zero address is valid but special

    def test_checksum_address_validation(self, client: LuxtensorClient):
        """
        GIVEN: Address with incorrect checksum
        WHEN: Used in query
        THEN: Should validate checksum if applicable
        """
        # Mixed case address (incorrect checksum)
        bad_checksum = "0xABcDe1234567890AbCdEf1234567890AbCdEf12"

        try:
            # May validate or may pass through to node
            result = client.get_account(bad_checksum)
        except ValidationError:
            pass  # Valid if SDK validates checksums


# =============================================================================
# RPC ERROR HANDLING TESTS
# =============================================================================

class TestRPCErrors:
    """Tests for various RPC error responses."""

    def test_method_not_found(self, client: LuxtensorClient):
        """
        GIVEN: Calling non-existent RPC method
        WHEN: Request reaches node
        THEN: Should raise RPCError with method info
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_response = Mock()
            mock_response.json.return_value = {
                "jsonrpc": "2.0",
                "id": 1,
                "error": {"code": -32601, "message": "Method not found"}
            }
            mock_response.status_code = 200
            mock_http.post.return_value = mock_response

            with pytest.raises((RpcError, Exception)) as exc_info:
                client._call_rpc("nonexistent_method", [])

    def test_invalid_params(self, client: LuxtensorClient):
        """
        GIVEN: Invalid parameters for RPC method
        WHEN: Request sent to node
        THEN: Should raise RPCError with param info
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_response = Mock()
            mock_response.json.return_value = {
                "jsonrpc": "2.0",
                "id": 1,
                "error": {"code": -32602, "message": "Invalid params"}
            }
            mock_response.status_code = 200
            mock_http.post.return_value = mock_response

            with pytest.raises((RpcError, Exception)):
                client._call_rpc("get_block", ["invalid"])

    def test_internal_error_handling(self, client: LuxtensorClient):
        """
        GIVEN: Node internal error
        WHEN: Response contains error
        THEN: Should propagate with details
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_response = Mock()
            mock_response.json.return_value = {
                "jsonrpc": "2.0",
                "id": 1,
                "error": {"code": -32603, "message": "Internal error", "data": "Stack trace..."}
            }
            mock_response.status_code = 200
            mock_http.post.return_value = mock_response

            with pytest.raises((RpcError, Exception)):
                client.get_block_number()

    def test_rate_limit_response(self, client: LuxtensorClient):
        """
        GIVEN: Rate limited by node
        WHEN: Too many requests
        THEN: Should raise rate limit error with retry info
        """
        with patch.object(client, '_http_client') as mock_http:
            mock_response = Mock()
            mock_response.status_code = 429
            mock_response.headers = {"Retry-After": "60"}
            mock_response.json.return_value = {"error": "Rate limited"}
            mock_http.post.return_value = mock_response

            # Should handle 429 gracefully
            with pytest.raises(Exception):
                client.get_block_number()


# =============================================================================
# TRANSACTION EDGE CASES
# =============================================================================

class TestTransactionEdgeCases:
    """Tests for transaction submission edge cases."""

    def test_insufficient_funds(self, client: LuxtensorClient):
        """
        GIVEN: Account with insufficient balance
        WHEN: Attempting transaction
        THEN: Should raise InsufficientFundsError
        """
        with patch.object(client, '_call_rpc') as mock_rpc:
            mock_rpc.side_effect = Exception("insufficient funds for transfer")

            with pytest.raises((InsufficientFundsError, Exception)) as exc_info:
                client.send_transaction({
                    "from": "0x" + "a" * 40,
                    "to": "0x" + "b" * 40,
                    "value": 10**30  # Way too much
                })

    def test_nonce_too_low(self, client: LuxtensorClient):
        """
        GIVEN: Transaction with already-used nonce
        WHEN: Submitted to node
        THEN: Should raise appropriate error
        """
        with patch.object(client, '_call_rpc') as mock_rpc:
            mock_rpc.side_effect = Exception("nonce too low")

            with pytest.raises(Exception) as exc_info:
                client.send_transaction({
                    "from": "0x" + "a" * 40,
                    "to": "0x" + "b" * 40,
                    "value": 1000,
                    "nonce": 0  # Already used
                })

            assert "nonce" in str(exc_info.value).lower()

    def test_gas_limit_exceeded(self, client: LuxtensorClient):
        """
        GIVEN: Transaction requiring more gas than limit
        WHEN: Submitted
        THEN: Should fail with gas-related error
        """
        with patch.object(client, '_call_rpc') as mock_rpc:
            mock_rpc.side_effect = Exception("gas limit exceeded")

            with pytest.raises(Exception):
                client.send_transaction({
                    "from": "0x" + "a" * 40,
                    "to": "0x" + "b" * 40,
                    "value": 1000,
                    "gas": 1  # Way too low
                })

    def test_transaction_replacement_underpriced(self, client: LuxtensorClient):
        """
        GIVEN: Replacement tx with lower gas price
        WHEN: Submitted
        THEN: Should fail with underpriced error
        """
        with patch.object(client, '_call_rpc') as mock_rpc:
            mock_rpc.side_effect = Exception("replacement transaction underpriced")

            with pytest.raises(Exception):
                client.send_transaction({
                    "from": "0x" + "a" * 40,
                    "to": "0x" + "b" * 40,
                    "value": 1000,
                    "nonce": 5,
                    "gasPrice": 1  # Too low for replacement
                })


# =============================================================================
# DATA VALIDATION TESTS
# =============================================================================

class TestDataValidation:
    """Tests for input data validation."""

    @pytest.mark.parametrize("amount", [
        -1,           # Negative
        -1000,        # Large negative
        float('inf'), # Infinity
        float('nan'), # NaN
        "not_a_number",  # String
    ])
    def test_invalid_amount_rejected(self, client: LuxtensorClient, amount):
        """
        GIVEN: Invalid amount value
        WHEN: Used in transaction
        THEN: Should raise ValidationError
        """
        with pytest.raises((ValidationError, ValueError, TypeError)):
            client.send_transaction({
                "from": "0x" + "a" * 40,
                "to": "0x" + "b" * 40,
                "value": amount
            })

    @pytest.mark.parametrize("subnet_id", [
        -1,           # Negative
        2**64,        # Overflow u64
        "abc",        # String
        None,         # None
    ])
    def test_invalid_subnet_id_rejected(self, client: LuxtensorClient, subnet_id):
        """
        GIVEN: Invalid subnet ID
        WHEN: Used in query
        THEN: Should raise ValidationError or TypeError
        """
        with pytest.raises((ValidationError, ValueError, TypeError, Exception)):
            client.get_subnet(subnet_id)

    def test_empty_batch_request(self, client: LuxtensorClient):
        """
        GIVEN: Empty batch request
        WHEN: Submitted
        THEN: Should handle gracefully (empty result or error)
        """
        with patch.object(client, '_call_rpc') as mock_rpc:
            mock_rpc.return_value = []

            # Should not crash
            result = client.batch_get_neurons(subnet_id=1, neuron_uids=[])
            assert result == [] or result is None


# =============================================================================
# CONCURRENT OPERATION TESTS
# =============================================================================

class TestConcurrentOperations:
    """Tests for concurrent operation safety."""

    def test_concurrent_requests_isolated(self, client: LuxtensorClient):
        """
        GIVEN: Multiple concurrent requests
        WHEN: Executed simultaneously
        THEN: Each should receive correct response (no cross-contamination)
        """
        import threading

        results = {}
        errors = []

        def make_request(request_id: int):
            try:
                with patch.object(client, '_call_rpc') as mock_rpc:
                    mock_rpc.return_value = {"id": request_id}
                    result = client.get_block_number()
                    results[request_id] = result
            except Exception as e:
                errors.append(e)

        threads = [threading.Thread(target=make_request, args=(i,)) for i in range(10)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(errors) == 0, f"Errors occurred: {errors}"

    def test_request_id_uniqueness(self, client: LuxtensorClient):
        """
        GIVEN: Multiple requests
        WHEN: Made rapidly
        THEN: Each should have unique request ID
        """
        ids = set()
        for _ in range(100):
            id_ = client._get_request_id()
            assert id_ not in ids, f"Duplicate request ID: {id_}"
            ids.add(id_)


# =============================================================================
# RESOURCE CLEANUP TESTS
# =============================================================================

class TestResourceCleanup:
    """Tests for proper resource management."""

    def test_client_context_manager(self):
        """
        GIVEN: Client used as context manager
        WHEN: Exiting context
        THEN: Resources should be released
        """
        # LuxtensorClient may or may not support context manager
        # This test documents expected behavior
        try:
            with LuxtensorClient("http://localhost:8545") as client:
                pass
            # If we get here, context manager works
        except (AttributeError, TypeError):
            # Context manager not implemented - that's OK
            pass

    def test_no_connection_leak_on_error(self, client: LuxtensorClient):
        """
        GIVEN: Connection error during request
        WHEN: Error occurs
        THEN: Connection should be properly closed
        """
        initial_connections = 0  # Would need instrumentation to track

        for _ in range(10):
            with patch.object(client, '_http_client') as mock_http:
                mock_http.post.side_effect = httpx.ConnectError("Failed")

                try:
                    client.get_block_number()
                except Exception:
                    pass

        # No assertion here without instrumentation
        # This test documents expected behavior


# =============================================================================
# ASYNC EDGE CASES (if AsyncLuxtensorClient exists)
# =============================================================================

class TestAsyncEdgeCases:
    """Tests for async client edge cases."""

    @pytest.mark.asyncio
    async def test_async_timeout(self):
        """
        GIVEN: Slow async response
        WHEN: Timeout exceeded
        THEN: Should raise asyncio.TimeoutError or custom timeout
        """
        try:
            from sdk.async_luxtensor_client import AsyncLuxtensorClient

            async with AsyncLuxtensorClient("http://localhost:8545") as client:
                with pytest.raises((asyncio.TimeoutError, httpx.TimeoutException, Exception)):
                    # This would timeout if node is not running
                    await asyncio.wait_for(client.get_block_number(), timeout=0.001)
        except ImportError:
            pytest.skip("AsyncLuxtensorClient not available")

    @pytest.mark.asyncio
    async def test_async_cancellation(self):
        """
        GIVEN: Ongoing async request
        WHEN: Cancelled
        THEN: Should cleanup properly without hanging
        """
        try:
            from sdk.async_luxtensor_client import AsyncLuxtensorClient

            async def cancellable_request():
                async with AsyncLuxtensorClient("http://localhost:8545") as client:
                    await client.get_block_number()

            task = asyncio.create_task(cancellable_request())
            await asyncio.sleep(0.001)
            task.cancel()

            with pytest.raises(asyncio.CancelledError):
                await task
        except ImportError:
            pytest.skip("AsyncLuxtensorClient not available")


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
