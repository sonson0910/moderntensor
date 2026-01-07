"""
Async Luxtensor Client

Asynchronous blockchain client for ModernTensor network.
Provides non-blocking operations for all blockchain interactions.
"""

import asyncio
import logging
from typing import Optional, List, Dict, Any, Union
from contextlib import asynccontextmanager

import aiohttp
from aiohttp import ClientSession, ClientTimeout

from sdk.models import (
    NeuronInfo,
    SubnetInfo,
    StakeInfo,
    ValidatorInfo,
    MinerInfo,
    BlockInfo,
    TransactionInfo,
)


logger = logging.getLogger(__name__)


class AsyncLuxtensorClient:
    """
    Asynchronous client for interacting with the ModernTensor blockchain.
    
    Provides non-blocking operations for:
    - Blockchain state queries
    - Transaction submission
    - Network information retrieval
    - Real-time updates
    
    Example:
        ```python
        async with AsyncLuxtensorClient("ws://localhost:9944") as client:
            neuron = await client.get_neuron(uid=0, netuid=1)
            print(f"Neuron stake: {neuron.stake}")
        ```
    """
    
    def __init__(
        self,
        url: str,
        timeout: int = 30,
        max_connections: int = 100,
        retry_attempts: int = 3,
        retry_delay: float = 1.0,
    ):
        """
        Initialize async Luxtensor client.
        
        Args:
            url: WebSocket or HTTP URL to blockchain node
            timeout: Request timeout in seconds
            max_connections: Maximum number of concurrent connections
            retry_attempts: Number of retry attempts for failed requests
            retry_delay: Delay between retries in seconds
        """
        self.url = url
        self.timeout = ClientTimeout(total=timeout)
        self.max_connections = max_connections
        self.retry_attempts = retry_attempts
        self.retry_delay = retry_delay
        
        self._session: Optional[ClientSession] = None
        self._connection_pool_size = max_connections
        self._request_counter = 0
        
        logger.info(f"Initialized AsyncLuxtensorClient with URL: {url}")
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()
    
    async def connect(self):
        """Establish connection to blockchain node."""
        if self._session is None:
            connector = aiohttp.TCPConnector(
                limit=self.max_connections,
                limit_per_host=self.max_connections,
            )
            self._session = ClientSession(
                connector=connector,
                timeout=self.timeout,
            )
            logger.info("Connection established")
    
    async def close(self):
        """Close connection to blockchain node."""
        if self._session:
            await self._session.close()
            self._session = None
            logger.info("Connection closed")
    
    async def _make_request(
        self,
        method: str,
        params: Optional[List[Any]] = None,
    ) -> Dict[str, Any]:
        """
        Make RPC request with retry logic.
        
        Args:
            method: RPC method name
            params: Method parameters
            
        Returns:
            Response data
            
        Raises:
            ConnectionError: If connection fails after retries
            ValueError: If response is invalid
        """
        if not self._session:
            await self.connect()
        
        self._request_counter += 1
        request_id = self._request_counter
        
        payload = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params or [],
        }
        
        for attempt in range(self.retry_attempts):
            try:
                async with self._session.post(self.url, json=payload) as response:
                    response.raise_for_status()
                    data = await response.json()
                    
                    if "error" in data:
                        raise ValueError(f"RPC error: {data['error']}")
                    
                    return data.get("result", {})
                    
            except (aiohttp.ClientError, asyncio.TimeoutError) as e:
                if attempt < self.retry_attempts - 1:
                    logger.warning(
                        f"Request failed (attempt {attempt + 1}/{self.retry_attempts}): {e}"
                    )
                    await asyncio.sleep(self.retry_delay * (attempt + 1))
                else:
                    logger.error(f"Request failed after {self.retry_attempts} attempts")
                    raise ConnectionError(f"Failed to connect to {self.url}") from e
        
        raise ConnectionError("Unexpected error in request")
    
    # =============================================================================
    # Neuron Queries
    # =============================================================================
    
    async def get_neuron(self, uid: int, netuid: int) -> Optional[NeuronInfo]:
        """
        Get neuron information.
        
        Args:
            uid: Neuron UID
            netuid: Network/subnet UID
            
        Returns:
            NeuronInfo object or None if not found
        """
        try:
            result = await self._make_request(
                "neuron_info",
                params=[netuid, uid]
            )
            return NeuronInfo(**result) if result else None
        except Exception as e:
            logger.error(f"Error getting neuron {uid} on netuid {netuid}: {e}")
            return None
    
    async def get_neurons(self, netuid: int) -> List[NeuronInfo]:
        """
        Get all neurons in a subnet.
        
        Args:
            netuid: Network/subnet UID
            
        Returns:
            List of NeuronInfo objects
        """
        try:
            result = await self._make_request(
                "neurons",
                params=[netuid]
            )
            return [NeuronInfo(**n) for n in result] if result else []
        except Exception as e:
            logger.error(f"Error getting neurons for netuid {netuid}: {e}")
            return []
    
    async def get_neurons_batch(
        self,
        uids: List[int],
        netuid: int
    ) -> List[Optional[NeuronInfo]]:
        """
        Get multiple neurons in parallel (batch operation).
        
        Args:
            uids: List of neuron UIDs
            netuid: Network/subnet UID
            
        Returns:
            List of NeuronInfo objects (None for not found)
        """
        tasks = [self.get_neuron(uid, netuid) for uid in uids]
        return await asyncio.gather(*tasks)
    
    # =============================================================================
    # Subnet Queries
    # =============================================================================
    
    async def get_subnet(self, netuid: int) -> Optional[SubnetInfo]:
        """
        Get subnet information.
        
        Args:
            netuid: Network/subnet UID
            
        Returns:
            SubnetInfo object or None if not found
        """
        try:
            result = await self._make_request(
                "subnet_info",
                params=[netuid]
            )
            return SubnetInfo(**result) if result else None
        except Exception as e:
            logger.error(f"Error getting subnet {netuid}: {e}")
            return None
    
    async def get_subnets(self) -> List[SubnetInfo]:
        """
        Get all subnets.
        
        Returns:
            List of SubnetInfo objects
        """
        try:
            result = await self._make_request("subnets")
            return [SubnetInfo(**s) for s in result] if result else []
        except Exception as e:
            logger.error(f"Error getting subnets: {e}")
            return []
    
    # =============================================================================
    # Stake Queries
    # =============================================================================
    
    async def get_stake(self, hotkey: str, coldkey: str) -> Optional[StakeInfo]:
        """
        Get stake information for a hotkey-coldkey pair.
        
        Args:
            hotkey: Hotkey address
            coldkey: Coldkey address
            
        Returns:
            StakeInfo object or None if not found
        """
        try:
            result = await self._make_request(
                "stake_info",
                params=[hotkey, coldkey]
            )
            return StakeInfo(**result) if result else None
        except Exception as e:
            logger.error(f"Error getting stake for {hotkey}: {e}")
            return None
    
    async def get_total_stake(self, hotkey: str) -> float:
        """
        Get total stake for a hotkey.
        
        Args:
            hotkey: Hotkey address
            
        Returns:
            Total stake amount
        """
        try:
            result = await self._make_request(
                "total_stake",
                params=[hotkey]
            )
            return float(result) if result else 0.0
        except Exception as e:
            logger.error(f"Error getting total stake for {hotkey}: {e}")
            return 0.0
    
    # =============================================================================
    # Block Queries
    # =============================================================================
    
    async def get_block(self, block_number: Optional[int] = None) -> Optional[BlockInfo]:
        """
        Get block information.
        
        Args:
            block_number: Block number (None for latest)
            
        Returns:
            BlockInfo object or None if not found
        """
        try:
            result = await self._make_request(
                "block_info",
                params=[block_number] if block_number else []
            )
            return BlockInfo(**result) if result else None
        except Exception as e:
            logger.error(f"Error getting block {block_number}: {e}")
            return None
    
    async def get_block_number(self) -> int:
        """
        Get current block number.
        
        Returns:
            Current block number
        """
        try:
            result = await self._make_request("block_number")
            return int(result) if result else 0
        except Exception as e:
            logger.error(f"Error getting block number: {e}")
            return 0
    
    # =============================================================================
    # Transaction Operations
    # =============================================================================
    
    async def submit_transaction(
        self,
        tx_data: Dict[str, Any],
    ) -> Optional[str]:
        """
        Submit a transaction to the blockchain.
        
        Args:
            tx_data: Transaction data
            
        Returns:
            Transaction hash or None if failed
        """
        try:
            result = await self._make_request(
                "submit_transaction",
                params=[tx_data]
            )
            return result.get("tx_hash") if result else None
        except Exception as e:
            logger.error(f"Error submitting transaction: {e}")
            return None
    
    async def get_transaction(self, tx_hash: str) -> Optional[TransactionInfo]:
        """
        Get transaction information.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            TransactionInfo object or None if not found
        """
        try:
            result = await self._make_request(
                "transaction_info",
                params=[tx_hash]
            )
            return TransactionInfo(**result) if result else None
        except Exception as e:
            logger.error(f"Error getting transaction {tx_hash}: {e}")
            return None
    
    # =============================================================================
    # Utility Methods
    # =============================================================================
    
    async def is_connected(self) -> bool:
        """
        Check if client is connected to blockchain.
        
        Returns:
            True if connected, False otherwise
        """
        try:
            await self.get_block_number()
            return True
        except Exception:
            return False
    
    async def wait_for_block(self, target_block: int, poll_interval: float = 1.0):
        """
        Wait for a specific block number.
        
        Args:
            target_block: Target block number to wait for
            poll_interval: Polling interval in seconds
        """
        while True:
            current_block = await self.get_block_number()
            if current_block >= target_block:
                break
            await asyncio.sleep(poll_interval)
    
    def __repr__(self) -> str:
        return f"AsyncLuxtensorClient(url='{self.url}')"
