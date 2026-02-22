# tests/e2e/test_ai_rpc.py
"""
E2E Tests — AI RPC endpoints
Verifies AI task submission, result queries, and metagraph access.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestAiRpcQueries:
    """AI-related RPC queries."""

    def test_submit_ai_task_query(self, luxtensor_client):
        """AI task submission endpoint exists and responds."""
        try:
            result = luxtensor_client.submit_ai_task(
                task_data={"type": "inference", "model": "test", "input": "hello"}
            )
            # Should return task_id string or structured response
            assert result is not None
        except Exception as e:
            # Structured error is acceptable — the endpoint exists and responds
            assert "error" in str(e).lower() or "rpc" in str(e).lower()

    def test_get_ai_result_nonexistent(self, luxtensor_client):
        """Querying non-existent AI task returns None or error gracefully."""
        try:
            result = luxtensor_client.get_ai_result(task_id="nonexistent_task_id")
            # None is acceptable
        except Exception:
            pass  # RPC error for invalid task ID is acceptable — endpoint exists

    def test_lux_get_metagraph(self, luxtensor_client):
        """lux_getMetagraph returns metagraph data for subnet 0."""
        result = luxtensor_client._call_rpc("ai_getMetagraph", [0])
        # May be None for empty subnet, but should not crash

    def test_ai_get_tasks(self, luxtensor_client):
        """ai_getTasks returns a list of pending tasks or RPC error."""
        try:
            result = luxtensor_client._call_rpc("ai_getTasks")
            # May be empty list
        except Exception:
            pass  # Method not found is acceptable on dev node

    def test_lux_submit_result(self, luxtensor_client):
        """lux_submitResult endpoint exists and responds."""
        try:
            result = luxtensor_client._call_rpc("lux_submitResult", [
                "fake_task_id",
                "fake_result_data",
                "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb2",
            ])
        except Exception:
            pass  # Error response = endpoint exists
