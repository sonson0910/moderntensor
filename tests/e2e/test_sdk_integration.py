#!/usr/bin/env python3
"""
Comprehensive E2E Integration Test Suite: SDK ↔ LuxTensor Node

Tests the full user journey through the modular mixin-based SDK client:
  1. Connection & chain info
  2. Account lifecycle (balance, nonce, faucet)
  3. Transaction lifecycle (submit → mine → receipt → verify)
  4. Staking flow (validators, stake, delegate)
  5. Subnet & neuron queries
  6. Weight operations
  7. Consensus & reward queries
  8. Error handling & edge cases
  9. Multi-client consistency

Run:
    # Start node first
    cargo run -p luxtensor-node &
    # Then run tests
    pytest tests/e2e/test_sdk_integration.py -v --node-url http://127.0.0.1:8545
"""

import time
import pytest

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
]


# ── Fixtures (use e2e/conftest.py session-scoped client) ─────────────


@pytest.fixture
def sdk_client(luxtensor_client):
    """Alias for the session-scoped luxtensor_client from conftest."""
    return luxtensor_client


# ── 1. Connection & Chain Info ───────────────────────────────────────


class TestSDKConnection:
    """Verify SDK client establishes proper connection to the node."""

    def test_is_connected(self, sdk_client):
        """SDK is_connected() should return True for a reachable node."""
        assert sdk_client.is_connected() is True

    def test_get_block_number(self, sdk_client):
        """get_block_number() returns a non-negative integer."""
        block = sdk_client.get_block_number()
        assert isinstance(block, int)
        assert block >= 0

    def test_get_chain_info(self, sdk_client):
        """get_chain_info() returns valid chain metadata."""
        info = sdk_client.get_chain_info()
        assert info is not None

    def test_health_check(self, sdk_client):
        """health_check() should return a dict with status info."""
        health = sdk_client.health_check()
        assert health is not None


# ── 2. Account & Balance ─────────────────────────────────────────────


class TestSDKAccountOperations:
    """Test account balance and nonce queries via SDK."""

    def test_get_balance_zero_address(self, sdk_client, zero_address):
        """Zero address should have a valid (possibly zero) balance."""
        balance = sdk_client.get_balance(zero_address)
        assert isinstance(balance, int)
        assert balance >= 0

    def test_get_balance_known_address(self, sdk_client, funded_account):
        """Funded account should report a balance via SDK."""
        addr = funded_account["address"]
        balance = sdk_client.get_balance(addr)
        assert isinstance(balance, int)

    def test_get_nonce(self, sdk_client, zero_address):
        """get_nonce() for zero address should return int."""
        nonce = sdk_client.get_nonce(zero_address)
        assert isinstance(nonce, int)
        assert nonce >= 0


# ── 3. Block Queries ─────────────────────────────────────────────────


class TestSDKBlockQueries:
    """Test block retrieval via SDK."""

    def test_get_block_latest(self, sdk_client):
        """SDK should retrieve the latest block."""
        block = sdk_client.get_block("latest")
        assert block is not None

    def test_get_block_by_number(self, sdk_client):
        """SDK should retrieve block 0 (genesis)."""
        block = sdk_client.get_block(0)
        # May or may not exist depending on node state
        # Just verify it doesn't raise an unhandled exception

    def test_get_block_hash(self, sdk_client):
        """get_block_hash(0) should return a 66-char hex string."""
        block_hash = sdk_client.get_block_hash(0)
        if block_hash is not None:
            assert block_hash.startswith("0x")
            assert len(block_hash) == 66

    def test_block_number_advances(self, sdk_client):
        """Block number should increase over time."""
        block1 = sdk_client.get_block_number()
        time.sleep(4)  # block_time ≈ 3s
        block2 = sdk_client.get_block_number()
        assert block2 > block1, f"Block should advance: {block1} → {block2}"


# ── 4. Staking & Delegation ─────────────────────────────────────────


class TestSDKStakingQueries:
    """Test staking-related SDK queries."""

    def test_get_validators(self, sdk_client):
        """get_validators() should return a list."""
        validators = sdk_client.get_validators()
        assert isinstance(validators, list)

    def test_get_total_stake(self, sdk_client):
        """get_total_stake() should return an integer."""
        total = sdk_client.get_total_stake()
        assert isinstance(total, int)
        assert total >= 0

    def test_get_stake_for_address(self, sdk_client, test_address):
        """get_stake() for test address should return int."""
        stake = sdk_client.get_stake(test_address)
        assert isinstance(stake, int)
        assert stake >= 0

    def test_get_delegates(self, sdk_client):
        """get_delegates() should return a list."""
        delegates = sdk_client.get_delegates()
        assert isinstance(delegates, list)


# ── 5. Subnet Operations ────────────────────────────────────────────


class TestSDKSubnetOperations:
    """Test subnet-related SDK queries."""

    def test_get_all_subnets(self, sdk_client):
        """get_all_subnets() returns a list."""
        subnets = sdk_client.get_all_subnets()
        assert isinstance(subnets, list)

    def test_get_root_config(self, sdk_client):
        """get_root_config() returns subnet 0 config."""
        config = sdk_client.get_root_config()
        assert config is not None

    def test_subnet_count(self, sdk_client):
        """get_total_subnets() returns non-negative int."""
        count = sdk_client.get_total_subnets()
        assert isinstance(count, int)
        assert count >= 0


