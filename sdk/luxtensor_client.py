"""
Luxtensor Python Client

Python client to interact with Luxtensor blockchain via JSON-RPC.
This is equivalent to subtensor.py in Bittensor SDK.
"""

import asyncio
import logging
from typing import Optional, Dict, Any, List, Union
from dataclasses import dataclass
import httpx
import json

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


class LuxtensorClient:
    """
    Synchronous Python client for Luxtensor blockchain.
    
    Provides methods to:
    - Query blockchain state
    - Submit transactions
    - Get account information
    - Query blocks and transactions
    - Network information
    
    Similar to subtensor.py in Bittensor SDK but for Luxtensor.
    """
    
    def __init__(
        self,
        url: str = "http://localhost:9944",
        network: str = "testnet",
        timeout: int = 30,
    ):
        """
        Initialize Luxtensor client.
        
        Args:
            url: Luxtensor RPC endpoint URL
            network: Network name (mainnet, testnet, devnet)
            timeout: Request timeout in seconds
        """
        self.url = url
        self.network = network
        self.timeout = timeout
        self._request_id = 0
        
        logger.info(f"Initialized Luxtensor client for {network} at {url}")
    
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
    
    # ========================================================================
    # Chain Information Methods
    # ========================================================================
    
    def get_chain_info(self) -> ChainInfo:
        """
        Get blockchain information.
        
        Returns:
            ChainInfo object with blockchain metadata
        """
        result = self._call_rpc("chain_getInfo")
        return ChainInfo(**result)
    
    def get_block_number(self) -> int:
        """
        Get current block height.
        
        Returns:
            Current block number
        """
        return self._call_rpc("chain_getBlockNumber")
    
    def get_block(self, block_number: Optional[int] = None) -> Dict[str, Any]:
        """
        Get block by number.
        
        Args:
            block_number: Block number (None for latest)
            
        Returns:
            Block data
        """
        params = [block_number] if block_number is not None else ["latest"]
        return self._call_rpc("chain_getBlock", params)
    
    def get_block_hash(self, block_number: int) -> str:
        """
        Get block hash by number.
        
        Args:
            block_number: Block number
            
        Returns:
            Block hash
        """
        return self._call_rpc("chain_getBlockHash", [block_number])
    
    # ========================================================================
    # Account Methods
    # ========================================================================
    
    def get_account(self, address: str) -> Account:
        """
        Get account information.
        
        Args:
            address: Account address
            
        Returns:
            Account object with balance, nonce, stake
        """
        result = self._call_rpc("state_getAccount", [address])
        return Account(**result)
    
    def get_balance(self, address: str) -> int:
        """
        Get account balance.
        
        Args:
            address: Account address
            
        Returns:
            Balance in smallest unit
        """
        account = self.get_account(address)
        return account.balance
    
    def get_nonce(self, address: str) -> int:
        """
        Get account nonce (transaction count).
        
        Args:
            address: Account address
            
        Returns:
            Current nonce
        """
        account = self.get_account(address)
        return account.nonce
    
    # ========================================================================
    # Transaction Methods
    # ========================================================================
    
    def submit_transaction(self, signed_tx: str) -> TransactionResult:
        """
        Submit signed transaction to Luxtensor.
        
        Args:
            signed_tx: Signed transaction (hex encoded)
            
        Returns:
            TransactionResult with tx_hash and status
        """
        result = self._call_rpc("tx_submit", [signed_tx])
        return TransactionResult(**result)
    
    def get_transaction(self, tx_hash: str) -> Dict[str, Any]:
        """
        Get transaction by hash.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction data
        """
        return self._call_rpc("tx_get", [tx_hash])
    
    def get_transaction_receipt(self, tx_hash: str) -> Dict[str, Any]:
        """
        Get transaction receipt.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Transaction receipt with execution result
        """
        return self._call_rpc("tx_getReceipt", [tx_hash])
    
    # ========================================================================
    # AI/ML Specific Methods
    # ========================================================================
    
    def get_validators(self) -> List[Dict[str, Any]]:
        """
        Get list of active validators.
        
        Returns:
            List of validator information
        """
        return self._call_rpc("validators_getActive")
    
    def get_subnet_info(self, subnet_id: int) -> Dict[str, Any]:
        """
        Get subnet information.
        
        Args:
            subnet_id: Subnet ID
            
        Returns:
            Subnet metadata and configuration
        """
        return self._call_rpc("subnet_getInfo", [subnet_id])
    
    def get_neurons(self, subnet_id: int) -> List[Dict[str, Any]]:
        """
        Get neurons (miners/validators) in subnet.
        
        Args:
            subnet_id: Subnet ID
            
        Returns:
            List of neuron information
        """
        return self._call_rpc("subnet_getNeurons", [subnet_id])
    
    def get_weights(self, subnet_id: int, neuron_uid: int) -> List[float]:
        """
        Get weight matrix for neuron.
        
        Args:
            subnet_id: Subnet ID
            neuron_uid: Neuron UID
            
        Returns:
            Weight values
        """
        return self._call_rpc("subnet_getWeights", [subnet_id, neuron_uid])
    
    # ========================================================================
    # Staking Methods
    # ========================================================================
    
    def get_stake(self, address: str) -> int:
        """
        Get staked amount for address.
        
        Args:
            address: Account address
            
        Returns:
            Staked amount
        """
        account = self.get_account(address)
        return account.stake
    
    def get_total_stake(self) -> int:
        """
        Get total staked in network.
        
        Returns:
            Total stake amount
        """
        return self._call_rpc("staking_getTotalStake")
    
    # ========================================================================
    # Utility Methods
    # ========================================================================
    
    def is_connected(self) -> bool:
        """
        Check if connected to Luxtensor.
        
        Returns:
            True if connected, False otherwise
        """
        try:
            self.get_block_number()
            return True
        except:
            return False
    
    def health_check(self) -> Dict[str, Any]:
        """
        Get node health status.
        
        Returns:
            Health information
        """
        return self._call_rpc("system_health")


