"""
Base RPC Client

Shared functionality for all domain-specific RPC clients.

Features:
    - Automatic retry with exponential backoff for transient errors
    - Typed exceptions (:class:`RpcError`, :class:`RpcConnectionError`)
    - Persistent HTTP connections with connection pooling
    - URL validation on construction

Usage::

    client = BaseRpcClient("http://localhost:8545")
    result = client._call_rpc("eth_blockNumber")
"""

import logging
import time
from typing import Optional, List, Any, Callable
from urllib.parse import urlparse

import httpx

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Typed exceptions
# ---------------------------------------------------------------------------


class RpcError(Exception):
    """Raised when the RPC server returns a JSON-RPC error response."""

    def __init__(self, code: int, message: str):
        self.code = code
        self.message = message
        super().__init__(f"RPC error {code}: {message}")


class RpcConnectionError(Exception):
    """Raised when the client cannot reach the RPC server."""


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_MAX_RETRIES = 3
_RETRY_BASE_DELAY = 0.5  # seconds


class BaseRpcClient:
    """
    Base class for domain-specific RPC clients.

    Provides shared JSON-RPC call functionality with automatic retry
    for transient network failures.

    Args:
        url: Luxtensor RPC endpoint URL (must be http or https).
        timeout: Request timeout in seconds (>= 1).
        _shared_request_id: Optional shared request ID generator for composition.
    """

    def __init__(
        self,
        url: str = "http://localhost:8545",
        timeout: int = 30,
        _shared_request_id: Optional[Callable[[], int]] = None,
    ):
        # Validate URL
        parsed = urlparse(url)
        if parsed.scheme not in ("http", "https"):
            raise ValueError(
                f"Invalid RPC URL scheme '{parsed.scheme}': expected 'http' or 'https'"
            )
        if not parsed.hostname:
            raise ValueError(f"Invalid RPC URL: missing hostname in '{url}'")

        if timeout < 1:
            raise ValueError(f"Timeout must be >= 1, got {timeout}")

        self.url = url
        self.timeout = timeout
        self._request_id = 0
        self._shared_request_id = _shared_request_id
        self._http_client: Optional[httpx.Client] = None

    # ----- lifecycle -----

    def close(self) -> None:
        """Close the underlying HTTP client and release its connection pool."""
        if self._http_client is not None:
            self._http_client.close()
            self._http_client = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    # ----- internals -----

    def _get_request_id(self) -> int:
        """Get next request ID (uses shared generator if available)."""
        if self._shared_request_id:
            return self._shared_request_id()
        self._request_id += 1
        return self._request_id

    def _get_http_client(self) -> httpx.Client:
        """Get or create a persistent HTTP client for connection reuse."""
        if self._http_client is None:
            self._http_client = httpx.Client(timeout=self.timeout)
        return self._http_client

    def _call_rpc(self, method: str, params: Optional[List[Any]] = None) -> Any:
        """
        Make a JSON-RPC 2.0 call to the Luxtensor node.

        Retries up to :data:`_MAX_RETRIES` times on transient network errors
        with exponential backoff.

        Args:
            method: RPC method name.
            params: Method parameters.

        Returns:
            Result from the RPC call.

        Raises:
            RpcError: If the server returns a JSON-RPC error.
            RpcConnectionError: If all retry attempts fail.
        """
        request = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": self._get_request_id(),
        }

        last_exc: Optional[Exception] = None

        for attempt in range(_MAX_RETRIES):
            try:
                client = self._get_http_client()
                response = client.post(self.url, json=request)
                response.raise_for_status()

                result = response.json()

                if "error" in result:
                    err = result["error"]
                    code = err.get("code", -1) if isinstance(err, dict) else -1
                    msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
                    raise RpcError(code, msg)

                return result.get("result")

            except RpcError:
                raise  # server-side errors are not retryable
            except (httpx.RequestError, httpx.HTTPStatusError) as exc:
                last_exc = exc
                delay = _RETRY_BASE_DELAY * (2 ** attempt)
                logger.warning(
                    "RPC request to %s failed (attempt %d/%d): %s — retrying in %.1fs",
                    self.url, attempt + 1, _MAX_RETRIES, exc, delay,
                )
                time.sleep(delay)

        raise RpcConnectionError(
            f"Failed to connect to Luxtensor at {self.url} after "
            f"{_MAX_RETRIES} attempts: {last_exc}"
        ) from last_exc

    @staticmethod
    def _hex_to_int(value: Any) -> int:
        """Convert hex string to int, handles various input types."""
        if isinstance(value, str):
            return int(value, 16) if value.startswith("0x") else int(value)
        return value if value else 0

