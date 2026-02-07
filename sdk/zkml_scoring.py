"""
ModernTensor SDK - zkML Integration for Scoring

Integrates zero-knowledge proof verification into the scoring pipeline.
Miners must submit proofs that they actually ran the model.

Flow:
1. Miner receives request from validator
2. Miner runs inference AND generates zkML proof
3. Miner returns response + proof
4. Validator verifies proof before scoring
5. Invalid/missing proof = score penalty or rejection

Usage:
    from sdk.zkml_scoring import ZkMLScoringConfig, ZkMLResponseVerifier

    config = ZkMLScoringConfig(
        require_proof=True,
        proof_weight=0.3,  # 30% of score from proof validity
    )

    verifier = ZkMLResponseVerifier(config)

    # Verify response with proof
    result = verifier.verify_response(
        request=my_request,
        response=my_response,
        proof=miner_proof,
    )

    if result.is_valid:
        final_score = result.adjusted_score
    else:
        # Handle invalid proof

Author: ModernTensor Team
Version: 1.0.0
"""

import logging
import time
from typing import Any, Dict, List, Optional, TYPE_CHECKING
from dataclasses import dataclass, field
from enum import Enum

if TYPE_CHECKING:
    from .ai_ml.zkml.proof_generator import Proof

logger = logging.getLogger(__name__)


# =============================================================================
# Configuration
# =============================================================================

class ProofRequirement(str, Enum):
    """How strictly to require proofs"""
    NONE = "none"  # Don't require proofs
    OPTIONAL = "optional"  # Accept but don't require
    RECOMMENDED = "recommended"  # Penalize missing proofs
    REQUIRED = "required"  # Reject responses without proofs


@dataclass
class ZkMLScoringConfig:
    """Configuration for zkML-integrated scoring"""

    # Proof requirement level
    requirement: ProofRequirement = ProofRequirement.RECOMMENDED

    # Weight of proof validity in final score (0.0 - 1.0)
    proof_weight: float = 0.2

    # Penalty multiplier for missing proof (when RECOMMENDED)
    missing_proof_penalty: float = 0.5

    # Penalty multiplier for invalid proof
    invalid_proof_penalty: float = 0.0

    # Verification timeout in seconds
    verification_timeout: float = 30.0

    # Cache verified proofs to avoid re-verification
    cache_verified: bool = True

    # Proof backend (ezkl, mock)
    backend: str = "ezkl"

    # Path to verification key (optional)
    verification_key_path: Optional[str] = None


@dataclass
class VerificationResult:
    """Result of proof verification"""
    is_valid: bool
    proof_score: float  # 0.0 - 1.0
    original_score: float
    adjusted_score: float
    verification_time: float
    error: Optional[str] = None
    details: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "is_valid": self.is_valid,
            "proof_score": self.proof_score,
            "original_score": self.original_score,
            "adjusted_score": self.adjusted_score,
            "verification_time": self.verification_time,
            "error": self.error,
            "details": self.details,
        }


# =============================================================================
# Proof-Aware Response Model
# =============================================================================

@dataclass
class ProofAttachedResponse:
    """Response from miner with attached zkML proof"""

    # Standard response data
    output: Any
    response_time: float
    metadata: Dict[str, Any] = field(default_factory=dict)

    # zkML proof
    proof_data: Optional[bytes] = None
    proof_hash: Optional[str] = None
    circuit_hash: Optional[str] = None
    public_inputs: Optional[List[float]] = None
    public_outputs: Optional[List[float]] = None

    @property
    def has_proof(self) -> bool:
        return self.proof_data is not None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "output": self.output,
            "response_time": self.response_time,
            "has_proof": self.has_proof,
            "proof_hash": self.proof_hash,
            "circuit_hash": self.circuit_hash,
            "metadata": self.metadata,
        }

    @classmethod
    def from_synapse_response(cls, response: Dict[str, Any]) -> 'ProofAttachedResponse':
        """Create from SynapseResponse payload"""
        payload = response.get("payload", {})
        zkml = payload.get("zkml", {})

        proof_data = None
        if zkml.get("proof"):
            proof_data = bytes.fromhex(zkml["proof"]) if isinstance(zkml["proof"], str) else zkml["proof"]

        return cls(
            output=payload.get("output"),
            response_time=response.get("processing_time", 0),
            metadata=response.get("metadata", {}),
            proof_data=proof_data,
            proof_hash=zkml.get("proof_hash"),
            circuit_hash=zkml.get("circuit_hash"),
            public_inputs=zkml.get("public_inputs"),
            public_outputs=zkml.get("public_outputs"),
        )


# =============================================================================
# Verifier
# =============================================================================