class AsyncLuxtensorClient:
    """
    Asynchronous Python client for Luxtensor blockchain.
    
    Provides async methods for high-performance operations:
    - Batch queries
    - Concurrent transaction submission
    - Non-blocking blockchain calls
    
    Similar to async_subtensor.py in Bittensor SDK.
    """
    
    def __init__(
        self,
        url: str = "http://localhost:9944",
        network: str = "testnet",
        timeout: int = 30,
        max_connections: int = 100,
    ):
        """
        Initialize async Luxtensor client.
        
        Args:
            url: Luxtensor RPC endpoint URL
            network: Network name
            timeout: Request timeout in seconds
            max_connections: Max concurrent connections
        """
        self.url = url
        self.network = network
        self.timeout = timeout
        self.max_connections = max_connections
        self._request_id = 0
        
        # Connection pool limits
        self._limits = httpx.Limits(
            max_connections=max_connections,
            max_keepalive_connections=20
        )
        
        logger.info(f"Initialized async Luxtensor client for {network} at {url}")
    
    def _get_request_id(self) -> int:
        """Get next request ID"""
        self._request_id += 1
        return self._request_id
    
    async def _call_rpc(self, method: str, params: Optional[List[Any]] = None) -> Any:
        """
        Make async JSON-RPC call to Luxtensor.
        
        Args:
            method: RPC method name
            params: Method parameters
            
        Returns:
            Result from RPC call
        """
        request = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": self._get_request_id()
        }
        
        async with httpx.AsyncClient(timeout=self.timeout, limits=self._limits) as client:
            try:
                response = await client.post(self.url, json=request)
                response.raise_for_status()
                
                result = response.json()
                
                if "error" in result:
                    raise Exception(f"RPC error: {result['error']}")
                
                return result.get("result")
                
            except httpx.RequestError as e:
                logger.error(f"Request error: {e}")
                raise Exception(f"Failed to connect to Luxtensor at {self.url}: {e}")
    
    async def batch_call(self, calls: List[tuple]) -> List[Any]:
        """
        Execute multiple RPC calls concurrently.
        
        Args:
            calls: List of (method, params) tuples
            
        Returns:
            List of results in same order
        """
        tasks = [self._call_rpc(method, params) for method, params in calls]
        return await asyncio.gather(*tasks)
    
    # Async versions of sync methods
    
    async def get_block_number(self) -> int:
        """Get current block height (async)"""
        return await self._call_rpc("chain_getBlockNumber")
    
    async def get_block(self, block_number: Optional[int] = None) -> Dict[str, Any]:
        """Get block by number (async)"""
        params = [block_number] if block_number is not None else ["latest"]
        return await self._call_rpc("chain_getBlock", params)
    
    async def get_account(self, address: str) -> Account:
        """Get account information (async)"""
        result = await self._call_rpc("state_getAccount", [address])
        return Account(**result)
    
    async def submit_transaction(self, signed_tx: str) -> TransactionResult:
        """Submit transaction (async)"""
        result = await self._call_rpc("tx_submit", [signed_tx])
        return TransactionResult(**result)
    
    async def get_validators(self) -> List[Dict[str, Any]]:
        """Get active validators (async)"""
        return await self._call_rpc("validators_getActive")
    
    async def get_neurons(self, subnet_id: int) -> List[Dict[str, Any]]:
        """Get neurons in subnet (async)"""
        return await self._call_rpc("subnet_getNeurons", [subnet_id])
    
    async def is_connected(self) -> bool:
        """Check connection (async)"""
        try:
            await self.get_block_number()
            return True
        except:
            return False


# Convenience function
def connect(url: str = "http://localhost:9944", network: str = "testnet") -> LuxtensorClient:
    """
    Create and return Luxtensor client.
    
    Args:
        url: Luxtensor RPC URL
        network: Network name
        
    Returns:
        LuxtensorClient instance
    """
    return LuxtensorClient(url=url, network=network)


def async_connect(url: str = "http://localhost:9944", network: str = "testnet") -> AsyncLuxtensorClient:
    """
    Create and return async Luxtensor client.
    
    Args:
        url: Luxtensor RPC URL
        network: Network name
        
    Returns:
        AsyncLuxtensorClient instance
    """
    return AsyncLuxtensorClient(url=url, network=network)
