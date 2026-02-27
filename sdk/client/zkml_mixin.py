"""
Zero-Knowledge Machine Learning (zkML) Mixin for LuxtensorClient

Wraps L1 zkml_* RPC methods for proof submission, verification,
proof generation, and trusted model management.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class ZkmlMixin:
    """
    Mixin providing zero-knowledge ML (zkML) methods.

    Methods:
        Proof Operations:
            - zkml_submit_proof()         — Submit a zk proof for verification
            - zkml_get_proof()            — Retrieve a proof by ID
            - zkml_verify_proof()         — Verify a proof synchronously
            - zkml_generate_proof()       — Request on-chain proof generation

        Model Management:
            - zkml_register_model()       — Register a model as trusted
            - zkml_list_trusted_models()  — List all trusted model hashes
            - zkml_is_model_trusted()     — Check if a model hash is trusted
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ---------------------------------------------------------------
    # Proof Operations
    # ---------------------------------------------------------------

    def zkml_submit_proof(
        self,
        model_hash: str,
        input_hash: str,
        output_hash: str,
        proof_data: str,
        submitter: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Submit a zero-knowledge proof for a model inference.

        Args:
            model_hash: Hash of the ML model (hex)
            input_hash: Hash of the input data (hex)
            output_hash: Hash of the output/result (hex)
            proof_data: Serialised proof bytes (hex)
            submitter: Optional submitter address (0x...)

        Returns:
            Dict with proof_id and initial verification status
        """
        try:
            params: Dict[str, Any] = {
                "model_hash": model_hash,
                "input_hash": input_hash,
                "output_hash": output_hash,
                "proof_data": proof_data,
            }
            if submitter:
                params["submitter"] = submitter
            return self._rpc()._call_rpc("zkml_submitProof", [params]) or {}
        except Exception as e:
            logger.error("Failed to submit zkML proof: %s", e)
            raise

    def zkml_get_proof(self, proof_id: str) -> Optional[Dict[str, Any]]:
        """
        Retrieve a previously submitted proof by ID.

        Args:
            proof_id: Proof ID returned by zkml_submit_proof

        Returns:
            Proof details dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("zkml_getProof", [proof_id])
        except Exception as e:
            logger.warning("Failed to get zkML proof %s: %s", proof_id, e)
            return None

    def zkml_verify_proof(self, proof_id: str) -> Dict[str, Any]:
        """
        Trigger synchronous verification of a submitted proof.

        Args:
            proof_id: Proof ID to verify

        Returns:
            Dict with valid (bool), verified_at, and verifier info
        """
        try:
            return self._rpc()._call_rpc("zkml_verifyProof", [proof_id]) or {}
        except Exception as e:
            logger.error("Failed to verify zkML proof %s: %s", proof_id, e)
            return {"valid": False, "error": str(e)}

    def zkml_generate_proof(
        self,
        model_hash: str,
        input_data: str,
        requester: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Request on-chain proof generation for a model inference.

        Args:
            model_hash: Model hash identifying the zkML model
            input_data: Hex-encoded input data for the model
            requester: Optional requester address (0x...)

        Returns:
            Dict with proof_id and generation status
        """
        try:
            params: Dict[str, Any] = {
                "model_hash": model_hash,
                "input_data": input_data,
            }
            if requester:
                params["requester"] = requester
            return self._rpc()._call_rpc("zkml_generateProof", [params]) or {}
        except Exception as e:
            logger.error("Failed to generate zkML proof: %s", e)
            raise

    # ---------------------------------------------------------------
    # Model Management
    # ---------------------------------------------------------------

    def zkml_register_model(
        self,
        model_hash: str,
        name: Optional[str] = None,
        description: Optional[str] = None,
    ) -> Dict[str, Any]:
        """
        Register a model hash as trusted in the zkML registry.

        Args:
            model_hash: Model hash to register (hex)
            name: Optional human-readable model name
            description: Optional description

        Returns:
            Registration result dict
        """
        try:
            params: Dict[str, Any] = {"model_hash": model_hash}
            if name:
                params["name"] = name
            if description:
                params["description"] = description
            return self._rpc()._call_rpc("zkml_registerModel", [params]) or {}
        except Exception as e:
            logger.error("Failed to register zkML model %s: %s", model_hash, e)
            raise

    def zkml_list_trusted_models(self) -> List[Dict[str, Any]]:
        """
        List all trusted model hashes in the registry.

        Returns:
            List of trusted model info dicts
        """
        try:
            result = self._rpc()._call_rpc("zkml_listTrustedModels", [])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to list trusted zkML models: %s", e)
            return []

    def zkml_is_model_trusted(self, model_hash: str) -> bool:
        """
        Check whether a model hash is registered as trusted.

        Args:
            model_hash: Model hash to check

        Returns:
            True if trusted, False otherwise
        """
        try:
            result = self._rpc()._call_rpc("zkml_isModelTrusted", [model_hash])
            if isinstance(result, dict):
                return bool(result.get("trusted", False))
            return bool(result)
        except Exception as e:
            logger.warning("Failed to check zkML model trust for %s: %s", model_hash, e)
            return False