class ZkMLResponseVerifier:
    """
    Verify miner responses with zkML proofs.

    Integrates with the scoring pipeline to adjust scores based on proof validity.

    Example:
        config = ZkMLScoringConfig(
            requirement=ProofRequirement.REQUIRED,
            proof_weight=0.3,
        )

        verifier = ZkMLResponseVerifier(config)

        result = verifier.verify_response(
            request=request_data,
            response=response_with_proof,
            original_score=0.85,
        )

        if result.is_valid:
            print(f"Final score: {result.adjusted_score}")
        else:
            print(f"Invalid: {result.error}")
    """

    def __init__(self, config: ZkMLScoringConfig):
        """
        Initialize verifier.

        Args:
            config: Verification configuration
        """
        self.config = config
        self._proof_verifier = None
        self._cache: Dict[str, bool] = {}

        self._init_verifier()

    def _init_verifier(self):
        """Initialize the underlying proof verifier"""
        try:
            from sdk.ai_ml.zkml import ProofVerifier, ProofConfig

            proof_config = ProofConfig(backend=self.config.backend)

            vk = None
            if self.config.verification_key_path:
                from pathlib import Path
                vk_path = Path(self.config.verification_key_path)
                if vk_path.exists():
                    vk = vk_path.read_bytes()

            self._proof_verifier = ProofVerifier(
                verification_key=vk,
                config=proof_config,
            )

            logger.info(f"zkML verifier initialized with backend: {self.config.backend}")

        except ImportError as e:
            logger.warning(f"Could not import zkML modules: {e}")
            self._proof_verifier = None

    def verify_response(
        self,
        request: Any,
        response: ProofAttachedResponse,
        original_score: float = 1.0,
    ) -> VerificationResult:
        """
        Verify a response with proof and adjust score.

        Args:
            request: Original request data
            response: Response with attached proof
            original_score: Score before proof verification

        Returns:
            VerificationResult with adjusted score
        """
        start_time = time.time()

        # Check if proofs are required
        if self.config.requirement == ProofRequirement.NONE:
            return VerificationResult(
                is_valid=True,
                proof_score=1.0,
                original_score=original_score,
                adjusted_score=original_score,
                verification_time=0,
                details={"skipped": True},
            )

        # Check if proof is present
        if not response.has_proof:
            return self._handle_missing_proof(original_score, time.time() - start_time)

        # Check cache
        if self.config.cache_verified and response.proof_hash:
            if response.proof_hash in self._cache:
                cached_valid = self._cache[response.proof_hash]
                return self._create_result(
                    is_valid=cached_valid,
                    original_score=original_score,
                    verification_time=time.time() - start_time,
                    details={"cached": True},
                )

        # Verify proof
        try:
            is_valid = self._verify_proof(response)

            # Cache result
            if self.config.cache_verified and response.proof_hash:
                self._cache[response.proof_hash] = is_valid

            verification_time = time.time() - start_time

            if is_valid:
                return VerificationResult(
                    is_valid=True,
                    proof_score=1.0,
                    original_score=original_score,
                    adjusted_score=original_score,  # Full score
                    verification_time=verification_time,
                    details={"verified": True},
                )
            else:
                return self._handle_invalid_proof(original_score, verification_time)

        except Exception as e:
            logger.error(f"Proof verification error: {e}")
            return VerificationResult(
                is_valid=False,
                proof_score=0.0,
                original_score=original_score,
                adjusted_score=original_score * self.config.invalid_proof_penalty,
                verification_time=time.time() - start_time,
                error=str(e),
            )

    def _verify_proof(self, response: ProofAttachedResponse) -> bool:
        """Verify the proof using zkML verifier"""
        if self._proof_verifier is None:
            logger.error("No proof verifier available — rejecting proof for security")
            return False

        try:
            from sdk.ai_ml.zkml import Proof

            proof = Proof(
                proof_data=response.proof_data,
                public_inputs=response.public_inputs or [],
                public_outputs=response.public_outputs or [],
                circuit_hash=response.circuit_hash or "",
            )

            return self._proof_verifier.verify(proof)

        except Exception as e:
            logger.error(f"Proof verification failed: {e}")
            return False

    def _handle_missing_proof(
        self,
        original_score: float,
        verification_time: float,
    ) -> VerificationResult:
        """Handle case where proof is missing"""

        if self.config.requirement == ProofRequirement.REQUIRED:
            # Reject completely
            return VerificationResult(
                is_valid=False,
                proof_score=0.0,
                original_score=original_score,
                adjusted_score=0.0,
                verification_time=verification_time,
                error="Proof required but not provided",
            )

        elif self.config.requirement == ProofRequirement.RECOMMENDED:
            # Penalize but accept
            adjusted = original_score * self.config.missing_proof_penalty
            return VerificationResult(
                is_valid=True,  # Accept but penalize
                proof_score=0.0,
                original_score=original_score,
                adjusted_score=adjusted,
                verification_time=verification_time,
                details={"missing_proof_penalty": True},
            )

        else:  # OPTIONAL
            return VerificationResult(
                is_valid=True,
                proof_score=0.0,
                original_score=original_score,
                adjusted_score=original_score,
                verification_time=verification_time,
                details={"proof_optional": True},
            )

    def _handle_invalid_proof(
        self,
        original_score: float,
        verification_time: float,
    ) -> VerificationResult:
        """Handle case where proof is invalid"""

        adjusted = original_score * self.config.invalid_proof_penalty

        return VerificationResult(
            is_valid=False,
            proof_score=0.0,
            original_score=original_score,
            adjusted_score=adjusted,
            verification_time=verification_time,
            error="Proof verification failed",
        )

    def _create_result(
        self,
        is_valid: bool,
        original_score: float,
        verification_time: float,
        details: Dict[str, Any],
    ) -> VerificationResult:
        """Create result from cached validation"""
        if is_valid:
            return VerificationResult(
                is_valid=True,
                proof_score=1.0,
                original_score=original_score,
                adjusted_score=original_score,
                verification_time=verification_time,
                details=details,
            )
        else:
            return self._handle_invalid_proof(original_score, verification_time)

    def clear_cache(self):
        """Clear verification cache"""
        self._cache.clear()


