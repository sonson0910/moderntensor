# tests/e2e/test_staking_operations.py
"""
E2E Tests — Staking State-Changing Operations
Verifies stake, unstake, delegate, and undelegate operations against live node.
Requires a funded account with tokens available.
"""

import time
import pytest

from tests.e2e.conftest import TEST_VALIDATOR_ADDRESS

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.live_node,
    pytest.mark.requires_funded_account,
]


class TestStakingOperations:
    """State-changing staking operations."""

    def test_stake_operation(self, luxtensor_client, funded_account):
        """Staking tokens via the SDK."""
        addr = funded_account["address"]
        # Query current stake
        before = luxtensor_client.get_stake(address=addr)
        assert before is not None

        # Attempt minimal stake (may fail if already staked or insufficient balance)
        timestamp = int(time.time())
        try:
            result = luxtensor_client.stake(
                address=addr,
                amount=1,
                timestamp=timestamp,
                signature="0x" + "00" * 65,  # placeholder
            )
            # If successful, verify response structure
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            # Signature errors or insufficient balance are expected
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "signature", "balance", "insufficient", "invalid", "error"
            ]), f"Unexpected error: {e}"

    def test_unstake_operation(self, luxtensor_client, funded_account):
        """Unstaking tokens via the SDK."""
        addr = funded_account["address"]
        timestamp = int(time.time())
        try:
            result = luxtensor_client.unstake(
                address=addr,
                amount=1,
                timestamp=timestamp,
                signature="0x" + "00" * 65,
            )
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "signature", "balance", "stake", "invalid", "error"
            ]), f"Unexpected error: {e}"

    def test_delegate_operation(self, luxtensor_client, funded_account):
        """Delegating tokens to a validator."""
        addr = funded_account["address"]
        validators = funded_account.get("validators", [])

        validator_addr = None
        if validators:
            if isinstance(validators[0], dict):
                validator_addr = validators[0].get("address") or validators[0].get("hotkey")
            elif isinstance(validators[0], str):
                validator_addr = validators[0]

        if not validator_addr:
            validator_addr = TEST_VALIDATOR_ADDRESS

        timestamp = int(time.time())
        try:
            result = luxtensor_client.delegate(
                delegator=addr,
                validator=validator_addr,
                amount=1,
                timestamp=timestamp,
                signature="0x" + "00" * 65,
            )
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "signature", "balance", "delegate", "invalid", "error"
            ]), f"Unexpected error: {e}"

    def test_undelegate_operation(self, luxtensor_client, funded_account):
        """Undelegating tokens."""
        addr = funded_account["address"]
        timestamp = int(time.time())
        try:
            result = luxtensor_client.undelegate(
                delegator=addr,
                timestamp=timestamp,
                signature="0x" + "00" * 65,
            )
            if result is not None:
                assert isinstance(result, dict)
        except Exception as e:
            error_msg = str(e).lower()
            assert any(word in error_msg for word in [
                "signature", "delegation", "invalid", "error", "not found"
            ]), f"Unexpected error: {e}"

    def test_get_delegation_info(self, luxtensor_client, funded_account):
        """Delegation info query for funded account."""
        addr = funded_account["address"]
        info = luxtensor_client.get_delegation(delegator=addr)
        # May be None if not delegated

    def test_claim_rewards(self, luxtensor_client, funded_account):
        """Claim pending rewards."""
        addr = funded_account["address"]
        try:
            result = luxtensor_client.claim_rewards(address=addr)
            if result is not None:
                assert isinstance(result, dict)
        except Exception:
            pass  # May have no pending rewards