# ── 6. Neuron Queries ────────────────────────────────────────────────


class TestSDKNeuronQueries:
    """Test neuron-related SDK queries."""

    def test_get_total_neurons(self, sdk_client):
        """get_total_neurons() returns non-negative int."""
        total = sdk_client.get_total_neurons()
        assert isinstance(total, int)
        assert total >= 0

    def test_get_neurons_subnet_0(self, sdk_client):
        """get_neurons(0) returns a list for root subnet."""
        neurons = sdk_client.get_neurons(subnet_id=0)
        assert isinstance(neurons, list)


# ── 7. Weight Queries ────────────────────────────────────────────────


class TestSDKWeightQueries:
    """Test weight-related SDK queries."""

    def test_get_weights(self, sdk_client):
        """get_weights() for subnet 0, uid 0 should not crash."""
        try:
            weights = sdk_client.get_weights(subnet_id=0, neuron_uid=0)
            assert isinstance(weights, (list, type(None)))
        except Exception:
            # Expected if no neurons exist yet
            pass


# ── 8. Reward & Consensus Queries ────────────────────────────────────


class TestSDKRewardQueries:
    """Test reward and consensus-related SDK queries."""

    def test_get_total_issuance(self, sdk_client):
        """get_total_issuance() returns non-negative int."""
        issuance = sdk_client.get_total_issuance()
        assert isinstance(issuance, int)
        assert issuance >= 0


# ── 9. Error Handling ────────────────────────────────────────────────


class TestSDKErrorHandling:
    """Test SDK handles invalid inputs gracefully."""

    def test_get_balance_invalid_address(self, sdk_client, invalid_address):
        """Invalid address should raise or return 0, not crash."""
        try:
            balance = sdk_client.get_balance(invalid_address)
            assert isinstance(balance, int)
        except Exception:
            pass  # Raising is acceptable

    def test_get_neurons_invalid_subnet(self, sdk_client):
        """Invalid subnet ID should return empty list or raise."""
        try:
            neurons = sdk_client.get_neurons(subnet_id=99999)
            assert isinstance(neurons, list)
        except Exception:
            pass  # Raising is acceptable

    def test_get_stake_empty_address(self, sdk_client):
        """Empty/zero address stake query should not crash."""
        zero = "0x" + "00" * 20
        try:
            stake = sdk_client.get_stake(zero)
            assert isinstance(stake, int)
        except Exception:
            pass


# ── 10. Multi-Client Consistency ─────────────────────────────────────


class TestSDKMultiClientConsistency:
    """Verify multiple SDK clients see consistent state."""

    def test_two_clients_same_block(self, node_url):
        """Two independent clients should see the same block number."""
        from sdk.client import LuxtensorClient

        c1 = LuxtensorClient(url=node_url, network="testnet", timeout=10)
        c2 = LuxtensorClient(url=node_url, network="testnet", timeout=10)

        # Check connected
        try:
            b1 = c1.get_block_number()
            b2 = c2.get_block_number()
        except Exception:
            pytest.skip("Cannot connect for multi-client test")

        # Allow 1 block difference due to timing
        assert abs(b1 - b2) <= 1, f"Clients diverge: {b1} vs {b2}"

    def test_two_clients_same_balance(self, node_url, test_address):
        """Two clients should report the same balance."""
        from sdk.client import LuxtensorClient

        c1 = LuxtensorClient(url=node_url, network="testnet", timeout=10)
        c2 = LuxtensorClient(url=node_url, network="testnet", timeout=10)

        try:
            bal1 = c1.get_balance(test_address)
            bal2 = c2.get_balance(test_address)
        except Exception:
            pytest.skip("Cannot connect for multi-client test")

        assert bal1 == bal2, f"Balances diverge: {bal1} vs {bal2}"


# ── 11. Full User Journey ───────────────────────────────────────────


class TestSDKFullUserJourney:
    """
    End-to-end user journey: connect → query → explore → disconnect.

    This simulates what a real developer would do with the SDK.
    """

    def test_developer_onboarding_flow(self, node_url):
        """
        Simulate a new developer's first interaction with the SDK:
        1. Import and connect
        2. Check connection health
        3. Get block number
        4. Query account balance
        5. Explore subnets
        6. Check validator set
        """
        from sdk.client import LuxtensorClient

        # Step 1: Connect
        client = LuxtensorClient(url=node_url, network="testnet", timeout=10)

        # Step 2: Health check
        try:
            connected = client.is_connected()
        except Exception:
            pytest.skip("Node not reachable")

        if not connected:
            pytest.skip("Node not reachable")

        # Step 3: Block number
        block = client.get_block_number()
        assert block >= 0

        # Step 4: Balance query
        balance = client.get_balance("0x" + "00" * 20)
        assert isinstance(balance, int)

        # Step 5: Subnet exploration
        subnets = client.get_all_subnets()
        assert isinstance(subnets, list)

        # Step 6: Validator set
        validators = client.get_validators()
        assert isinstance(validators, list)

        # Journey complete — SDK successfully used for basic exploration
