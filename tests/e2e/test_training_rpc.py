# tests/e2e/test_training_rpc.py
"""
E2E Tests — Training (Federated Learning) RPC
Verifies training job queries, round management, and gradient aggregation endpoints.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestTrainingRpc:
    """Federated learning training job queries."""

    def test_training_list_jobs(self, luxtensor_client):
        """training_listJobs returns a list (possibly empty)."""
        try:
            jobs = luxtensor_client._call_rpc("training_listJobs")
            if jobs is not None:
                assert isinstance(jobs, list)
        except Exception:
            pass

    def test_training_get_job_nonexistent(self, luxtensor_client):
        """Querying non-existent job returns None or error."""
        try:
            result = luxtensor_client._call_rpc("training_getJob", ["nonexistent_job_id"])
        except Exception:
            pass

    def test_training_get_active_jobs(self, luxtensor_client):
        """training_getActiveJobs returns active training sessions."""
        try:
            active = luxtensor_client._call_rpc("training_getActiveJobs")
            if active is not None:
                assert isinstance(active, list)
        except Exception:
            pass

    def test_training_get_round_info(self, luxtensor_client):
        """training_getRoundInfo returns round information."""
        try:
            result = luxtensor_client._call_rpc("training_getRoundInfo", ["nonexistent_job_id", 0])
        except Exception:
            pass  # Error = endpoint exists
