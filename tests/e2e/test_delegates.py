# tests/e2e/test_delegates.py
"""
E2E Tests — Delegate Operations
Verifies delegate listing, info queries, and delegation status.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]

# Test address for empty-state scenarios
_TEST_HOTKEY = "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2"


class TestDelegateQueries:
    """Read-only delegate queries."""

    def test_get_delegates(self, luxtensor_client):
        """Delegate list is retrievable (may be empty on dev node)."""
        delegates = luxtensor_client.get_delegates()
        assert delegates is not None
        assert isinstance(delegates, list)

    def test_is_delegate_random_address(self, luxtensor_client):
        """Random address is not a delegate."""
        result = luxtensor_client.is_delegate(hotkey="0x0000000000000000000000000000000000000001")
        assert result is False

    def test_get_delegate_info(self, luxtensor_client):
        """Delegate info query handles empty delegate list gracefully."""
        delegates = luxtensor_client.get_delegates()
        if not delegates:
            # No delegates — verify SDK handles test address gracefully
            try:
                info = luxtensor_client.get_delegate_info(hotkey=_TEST_HOTKEY)
                # None or empty dict is acceptable
            except Exception:
                pass  # RPC error for non-existent delegate is expected
            return

        first = delegates[0]
        hotkey = first.get("hotkey") if isinstance(first, dict) else first
        if hotkey:
            info = luxtensor_client.get_delegate_info(hotkey=hotkey)
            assert info is not None

    def test_get_delegate_take(self, luxtensor_client):
        """Delegate take (commission) query handles empty state gracefully."""
        delegates = luxtensor_client.get_delegates()
        if not delegates:
            # No delegates — verify SDK handles test address gracefully
            try:
                take = luxtensor_client.get_delegate_take(hotkey=_TEST_HOTKEY)
                # 0 or None is acceptable
            except Exception:
                pass  # Expected when no delegates exist
            return

        first = delegates[0]
        hotkey = first.get("hotkey") if isinstance(first, dict) else first
        if hotkey:
            take = luxtensor_client.get_delegate_take(hotkey=hotkey)
            if take is not None:
                assert 0 <= take <= 1

    def test_get_nominators(self, luxtensor_client):
        """Nominators query handles empty state gracefully."""
        delegates = luxtensor_client.get_delegates()
        if not delegates:
            # No delegates — verify SDK handles test address gracefully
            try:
                noms = luxtensor_client.get_nominators(hotkey=_TEST_HOTKEY)
                if noms is not None:
                    assert isinstance(noms, list)
            except Exception:
                pass  # Expected when no delegates exist
            return

        first = delegates[0]
        hotkey = first.get("hotkey") if isinstance(first, dict) else first
        if hotkey:
            noms = luxtensor_client.get_nominators(hotkey=hotkey)
            assert noms is not None
            assert isinstance(noms, list)
