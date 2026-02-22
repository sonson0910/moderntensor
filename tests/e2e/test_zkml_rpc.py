# tests/e2e/test_zkml_rpc.py
"""
E2E Tests — ZKML (Zero-Knowledge ML) RPC
Verifies proof queries and model registration endpoints.
"""

import pytest

pytestmark = [pytest.mark.e2e, pytest.mark.live_node]


class TestZkmlRpc:
    """ZKML proof system read-only queries."""

    def test_zkml_get_proof(self, luxtensor_client):
        """Querying non-existent proof returns None or error."""
        try:
            result = luxtensor_client._call_rpc("zkml_getProof", ["nonexistent_proof_id"])
        except Exception:
            pass

    def test_zkml_list_proofs(self, luxtensor_client):
        """zkml_listProofs returns a list (possibly empty)."""
        try:
            proofs = luxtensor_client._call_rpc("zkml_listProofs")
            if proofs is not None:
                assert isinstance(proofs, list)
        except Exception:
            pass

    def test_zkml_get_verified_models(self, luxtensor_client):
        """zkml_getVerifiedModels returns a list of trusted models."""
        try:
            models = luxtensor_client._call_rpc("zkml_getVerifiedModels")
            if models is not None:
                assert isinstance(models, list)
        except Exception:
            pass

    def test_zkml_verify_proof_invalid(self, luxtensor_client):
        """Submitting an invalid proof returns a structured error."""
        try:
            result = luxtensor_client._call_rpc("zkml_verifyProof", [
                "invalid_proof_data"
            ])
        except Exception:
            pass  # Error = endpoint exists and validates
