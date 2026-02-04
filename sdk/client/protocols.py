"""
Type Protocols for LuxtensorClient Mixins

Defines structural interfaces for type checking without concrete inheritance.
Uses PEP 544 Protocol for duck-typed type safety.
"""

from typing import Any, List, Optional, Protocol, runtime_checkable


@runtime_checkable
class RPCProvider(Protocol):
    """
    Protocol defining the RPC client interface.

    All mixins expect this interface to be available at runtime.
    BaseClient provides the concrete implementation.

    This Protocol enables type-safe mixin composition without concrete inheritance,
    following the Liskov Substitution Principle and allowing static type checking
    of duck-typed behavior.
    """

    url: str
    network: str
    timeout: int
    _request_id: int

    def _get_request_id(self) -> int:
        """
        Get next request ID for RPC calls.

        Returns:
            Incremented request ID
        """
        ...

    def _call_rpc(self, method: str, params: Optional[List[Any]] = None) -> Any:
        """
        Make JSON-RPC call to Luxtensor.

        Args:
            method: RPC method name (e.g., "query_weightsVersion")
            params: Method parameters as list

        Returns:
            Result from RPC call

        Raises:
            Exception: If RPC call fails (network error, invalid response, etc.)
        """
        ...
