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
"""

from .proof_generator import ProofGenerator, ProofConfig, Proof
from .verifier import ProofVerifier
from .circuit import CircuitCompiler, Circuit

__all__ = [
    "ProofGenerator",
    "ProofConfig",
    "Proof",
    "ProofVerifier",
    "CircuitCompiler",
    "Circuit",
]
