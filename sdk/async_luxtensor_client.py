"""
Async Luxtensor Client

Asynchronous blockchain client for ModernTensor network.
Provides non-blocking operations for all blockchain interactions.

Uses httpx.AsyncClient for consistency with the sync BaseClient.

Example::

    async with AsyncLuxtensorClient("http://localhost:8545") as client:
        block = await client.get_block_number()
        neuron = await client.get_neuron(uid=0, netuid=1)
"""

import asyncio
import logging
from typing import Optional, List, Dict, Any

import httpx

from .errors import parse_rpc_error, RpcError


logger = logging.getLogger(__name__)


class AsyncLuxtensorClient:
    """
    Asynchronous client for interacting with the ModernTensor blockchain.

    Provides non-blocking operations for:
    - Blockchain state queries
    - Transaction submission
    - Network information retrieval

    Uses ``httpx.AsyncClient`` internally, consistent with the sync
    :class:`~sdk.client.base.BaseClient`.

    Example::

        async with AsyncLuxtensorClient("http://localhost:8545") as client:
            neuron = await client.get_neuron(uid=0, netuid=1)
            print(f"Neuron stake: {neuron}")
    """

    def __init__(
        self,
        url: str = "http://localhost:8545",
        network: str = "testnet",
        timeout: int = 30,
        max_connections: int = 100,
        retry_attempts: int = 3,
        retry_delay: float = 1.0,
    ):
        """
        Initialize async Luxtensor client.

        Args:
            url: HTTP URL to blockchain RPC node
            network: Network name (mainnet, testnet, devnet)
            timeout: Request timeout in seconds
            max_connections: Maximum number of concurrent connections
            retry_attempts: Number of retry attempts for failed requests
            retry_delay: Base delay between retries in seconds (exponential backoff)
        """
        self.url = url
        self.network = network
        self.timeout = timeout
        self.max_connections = max_connections
        self.retry_attempts = retry_attempts
        self.retry_delay = retry_delay

        self._client: Optional[httpx.AsyncClient] = None
        self._request_counter = 0

        logger.info("Initialized AsyncLuxtensorClient for %s at %s", network, url)

    # =========================================================================
    # Lifecycle
    # =========================================================================

    async def connect(self) -> None:
        """Establish connection (create async HTTP client)."""
        if self._client is None:
            limits = httpx.Limits(
                max_connections=self.max_connections,
                max_keepalive_connections=self.max_connections,
            )
            self._client = httpx.AsyncClient(
                timeout=httpx.Timeout(self.timeout),
                limits=limits,
            )
            logger.info("Async connection established to %s", self.url)

    async def close(self) -> None:
        """Close connection and release resources."""
        if self._client is not None:
            await self._client.aclose()
            self._client = None
            logger.info("Async connection closed")

    async def __aenter__(self) -> "AsyncLuxtensorClient":
        """Async context manager entry."""
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        """Async context manager exit."""
        await self.close()

    # =========================================================================
    # Internal RPC
    # =========================================================================

    def _get_request_id(self) -> int:
        """Get next monotonically increasing request ID."""
        self._request_counter += 1
        return self._request_counter

    async def _call_rpc(
        self,
        method: str,
        params: Optional[List[Any]] = None,
    ) -> Any:
        """
        Make a JSON-RPC 2.0 call with retry logic.

        Args:
            method: RPC method name
            params: Method parameters

        Returns:
            Result from the RPC response

        Raises:
            RpcError: If the server returns a JSON-RPC error
            ConnectionError: If connection fails after all retries
        """
        if self._client is None:
            await self.connect()

        payload = {
            "jsonrpc": "2.0",
            "id": self._get_request_id(),
            "method": method,
            "params": params or [],
        }

        last_exc: Optional[Exception] = None

        for attempt in range(self.retry_attempts):
            try:
                response = await self._client.post(self.url, json=payload)
                response.raise_for_status()
                data = response.json()

                if "error" in data:
                    err = data["error"]
                    # Use structured error parsing from sdk.errors
                    raise parse_rpc_error(err)

                return data.get("result")

            except RpcError:
                raise  # server-side errors are not retryable
            except (httpx.RequestError, httpx.HTTPStatusError) as exc:
                last_exc = exc
                delay = self.retry_delay * (2 ** attempt)
                logger.warning(
                    "Async RPC to %s failed (attempt %d/%d): %s — retrying in %.1fs",
                    self.url, attempt + 1, self.retry_attempts, exc, delay,
                )
                await asyncio.sleep(delay)

        raise ConnectionError(
            f"Failed to connect to {self.url} after "
            f"{self.retry_attempts} attempts: {last_exc}"
        ) from last_exc

    async def _safe_call_rpc(
        self,
        method: str,
        params: Optional[List[Any]] = None,
    ) -> Optional[Any]:
        """
        Safe RPC call that returns None instead of raising on error.

        Suitable for query methods where missing data is not fatal.
        """
        try:
            return await self._call_rpc(method, params)
        except Exception as e:
            logger.debug("Safe RPC call %s failed: %s", method, e)
            return None

    @staticmethod
    def _hex_to_int(value: Any) -> int:
        """Convert hex string to int, handles various input types."""
        if isinstance(value, str):
            return int(value, 16) if value.startswith("0x") else int(value)
        return value if value else 0

    # =========================================================================
    # Blockchain Queries
    # =========================================================================

    async def get_block_number(self) -> int:
        """
        Get current block number.

        Returns:
            Current block number
        """
        result = await self._safe_call_rpc("eth_blockNumber")
        if result is None:
            return 0
        return self._hex_to_int(result)

    async def get_block(self, block_number: Optional[int] = None) -> Optional[Dict[str, Any]]:
        """
        Get block information.

        Args:
            block_number: Block number (None for latest)

        Returns:
            Block data dict or None if not found
        """
        block_param = hex(block_number) if block_number is not None else "latest"
        return await self._safe_call_rpc(
            "eth_getBlockByNumber", [block_param, True]
        )

    async def get_chain_info(self) -> Optional[Dict[str, Any]]:
        """Get chain information (chain ID, network version)."""
        return await self._safe_call_rpc("luxtensor_getChainInfo")

    # =========================================================================
    # Account Queries
    # =========================================================================

    async def get_balance(self, address: str) -> int:
        """
        Get account balance.

        Args:
            address: Account address

        Returns:
            Balance in base units (wei)
        """
        result = await self._safe_call_rpc("eth_getBalance", [address, "latest"])
        if result is None:
            return 0
        return self._hex_to_int(result)

    async def get_nonce(self, address: str) -> int:
        """
        Get account nonce (transaction count).

        Args:
            address: Account address

        Returns:
            Current nonce
        """
        result = await self._safe_call_rpc("eth_getTransactionCount", [address, "latest"])
        if result is None:
            return 0
        return self._hex_to_int(result)

    # =========================================================================
    # Neuron Queries
    # =========================================================================

    async def get_neuron(self, uid: int, netuid: int) -> Optional[Dict[str, Any]]:
        """
        Get neuron information.

        Args:
            uid: Neuron UID
            netuid: Network/subnet UID

        Returns:
            Neuron data dict or None if not found
        """
        return await self._safe_call_rpc("neuron_get", [netuid, uid])

    async def get_neurons(self, netuid: int) -> List[Dict[str, Any]]:
        """
        Get all neurons in a subnet.

        Args:
            netuid: Network/subnet UID

        Returns:
            List of neuron data dicts
        """
        result = await self._safe_call_rpc("neuron_listBySubnet", [netuid])
        return result if result else []

    async def get_neurons_batch(
        self,
        uids: List[int],
        netuid: int,
    ) -> List[Optional[Dict[str, Any]]]:
        """
        Get multiple neurons in parallel (batch operation).

        Args:
            uids: List of neuron UIDs
            netuid: Network/subnet UID

        Returns:
            List of neuron data (None for not found)
        """
        tasks = [self.get_neuron(uid, netuid) for uid in uids]
        return await asyncio.gather(*tasks)

    # =========================================================================
    # Subnet Queries
    # =========================================================================

    async def get_subnet(self, netuid: int) -> Optional[Dict[str, Any]]:
        """
        Get subnet information.

        Args:
            netuid: Network/subnet UID

        Returns:
            Subnet data dict or None if not found
        """
        return await self._safe_call_rpc("subnet_getInfo", [netuid])

    async def get_subnets(self) -> List[Dict[str, Any]]:
        """
        Get all subnets.

        Returns:
            List of subnet data dicts
        """
        result = await self._safe_call_rpc("subnet_getAll")
        return result if result else []

    # =========================================================================
    # Stake Queries
    # =========================================================================

    async def get_stake(self, hotkey: str, coldkey: str) -> int:
        """
        Get stake for a hotkey-coldkey pair.

        Args:
            hotkey: Hotkey address
            coldkey: Coldkey address

        Returns:
            Stake amount in base units
        """
        result = await self._safe_call_rpc(
            "staking_getStakeForPair", [coldkey, hotkey]
        )
        if result is None:
            return 0
        return self._hex_to_int(result)

    async def get_total_stake(self, hotkey: str) -> int:
        """
        Get total stake for a hotkey.

        Args:
            hotkey: Hotkey address

        Returns:
            Total stake amount in base units
        """
        result = await self._safe_call_rpc(
            "staking_getStake", [hotkey]
        )
        if result is None:
            return 0
        return self._hex_to_int(result)

    # =========================================================================
    # Transaction Operations
    # =========================================================================

    async def submit_transaction(self, raw_tx: str) -> Optional[str]:
        """
        Submit a signed transaction.

        Args:
            raw_tx: Hex-encoded signed transaction data

        Returns:
            Transaction hash or None if failed
        """
        try:
            result = await self._call_rpc("eth_sendRawTransaction", [raw_tx])
            return result
        except Exception as e:
            logger.error("Failed to submit transaction: %s", e)
            return None

    async def get_transaction(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction by hash.

        Args:
            tx_hash: Transaction hash

        Returns:
            Transaction data dict or None if not found
        """
        return await self._safe_call_rpc("eth_getTransactionByHash", [tx_hash])

    async def get_transaction_receipt(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction receipt.

        Args:
            tx_hash: Transaction hash

        Returns:
            Receipt data dict or None if not found
        """
        return await self._safe_call_rpc("eth_getTransactionReceipt", [tx_hash])

    # =========================================================================
    # Utility Methods
    # =========================================================================

    async def is_connected(self) -> bool:
        """
        Check if client can reach the blockchain node.

        Returns:
            True if connected, False otherwise
        """
        try:
            await self._call_rpc("eth_blockNumber")
            return True
        except Exception:
            return False

    async def wait_for_block(
        self,
        target_block: int,
        poll_interval: float = 1.0,
        timeout: Optional[float] = None,
    ) -> int:
        """
        Wait for a specific block number.

        Args:
            target_block: Target block number to wait for
            poll_interval: Polling interval in seconds
            timeout: Maximum wait time in seconds (None for indefinite)

        Returns:
            The block number reached

        Raises:
            asyncio.TimeoutError: If timeout is reached
        """
        async def _poll():
            while True:
                current = await self.get_block_number()
                if current >= target_block:
                    return current
                await asyncio.sleep(poll_interval)

        if timeout is not None:
            return await asyncio.wait_for(_poll(), timeout=timeout)
        return await _poll()

    def __repr__(self) -> str:
        return f"AsyncLuxtensorClient(url='{self.url}', network='{self.network}')"
