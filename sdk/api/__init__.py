"""
ModernTensor API Layer

Provides REST, GraphQL, and WebSocket APIs for interacting with the ModernTensor network.
This layer enables external applications to query and interact with the blockchain
and AI/ML network through standard HTTP, GraphQL, and WebSocket protocols.
"""

from .rest import RestAPI
from .websocket import WebSocketAPI
from .graphql import GraphQLAPI

__all__ = [
    "RestAPI",
    "WebSocketAPI",
    "GraphQLAPI",
]

__version__ = "0.5.0"
