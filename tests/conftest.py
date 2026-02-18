# tests/conftest.py
"""
Test configuration and fixtures for ModernTensor SDK tests.

Provides shared fixtures for RPC client mocking, test addresses,
and common test data used across multiple test modules.
"""

import pytest
from unittest.mock import MagicMock, patch
from typing import Any, Dict, Optional, List


# =============================================================================
# TEST ADDRESSES
# =============================================================================

TEST_VALIDATOR_ADDRESS = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
TEST_COLDKEY_ADDRESS = "0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3"
TEST_HOTKEY_ADDRESS = "0xa52d35Cc6634C0532925a3b844Bc9e7595f0dDe4"
TEST_DELEGATOR_ADDRESS = "0xb62d35Cc6634C0532925a3b844Bc9e7595f0eEf5"
TEST_ZERO_ADDRESS = "0x" + "00" * 20


# =============================================================================
# FIXTURES
# =============================================================================


@pytest.fixture
def test_address() -> str:
    """Standard test address."""
    return TEST_VALIDATOR_ADDRESS


@pytest.fixture
def test_validator_address() -> str:
    """Validator address for staking tests."""
    return TEST_VALIDATOR_ADDRESS


@pytest.fixture
def test_coldkey() -> str:
    """Coldkey address for staking tests."""
    return TEST_COLDKEY_ADDRESS


@pytest.fixture
def test_hotkey() -> str:
    """Hotkey address for staking tests."""
    return TEST_HOTKEY_ADDRESS


@pytest.fixture
def mock_rpc_response():
    """
    Factory fixture for creating mock RPC responses.

    Usage:
        def test_something(mock_rpc_response):
            response = mock_rpc_response(result={"balance": "0x100"})
            # response == {"jsonrpc": "2.0", "id": 1, "result": {"balance": "0x100"}}
    """
    def _make_response(
        result: Any = None,
        error: Optional[Dict[str, Any]] = None,
        request_id: int = 1,
    ) -> Dict[str, Any]:
        response = {"jsonrpc": "2.0", "id": request_id}
        if error is not None:
            response["error"] = error
        else:
            response["result"] = result
        return response

    return _make_response


@pytest.fixture
def mock_rpc_client():
    """
    Create a mock RPC client with configurable responses.

    Usage:
        def test_get_stake(mock_rpc_client):
            client = mock_rpc_client({"staking_getStake": "0x1000"})
            stake = client.get_stake("0x...")
            assert stake == 4096
    """
    def _make_client(responses: Optional[Dict[str, Any]] = None):
        from sdk.client import LuxtensorClient

        responses = responses or {}
        client = LuxtensorClient.__new__(LuxtensorClient)

        # Initialize required attributes without real HTTP
        client.url = "http://mock:8545"
        client.network = "testnet"
        client.timeout = 5
        client._request_id = 0
        client._http_client = None
        client._delegates_cache = None
        client._delegates_cache_time = 0.0

        def mock_call_rpc(method: str, params: Optional[List[Any]] = None) -> Any:
            if method in responses:
                val = responses[method]
                if callable(val):
                    return val(params)
                return val
            return None

        client._call_rpc = mock_call_rpc
        client._safe_call_rpc = lambda m, p=None: mock_call_rpc(m, p)
        return client

    return _make_client


@pytest.fixture
def sample_delegates() -> List[Dict[str, Any]]:
    """Sample delegate data for testing delegate-related methods."""
    return [
        {
            "hotkey": TEST_HOTKEY_ADDRESS,
            "address": TEST_HOTKEY_ADDRESS,
            "take": 0.18,
            "commission": 0.18,
            "nominators": [TEST_DELEGATOR_ADDRESS],
            "total_stake": 1000000,
        },
        {
            "hotkey": "0xdef456789012345678901234567890abcdef1234",
            "address": "0xdef456789012345678901234567890abcdef1234",
            "take": 0.10,
            "commission": 0.10,
            "nominators": [],
            "total_stake": 500000,
        },
    ]
