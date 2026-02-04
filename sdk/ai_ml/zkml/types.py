"""
Core ZK types matching luxtensor-zkvm for SDK compatibility.

These types mirror the Rust structures in luxtensor-zkvm/src/types.rs
for seamless interoperability between Python SDK and Rust core.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional
import hashlib
import json


class ProofType(str, Enum):
    """Type of proof - matches luxtensor-zkvm ProofType"""
    STARK = "stark"      # Standard RISC Zero proof (STARK-based)
    GROTH16 = "groth16"  # SNARK-wrapped proof (smaller, uses Groth16)
    DEV = "dev"          # Development mode (no actual proof, for testing)


@dataclass
class ImageId:
    """
    Unique identifier for a guest program image.
    Matches luxtensor-zkvm ImageId.
    """
    bytes: bytes  # 32 bytes

    def __init__(self, data: bytes):
        if len(data) != 32:
            raise ValueError("ImageId must be exactly 32 bytes")
        self.bytes = data

    @classmethod
    def new(cls, data: bytes) -> 'ImageId':
        """Create a new ImageId from bytes"""
        return cls(data)

    @classmethod
    def from_hex(cls, hex_str: str) -> 'ImageId':
        """Create from hex string"""
        data = bytes.fromhex(hex_str)
        return cls(data)

    def to_hex(self) -> str:
        """Convert to hex string"""
        return self.bytes.hex()

    def __str__(self) -> str:
        return self.to_hex()

    def __eq__(self, other) -> bool:
        if isinstance(other, ImageId):
            return self.bytes == other.bytes
        return False

    def __hash__(self) -> int:
        return hash(self.bytes)


@dataclass
class GuestInput:
    """
    Input data for a guest program.
    Matches luxtensor-zkvm GuestInput.
    """
    data: bytes
    private_data: Optional[bytes] = None

    @classmethod
    def new(cls, data: bytes) -> 'GuestInput':
        """Create new guest input"""
        return cls(data=data)

    @classmethod
    def with_private(cls, data: bytes, private: bytes) -> 'GuestInput':
        """Create guest input with private data"""
        return cls(data=data, private_data=private)

    @classmethod
    def from_list(cls, inputs: List[float]) -> 'GuestInput':
        """Create from list of floats"""
        data = json.dumps(inputs).encode()
        return cls(data=data)


@dataclass
class GuestOutput:
    """
    Output data from a guest program.
    Matches luxtensor-zkvm GuestOutput.
    """
    journal: bytes  # Public outputs (committed)
    cycles: int = 0  # Execution cycles used

    def decode_json(self) -> Any:
        """Deserialize journal from JSON"""
        return json.loads(self.journal.decode())

    def decode_floats(self) -> List[float]:
        """Decode journal as list of floats"""
        return self.decode_json()


@dataclass
class ProofMetadata:
    """
    Metadata about proof generation.
    Matches luxtensor-zkvm ProofMetadata.
    """
    cycles: int = 0
    proving_time_ms: int = 0
    memory_bytes: int = 0
    gpu_used: bool = False
    segments: int = 1

    def to_dict(self) -> Dict[str, Any]:
        return {
            "cycles": self.cycles,
            "proving_time_ms": self.proving_time_ms,
            "memory_bytes": self.memory_bytes,
            "gpu_used": self.gpu_used,
            "segments": self.segments,
        }


@dataclass
class ProofReceipt:
    """
    Complete proof receipt including proof and public outputs.
    Matches luxtensor-zkvm ProofReceipt.
    """
    image_id: ImageId
    journal: bytes  # Public outputs
    seal: bytes     # The proof bytes
    proof_type: ProofType
    metadata: ProofMetadata = field(default_factory=ProofMetadata)

    def commitment_hash(self) -> bytes:
        """Compute the commitment hash of this receipt"""
        data = self.image_id.bytes + self.journal + self.seal
        return hashlib.sha3_256(data).digest()

    def to_bytes(self) -> bytes:
        """Serialize the receipt for on-chain submission"""
        return json.dumps({
            "image_id": self.image_id.to_hex(),
            "journal": self.journal.hex(),
            "seal": self.seal.hex(),
            "proof_type": self.proof_type.value,
            "metadata": self.metadata.to_dict(),
        }).encode()

    @classmethod
    def from_bytes(cls, data: bytes) -> 'ProofReceipt':
        """Deserialize a receipt"""
        obj = json.loads(data.decode())
        return cls(
            image_id=ImageId.from_hex(obj["image_id"]),
            journal=bytes.fromhex(obj["journal"]),
            seal=bytes.fromhex(obj["seal"]),
            proof_type=ProofType(obj["proof_type"]),
            metadata=ProofMetadata(**obj.get("metadata", {})),
        )


@dataclass
class ProverConfig:
    """
    Configuration for the prover.
    Matches luxtensor-zkvm ProverConfig.
    """
    use_gpu: bool = False
    max_memory: int = 0  # 0 = unlimited
    timeout_seconds: int = 300  # 5 minutes default
    wrap_to_groth16: bool = False
    threads: int = 1

    @classmethod
    def default(cls) -> 'ProverConfig':
        """Create default config"""
        import os
        return cls(threads=os.cpu_count() or 1)

    @classmethod
    def with_gpu(cls) -> 'ProverConfig':
        """Create GPU-accelerated config"""
        config = cls.default()
        config.use_gpu = True
        return config

    @classmethod
    def dev_mode(cls) -> 'ProverConfig':
        """Create fast development config (no real proofs)"""
        return cls(
            use_gpu=False,
            max_memory=0,
            timeout_seconds=60,
            wrap_to_groth16=False,
            threads=1,
        )


@dataclass
class VerificationResult:
    """
    Result of proof verification.
    Matches luxtensor-zkvm VerificationResult.
    """
    is_valid: bool
    image_id: ImageId
    journal_hash: bytes  # 32 bytes
    verification_time_us: int = 0
    error: Optional[str] = None

    @classmethod
    def valid(cls, image_id: ImageId, journal_hash: bytes, time_us: int) -> 'VerificationResult':
        """Create a successful verification result"""
        return cls(
            is_valid=True,
            image_id=image_id,
            journal_hash=journal_hash,
            verification_time_us=time_us,
        )

    @classmethod
    def invalid(cls, image_id: ImageId, error: str) -> 'VerificationResult':
        """Create a failed verification result"""
        return cls(
            is_valid=False,
            image_id=image_id,
            journal_hash=bytes(32),
            verification_time_us=0,
            error=error,
        )

    def to_dict(self) -> Dict[str, Any]:
        return {
            "is_valid": self.is_valid,
            "image_id": self.image_id.to_hex(),
            "journal_hash": self.journal_hash.hex(),
            "verification_time_us": self.verification_time_us,
            "error": self.error,
        }


# Module exports
__all__ = [
    "ProofType",
    "ImageId",
    "GuestInput",
    "GuestOutput",
    "ProofMetadata",
    "ProofReceipt",
    "ProverConfig",
    "VerificationResult",
]
