"""
Luxtensor Error Handling Module

Provides structured error types matching the Rust RPC implementation.
Includes JSON-RPC 2.0 error codes and detailed error parsing.
"""

from typing import Optional, Dict, Any
from dataclasses import dataclass
from enum import IntEnum


class RpcErrorCode(IntEnum):
    """Standard JSON-RPC 2.0 error codes + Luxtensor custom codes."""

    # Standard JSON-RPC 2.0 errors
    PARSE_ERROR = -32700
    INVALID_REQUEST = -32600
    METHOD_NOT_FOUND = -32601
    INVALID_PARAMS = -32602
    INTERNAL_ERROR = -32603

    # Luxtensor custom errors (-32000 to -32099)
    BLOCK_NOT_FOUND = -32001
    TRANSACTION_NOT_FOUND = -32002
    ACCOUNT_NOT_FOUND = -32003
    INSUFFICIENT_FUNDS = -32004
    INVALID_SIGNATURE = -32005
    NONCE_TOO_LOW = -32006
    NONCE_TOO_HIGH = -32007
    GAS_LIMIT_EXCEEDED = -32008
    CONTRACT_EXECUTION_ERROR = -32009
    RATE_LIMITED = -32010
    MEMPOOL_FULL = -32011
    STORAGE_ERROR = -32050


@dataclass
class RpcError(Exception):
    """
    Structured RPC error with code, message, and optional data.

    Matches the Luxtensor Rust RpcError implementation.
    """
    code: int
    message: str
    data: Optional[Dict[str, Any]] = None

    def __str__(self) -> str:
        if self.data:
            return f"RpcError({self.code}): {self.message} - {self.data}"
        return f"RpcError({self.code}): {self.message}"

    @classmethod
    def from_response(cls, error: Dict[str, Any]) -> "RpcError":
        """
        Parse RPC error from JSON response.

        Args:
            error: Error dict from JSON-RPC response

        Returns:
            RpcError instance
        """
        return cls(
            code=error.get("code", RpcErrorCode.INTERNAL_ERROR),
            message=error.get("message", "Unknown error"),
            data=error.get("data")
        )

    @property
    def is_retryable(self) -> bool:
        """Check if this error is retryable."""
        retryable_codes = [
            RpcErrorCode.INTERNAL_ERROR,
            RpcErrorCode.RATE_LIMITED,
        ]
        return self.code in retryable_codes

    @property
    def error_name(self) -> str:
        """Get human-readable error name."""
        try:
            return RpcErrorCode(self.code).name
        except ValueError:
            return "UNKNOWN_ERROR"


class BlockNotFoundError(RpcError):
    """Block not found error."""
    def __init__(self, block_id: str):
        super().__init__(
            code=RpcErrorCode.BLOCK_NOT_FOUND,
            message=f"Block not found: {block_id}"
        )


class TransactionNotFoundError(RpcError):
    """Transaction not found error."""
    def __init__(self, tx_hash: str):
        super().__init__(
            code=RpcErrorCode.TRANSACTION_NOT_FOUND,
            message=f"Transaction not found: {tx_hash}"
        )


class InsufficientFundsError(RpcError):
    """Insufficient funds error."""
    def __init__(self, have: int, need: int):
        super().__init__(
            code=RpcErrorCode.INSUFFICIENT_FUNDS,
            message=f"Insufficient funds: have {have}, need {need}",
            data={"have": str(have), "need": str(need)}
        )


class InvalidSignatureError(RpcError):
    """Invalid signature error."""
    def __init__(self):
        super().__init__(
            code=RpcErrorCode.INVALID_SIGNATURE,
            message="Invalid transaction signature"
        )


class NonceTooLowError(RpcError):
    """Nonce too low error."""
    def __init__(self, expected: int, got: int):
        super().__init__(
            code=RpcErrorCode.NONCE_TOO_LOW,
            message=f"Nonce too low: expected {expected}, got {got}",
            data={"expected": expected, "got": got}
        )


class GasLimitExceededError(RpcError):
    """Gas limit exceeded error."""
    def __init__(self, limit: int, required: int):
        super().__init__(
            code=RpcErrorCode.GAS_LIMIT_EXCEEDED,
            message=f"Gas limit exceeded: limit {limit}, required {required}",
            data={"limit": limit, "required": required}
        )


class RateLimitedError(RpcError):
    """Rate limited error."""
    def __init__(self, message: str = "Too many requests"):
        super().__init__(
            code=RpcErrorCode.RATE_LIMITED,
            message=message
        )


class MempoolFullError(RpcError):
    """Mempool full error."""
    def __init__(self, current: int, max_size: int):
        super().__init__(
            code=RpcErrorCode.MEMPOOL_FULL,
            message=f"Mempool full: {current}/{max_size} transactions",
            data={"current": current, "max": max_size}
        )


def parse_rpc_error(error: Dict[str, Any]) -> RpcError:
    """
    Parse RPC error and return appropriate error class.

    Args:
        error: Error dict from JSON-RPC response

    Returns:
        Specific RpcError subclass or generic RpcError
    """
    code = error.get("code", RpcErrorCode.INTERNAL_ERROR)
    message = error.get("message", "Unknown error")
    data = error.get("data", {})

    # Map to specific error classes
    if code == RpcErrorCode.BLOCK_NOT_FOUND:
        return BlockNotFoundError(message.split(": ")[-1] if ": " in message else "")
    elif code == RpcErrorCode.TRANSACTION_NOT_FOUND:
        return TransactionNotFoundError(message.split(": ")[-1] if ": " in message else "")
    elif code == RpcErrorCode.INSUFFICIENT_FUNDS:
        return InsufficientFundsError(
            have=int(data.get("have", 0)),
            need=int(data.get("need", 0))
        )
    elif code == RpcErrorCode.INVALID_SIGNATURE:
        return InvalidSignatureError()
    elif code == RpcErrorCode.NONCE_TOO_LOW:
        return NonceTooLowError(
            expected=int(data.get("expected", 0)),
            got=int(data.get("got", 0))
        )
    elif code == RpcErrorCode.GAS_LIMIT_EXCEEDED:
        return GasLimitExceededError(
            limit=int(data.get("limit", 0)),
            required=int(data.get("required", 0))
        )
    elif code == RpcErrorCode.RATE_LIMITED:
        return RateLimitedError(message)
    elif code == RpcErrorCode.MEMPOOL_FULL:
        return MempoolFullError(
            current=int(data.get("current", 0)),
            max_size=int(data.get("max", 0))
        )
    else:
        return RpcError.from_response(error)


# Convenience function for checking RPC responses
def check_rpc_response(response: Dict[str, Any]) -> Any:
    """
    Check RPC response and raise error if present.

    Args:
        response: Full JSON-RPC response dict

    Returns:
        The 'result' field if successful

    Raises:
        RpcError: If response contains an error
    """
    if "error" in response:
        raise parse_rpc_error(response["error"])
    return response.get("result")


class LuxtensorConnectionError(Exception):
    """Raised when unable to connect to a Luxtensor node.

    Replaces bare ``Exception`` in ``BaseClient._call_rpc()`` so callers
    can catch connection errors specifically without masking unrelated errors.
    """

    def __init__(self, url: str, cause: Optional[Exception] = None):
        self.url = url
        self.cause = cause
        msg = f"Failed to connect to Luxtensor at {url}"
        if cause:
            msg += f": {cause}"
        super().__init__(msg)
