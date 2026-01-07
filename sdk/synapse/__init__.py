"""
Synapse module - Protocol definition for Axon/Dendrite communication.

This module provides the request/response data structures and serialization
for communication between validators (Dendrite) and miners (Axon).
"""

from .synapse import Synapse, SynapseRequest, SynapseResponse
from .types import (
    ForwardRequest,
    ForwardResponse,
    BackwardRequest,
    BackwardResponse,
)
from .serializer import SynapseSerializer
from .version import ProtocolVersion, version_compatible

__all__ = [
    "Synapse",
    "SynapseRequest",
    "SynapseResponse",
    "ForwardRequest",
    "ForwardResponse",
    "BackwardRequest",
    "BackwardResponse",
    "SynapseSerializer",
    "ProtocolVersion",
    "version_compatible",
]