# =============================================================================
# Miner-side Proof Generator Helper
# =============================================================================

class MinerProofHelper:
    """
    Helper for miners to generate proofs with their responses.

    Example:
        helper = MinerProofHelper(model_path="model.onnx")

        # Generate response with proof
        output = run_my_model(input_data)
        proof = helper.generate_proof(input_data, output)

        # Attach to response
        response = ProofAttachedResponse(
            output=output,
            response_time=elapsed,
            proof_data=proof.proof_data,
            proof_hash=proof.circuit_hash,
            public_inputs=proof.public_inputs,
            public_outputs=proof.public_outputs,
        )
    """

    def __init__(
        self,
        model_path: Optional[str] = None,
        backend: str = "ezkl",
    ):
        """
        Initialize proof helper.

        Args:
            model_path: Path to ONNX model file
            backend: Proof backend (ezkl, mock)
        """
        self.model_path = model_path
        self.backend = backend
        self._generator = None
        self._is_setup = False

    def setup(self):
        """Setup the proof generator"""
        try:
            from sdk.ai_ml.zkml import ProofGenerator, ProofConfig
            from pathlib import Path

            config = ProofConfig(backend=self.backend)
            self._generator = ProofGenerator(config)

            if self.model_path:
                self._generator.setup_circuit(model_path=Path(self.model_path))
            else:
                self._generator.setup_circuit()

            self._is_setup = True
            logger.info("Miner proof helper setup complete")

        except Exception as e:
            logger.error(f"Failed to setup proof generator: {e}")
            self._is_setup = False

    def generate_proof(
        self,
        input_data: List[float],
        output_data: List[float],
    ) -> 'Proof':
        """
        Generate proof for inference.

        Args:
            input_data: Model inputs
            output_data: Model outputs

        Returns:
            Proof object
        """
        if not self._is_setup:
            self.setup()

        if not self._is_setup or self._generator is None:
            raise RuntimeError("Proof generator not setup")

        return self._generator.generate_proof(input_data, output_data)

    def attach_proof_to_response(
        self,
        response: Dict[str, Any],
        input_data: List[float],
        output_data: List[float],
    ) -> Dict[str, Any]:
        """
        Generate proof and attach to response payload.

        Args:
            response: Response dictionary
            input_data: Model inputs
            output_data: Model outputs

        Returns:
            Response with proof attached in payload
        """
        try:
            proof = self.generate_proof(input_data, output_data)

            # Attach to payload
            if "payload" not in response:
                response["payload"] = {}

            response["payload"]["zkml"] = {
                "proof": proof.proof_data.hex(),
                "proof_hash": proof.circuit_hash,
                "circuit_hash": proof.circuit_hash,
                "public_inputs": proof.public_inputs,
                "public_outputs": proof.public_outputs,
                "timestamp": proof.timestamp,
            }

            return response

        except Exception as e:
            logger.error(f"Failed to attach proof: {e}")
            return response


# =============================================================================
# Module exports
# =============================================================================

__all__ = [
    "ZkMLScoringConfig",
    "ZkMLResponseVerifier",
    "MinerProofHelper",
    "ProofAttachedResponse",
    "ProofRequirement",
    "VerificationResult",
]
