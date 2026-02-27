"""
Federated Learning Training Mixin for LuxtensorClient

Wraps L1 training_* RPC methods for managing federated learning jobs,
trainer registration, and gradient submission.
"""

import logging
import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class TrainingMixin:
    """
    Mixin providing federated learning training methods.

    Methods:
        Job Management:
            - training_create_job()       — Create a new FL training job
            - training_get_job()          — Get job info by ID
            - training_list_jobs()        — List jobs (all or by owner)
            - training_cancel_job()       — Cancel a job (signed)

        Trainer Management:
            - training_register_trainer() — Register a trainer for a job

        Gradient Operations:
            - training_submit_gradient()  — Submit gradient update (signed)
            - training_get_gradients()    — Get gradients for a training round
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    # ---------------------------------------------------------------
    # Job Management
    # ---------------------------------------------------------------

    def training_create_job(
        self,
        owner: str,
        model_hash: str,
        signature: str,
        min_trainers: int = 3,
        max_rounds: int = 10,
        reward_per_round: str = "0",
        dataset_hash: Optional[str] = None,
        hyperparams: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """
        Create a new federated learning training job.

        Args:
            owner: Owner address (0x...)
            model_hash: Initial model hash (hex)
            signature: Secp256k1 signature over `training_createJob:{model_hash}:{timestamp}`
            min_trainers: Minimum number of trainers required
            max_rounds: Maximum training rounds
            reward_per_round: Reward per round (u128 as decimal string)
            dataset_hash: Optional dataset hash
            hyperparams: Optional hyperparameters dict

        Returns:
            Job info dict with job_id, status, etc.
        """
        try:
            timestamp = int(time.time())
            params = {
                "owner": owner,
                "model_hash": model_hash,
                "min_trainers": min_trainers,
                "max_rounds": max_rounds,
                "reward_per_round": reward_per_round,
                "signature": signature,
                "timestamp": timestamp,
            }
            if dataset_hash:
                params["dataset_hash"] = dataset_hash
            if hyperparams:
                params["hyperparams"] = hyperparams
            return self._rpc()._call_rpc("training_createJob", [params]) or {}
        except Exception as e:
            logger.error("Failed to create training job: %s", e)
            raise

    def training_get_job(self, job_id: str) -> Optional[Dict[str, Any]]:
        """
        Get training job details.

        Args:
            job_id: Job ID

        Returns:
            Job info dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("training_getJob", [job_id])
        except Exception as e:
            logger.warning("Failed to get training job %s: %s", job_id, e)
            return None

    def training_list_jobs(
        self, owner: Optional[str] = None
    ) -> List[Dict[str, Any]]:
        """
        List training jobs.

        Args:
            owner: Optional owner address to filter by

        Returns:
            List of job info dicts
        """
        try:
            params = [owner] if owner else []
            result = self._rpc()._call_rpc("training_listJobs", params)
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning("Failed to list training jobs: %s", e)
            return []

    def training_cancel_job(
        self, job_id: str, owner: str, signature: str
    ) -> Dict[str, Any]:
        """
        Cancel a training job (requires owner signature).

        Args:
            job_id: Job ID to cancel
            owner: Owner address (0x...)
            signature: Secp256k1 signature

        Returns:
            Result dict with success status
        """
        try:
            timestamp = int(time.time())
            params = {
                "job_id": job_id,
                "owner": owner,
                "signature": signature,
                "timestamp": timestamp,
            }
            return self._rpc()._call_rpc("training_cancelJob", [params]) or {}
        except Exception as e:
            logger.error("Failed to cancel training job %s: %s", job_id, e)
            return {"success": False, "error": str(e)}

    # ---------------------------------------------------------------
    # Trainer Management
    # ---------------------------------------------------------------

    def training_register_trainer(
        self, job_id: str, trainer: str
    ) -> Dict[str, Any]:
        """
        Register a trainer node for a training job.

        Args:
            job_id: Job ID
            trainer: Trainer address (0x...)

        Returns:
            Result dict with success and trainer count
        """
        try:
            params = {"job_id": job_id, "trainer": trainer}
            return self._rpc()._call_rpc("training_registerTrainer", [params]) or {}
        except Exception as e:
            logger.error("Failed to register trainer for job %s: %s", job_id, e)
            return {"success": False, "error": str(e)}

    # ---------------------------------------------------------------
    # Gradient Operations
    # ---------------------------------------------------------------

    def training_submit_gradient(
        self,
        job_id: str,
        trainer: str,
        round_number: int,
        gradient_hash: str,
        gradient_data: Optional[str],
        signature: str,
    ) -> Dict[str, Any]:
        """
        Submit a gradient update for a training round (requires trainer signature).

        Args:
            job_id: Job ID
            trainer: Trainer address (0x...)
            round_number: Training round number
            gradient_hash: Hash of gradient data (hex)
            gradient_data: Optional serialised gradient payload (hex or None)
            signature: Secp256k1 signature

        Returns:
            Result dict with submission_id and aggregation status
        """
        try:
            timestamp = int(time.time())
            params = {
                "job_id": job_id,
                "trainer": trainer,
                "round": round_number,
                "gradient_hash": gradient_hash,
                "signature": signature,
                "timestamp": timestamp,
            }
            if gradient_data is not None:
                params["gradient_data"] = gradient_data
            return self._rpc()._call_rpc("training_submitGradient", [params]) or {}
        except Exception as e:
            logger.error("Failed to submit gradient for job %s: %s", job_id, e)
            return {"success": False, "error": str(e)}

    def training_get_gradients(
        self, job_id: str, round_number: int
    ) -> List[Dict[str, Any]]:
        """
        Get all gradient submissions for a specific training round.

        Args:
            job_id: Job ID
            round_number: Training round number

        Returns:
            List of gradient submission dicts
        """
        try:
            params = {"job_id": job_id, "round": round_number}
            result = self._rpc()._call_rpc("training_getGradientsForRound", [params])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(
                f"Failed to get gradients for job {job_id} round {round_number}: {e}"
            )
            return []
