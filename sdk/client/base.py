"""
LuxtensorClient Base Module

Core RPC functionality and data classes.
All mixins inherit from this base.
"""

import logging
from typing import Optional, List, Any
from dataclasses import dataclass
import httpx

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
    """Account information from Luxtensor"""
    address: str
    balance: int
    nonce: int
    stake: int = 0


@dataclass
class TransactionResult:
    """Transaction submission result"""
    tx_hash: str
    status: str
    block_number: Optional[int] = None
    error: Optional[str] = None


class BaseClient:
    """
    Base client with core RPC functionality.
    All mixin classes expect these methods to be available.
    """

    url: str
    network: str
    timeout: int
    _request_id: int

    def _get_request_id(self) -> int:
        """Get next request ID"""
        self._request_id += 1
        return self._request_id

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
            with httpx.Client(timeout=self.timeout) as client:
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
