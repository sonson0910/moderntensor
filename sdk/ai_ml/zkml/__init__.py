"""
zkML (Zero-Knowledge Machine Learning) Integration Module.

Provides verifiable ML inference using zero-knowledge proofs.
This surpasses Bittensor which doesn't have zkML support.

Features:
- Proof generation for ML inferences
- Proof verification
- EZKL integration
- Circuit compilation
- Witness generation
- Full compatibility with luxtensor-zkvm (Rust core)
"""

from .proof_generator import ProofGenerator, ProofConfig, Proof
from .verifier import ProofVerifier
from .circuit import CircuitCompiler, Circuit
from .types import (
    ProofType,
    ImageId,
    GuestInput,
    GuestOutput,
    ProofMetadata,
    ProofReceipt,
    ProverConfig,
    VerificationResult,
)

__all__ = [
    # Proof generation
    "ProofGenerator",
    "ProofConfig",
    "Proof",
    "ProofVerifier",
    "CircuitCompiler",
    "Circuit",
    # Core types (match luxtensor-zkvm)
    "ProofType",
    "ImageId",
    "GuestInput",
    "GuestOutput",
    "ProofMetadata",
    "ProofReceipt",
    "ProverConfig",
    "VerificationResult",
]

