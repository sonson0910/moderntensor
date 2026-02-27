"""
Synapse module — Protocol definition for Axon/Dendrite communication.

This module defines the request/response data structures and serialization
for communication between validators (Dendrite) and miners (Axon).

Usage::

    from sdk.synapse import Synapse, SynapseRequest

    request = Synapse.create_request(
        message_type="forward",
        payload={"input": tensor_data},
        sender_uid="validator_001",
    )
    Synapse.validate_request(request)
"""

from .synapse import Synapse, SynapseRequest, SynapseResponse
from .types import (
    ForwardRequest,
    ForwardResponse,
    BackwardRequest,
    BackwardResponse,
)
from .serializer import SynapseSerializer
from .version import ProtocolVersion, version_compatible, CURRENT_VERSION

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
    "CURRENT_VERSION",
]
