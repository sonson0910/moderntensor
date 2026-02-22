# tests/e2e/conftest.py
"""
E2E Integration Test Configuration — SDK ↔ Live LuxTensor Node

Provides:
- Session-scoped LuxtensorClient that auto-skips when node is offline
- CLI option --node-url to override default endpoint
- Pytest markers: e2e, live_node, slow, requires_funded_account
- Shared fixtures for addresses, funded accounts, and test data
"""

import time
import pytest
from typing import Optional

# ---------------------------------------------------------------------------
# CLI option
# ---------------------------------------------------------------------------

def pytest_addoption(parser):
    parser.addoption(
        "--node-url",
        action="store",
        default="http://localhost:8545",
        help="LuxTensor node RPC endpoint URL",
    )


# ---------------------------------------------------------------------------
# Markers
# ---------------------------------------------------------------------------

def pytest_configure(config):
    config.addinivalue_line("markers", "e2e: End-to-end integration test")
    config.addinivalue_line("markers", "live_node: Requires a running LuxTensor node")
    config.addinivalue_line("markers", "slow: Slow test (block waiting, etc.)")
    config.addinivalue_line(
        "markers",
        "requires_funded_account: Requires a funded account for state-changing ops",
    )


# ---------------------------------------------------------------------------
# Test addresses
# ---------------------------------------------------------------------------

TEST_VALIDATOR_ADDRESS = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"
TEST_COLDKEY_ADDRESS = "0x892d35Cc6634C0532925a3b844Bc9e7595f0cCd3"
TEST_HOTKEY_ADDRESS = "0xa52d35Cc6634C0532925a3b844Bc9e7595f0dDe4"
TEST_ZERO_ADDRESS = "0x" + "00" * 20
TEST_INVALID_ADDRESS = "not_a_valid_address"


# ---------------------------------------------------------------------------
# Session-scoped client — shared across all E2E tests
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def node_url(request) -> str:
    """RPC endpoint URL from CLI or default."""
    return request.config.getoption("--node-url")


@pytest.fixture(scope="session")
def luxtensor_client(node_url):
    """
    Create a session-scoped LuxtensorClient.

    Automatically skips the ENTIRE test session if the node is unreachable.
    This prevents noisy failures when running tests without a live node.
    """
    from sdk.luxtensor_client import LuxtensorClient

    client = LuxtensorClient(url=node_url, network="testnet", timeout=10)

    # Gate: skip all tests if node is offline
    try:
        connected = client.is_connected()
        if not connected:
            pytest.skip(f"LuxTensor node not reachable at {node_url}")
    except Exception as exc:
        pytest.skip(f"Cannot connect to LuxTensor node at {node_url}: {exc}")

    return client


# ---------------------------------------------------------------------------
# Convenience fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def test_address() -> str:
    return TEST_VALIDATOR_ADDRESS


@pytest.fixture
def zero_address() -> str:
    return TEST_ZERO_ADDRESS


@pytest.fixture
def invalid_address() -> str:
    return TEST_INVALID_ADDRESS


@pytest.fixture
def coldkey() -> str:
    return TEST_COLDKEY_ADDRESS


@pytest.fixture
def hotkey() -> str:
    return TEST_HOTKEY_ADDRESS


# ---------------------------------------------------------------------------
# Funded account fixture (for state-changing tests)
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def funded_account(luxtensor_client):
    """
    Provide a funded account for state-changing tests.

    Uses the first validator address returned by the node as the funded account.
    Falls back to TEST_VALIDATOR_ADDRESS when no validators are available,
    so that tests can still verify SDK error handling for empty-state scenarios.
    """
    try:
        validators = luxtensor_client.get_validators()
        if not validators:
            # No validators — use test address so tests verify empty-state handling
            return {"address": TEST_VALIDATOR_ADDRESS, "validators": []}

        # Use first validator as funded account
        addr = None
        if isinstance(validators, list) and len(validators) > 0:
            v = validators[0]
            if isinstance(v, dict):
                addr = v.get("address") or v.get("validator") or v.get("hotkey")
            elif isinstance(v, str):
                addr = v

        if not addr:
            return {"address": TEST_VALIDATOR_ADDRESS, "validators": validators}

        return {"address": addr, "validators": validators}
    except Exception:
        return {"address": TEST_VALIDATOR_ADDRESS, "validators": []}


# ---------------------------------------------------------------------------
# Helper: wait for next block
# ---------------------------------------------------------------------------

@pytest.fixture
def wait_next_block(luxtensor_client):
    """Factory fixture that waits for the next block to be produced."""
    def _wait(timeout: int = 30):
        start_block = luxtensor_client.get_block_number()
        deadline = time.time() + timeout
        while time.time() < deadline:
            current = luxtensor_client.get_block_number()
            if current > start_block:
                return current
            time.sleep(1)
        pytest.fail(f"No new block produced within {timeout}s")
    return _wait
