"""
LuxtensorClient Base Module

Core RPC functionality and data classes.
All mixins inherit from this base.
"""

import logging
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Union

import httpx

from sdk.errors import parse_rpc_error

logger = logging.getLogger(__name__)


@dataclass
class ChainInfo:
    """Blockchain information"""

    chain_id: str
    network: str
    block_height: int
    version: str


@dataclass
class Account:
    """Account information from Luxtensor (matches luxtensor-core Account struct)"""

    address: str
    balance: int
    nonce: int
    storage_root: str = "0x" + "00" * 32
    code_hash: str = "0x" + "00" * 32
    code: Optional[bytes] = None
    stake: int = 0


@dataclass
class TransactionResult:
    """Transaction submission result"""

    tx_hash: str
    status: str
    block_number: Optional[int] = None
    error: Optional[str] = None

    @property
    def success(self) -> bool:
        """Check if transaction was successful."""
        return self.status == "success" and self.error is None


class BaseClient:
    """
    Base client with core RPC functionality.
    All mixin classes expect these methods to be available.
    """

    url: str
    network: str
    timeout: int
    _request_id: int
    _http_client: Optional[httpx.Client]

    def _get_request_id(self) -> int:
        """Get next request ID"""
        self._request_id += 1
        return self._request_id

    def _get_http_client(self) -> httpx.Client:
        """Get or create a persistent HTTP client for connection reuse."""
        if not hasattr(self, '_http_client') or self._http_client is None:
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
            "id": self._get_request_id(),
        }

        try:
            client = self._get_http_client()
            response = client.post(self.url, json=request)
            response.raise_for_status()

            result = response.json()

            if "error" in result:
                raise parse_rpc_error(result["error"])

            return result.get("result")

        except httpx.RequestError as e:
            logger.error(f"Request error: {e}")
            raise Exception(f"Failed to connect to Luxtensor at {self.url}: {e}")
        except Exception as e:
            logger.error(f"RPC call failed: {e}")
            raise

    def _safe_call_rpc(self, method: str, params: Optional[List[Any]] = None) -> Optional[Any]:
        """
        Make JSON-RPC call, returning None on any error instead of raising.

        Use this for query methods where callers need to distinguish
        'value not found' from 'RPC error'. For mutations, use _call_rpc().

        Args:
            method: RPC method name
            params: Method parameters

        Returns:
            Result from RPC call, or None if the call failed
        """
        try:
            return self._call_rpc(method, params)
        except Exception as e:
            logger.warning(f"RPC call {method} failed (safe mode): {e}")
            return None

    @staticmethod
    def _parse_hex_int(value: Union[str, int, None], default: int = 0) -> int:
        """
        Parse a hex string or int value into an integer.

        Handles common RPC return formats: '0x1a2b', '12345', 12345, None.

        Args:
            value: Value to parse (hex string, decimal string, int, or None)
            default: Default value if parsing fails

        Returns:
            Parsed integer value
        """
        if value is None:
            return default
        if isinstance(value, int):
            return value
        if isinstance(value, str):
            try:
                return int(value, 16) if value.startswith("0x") else int(value)
            except (ValueError, TypeError):
                return default
        return default

    def _call_rpc_batch(
        self, requests: List[Dict[str, Any]]
    ) -> List[Optional[Any]]:
        """
        Make a batch JSON-RPC call (multiple requests in a single HTTP call).

        Follows the JSON-RPC 2.0 batch spec: sends an array of request objects
        and receives an array of response objects.

        Args:
            requests: List of dicts with 'method' and optional 'params' keys.
                      Example: [{'method': 'staking_getStake', 'params': ['0x...']}]

        Returns:
            List of results in the same order as requests.
            Individual failures are returned as None.
        """
        if not requests:
            return []

        batch = []
        for req in requests:
            batch.append({
                "jsonrpc": "2.0",
                "method": req["method"],
                "params": req.get("params", []),
                "id": self._get_request_id(),
            })

        try:
            client = self._get_http_client()
            response = client.post(self.url, json=batch)
            response.raise_for_status()
            responses = response.json()

            # Map response IDs back to order
            id_to_result: Dict[int, Optional[Any]] = {}
            if isinstance(responses, list):
                for resp in responses:
                    resp_id = resp.get("id")
                    if "error" in resp:
                        logger.warning(f"Batch RPC error (id={resp_id}): {resp['error']}")
                        id_to_result[resp_id] = None
                    else:
                        id_to_result[resp_id] = resp.get("result")

            # Return results in original request order
            return [
                id_to_result.get(req_obj["id"])
                for req_obj in batch
            ]

        except Exception as e:
            logger.error(f"Batch RPC call failed: {e}")
            return [None] * len(requests)

