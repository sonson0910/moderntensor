"""
Base RPC Client
Shared functionality for all domain clients.
"""

import logging
from typing import Optional, List, Any, Callable
import httpx

logger = logging.getLogger(__name__)


class BaseRpcClient:
    """
    Base class for domain-specific RPC clients.
    Provides shared RPC call functionality.
    """

    def __init__(
        self,
        url: str = "http://localhost:8545",
        timeout: int = 30,
        _shared_request_id: Optional[Callable[[], int]] = None,
    ):
        """
        Initialize base RPC client.

        Args:
            url: Luxtensor RPC endpoint URL
            timeout: Request timeout in seconds
            _shared_request_id: Optional shared request ID generator (for composition)
        """
        self.url = url
        self.timeout = timeout
        self._request_id = 0
        self._shared_request_id = _shared_request_id
        self._http_client: Optional[httpx.Client] = None

    def _get_request_id(self) -> int:
        """Get next request ID (uses shared generator if available)"""
        if self._shared_request_id:
            return self._shared_request_id()
        self._request_id += 1
        return self._request_id

    def _get_http_client(self) -> httpx.Client:
        """Get or create a persistent HTTP client for connection reuse.

        Creates the client once on first call and reuses it across requests,
        allowing the underlying httpx connection pool to stay warm.
        """
        if self._http_client is None:
            self._http_client = httpx.Client(timeout=self.timeout)
        return self._http_client

    def _call_rpc(self, method: str, params: Optional[List[Any]] = None) -> Any:
        """
        Make JSON-RPC call to Luxtensor.

        Args:
            method: RPC method name
            params: Method parameters

        Returns:
            Result from RPC call

        Raises:
            Exception: If RPC call fails
        """
        request = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": self._get_request_id()
        }

        try:
            client = self._get_http_client()
            response = client.post(self.url, json=request)
            response.raise_for_status()

            result = response.json()

            if "error" in result:
                raise Exception(f"RPC error: {result['error']}")

            return result.get("result")

        except httpx.RequestError as e:
            logger.error(f"Request error: {e}")
            raise Exception(f"Failed to connect to Luxtensor at {self.url}: {e}")
        except Exception as e:
            logger.error(f"RPC call failed: {e}")
            raise

    @staticmethod
    def _hex_to_int(value: Any) -> int:
        """Convert hex string to int, handles various input types"""
        if isinstance(value, str):
            return int(value, 16) if value.startswith('0x') else int(value)
        return value if value else 0
