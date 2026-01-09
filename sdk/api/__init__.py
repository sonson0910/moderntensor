"""
ModernTensor API Layer

Provides REST and WebSocket APIs for interacting with the ModernTensor network.
This layer enables external applications to query and interact with the blockchain
and AI/ML network through standard HTTP and WebSocket protocols.
"""

from .rest import RestAPI
from .websocket import WebSocketAPI

__all__ = [
    "RestAPI",
    "WebSocketAPI",
]

__version__ = "0.4.0"
