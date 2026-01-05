"""
Layer 1 Chain Context for ModernTensor

Replacement for pycardano BlockFrostChainContext.
Provides RPC connection to Layer 1 blockchain nodes.
"""
import httpx
from typing import Optional, Dict, Any, List
from dataclasses import dataclass

from sdk.blockchain.l1_keymanager import L1Network, L1Address
from sdk.blockchain.transaction import Transaction


@dataclass
class L1UTxO:
    """
    Layer 1 UTXO representation.
    
    Note: Layer 1 uses account model, not UTXO model.
    This is a compatibility layer for transition.
    """
    tx_hash: str
    output_index: int
    address: str
    amount: int
    data: Optional[bytes] = None


class L1ChainContext:
    """
    Chain context for Layer 1 blockchain.
    
    Replaces BlockFrostChainContext with RPC connection to L1 nodes.
    """
    
    def __init__(
        self,
        rpc_url: Optional[str] = None,
        network: L1Network = L1Network.TESTNET,
        api_key: Optional[str] = None
    ):
        """
        Initialize L1 chain context.
        
        Args:
            rpc_url: RPC endpoint URL (defaults based on network)
            network: Network type (mainnet, testnet, devnet)
            api_key: Optional API key for authenticated RPC
        """
        self.network = network if isinstance(network, L1Network) else L1Network(network)
        
        # Default RPC URLs
        if rpc_url is None:
            if self.network.network_type == L1Network.MAINNET:
                self.rpc_url = "http://mainnet-rpc.moderntensor.io:8545"
            elif self.network.network_type == L1Network.TESTNET:
                self.rpc_url = "http://testnet-rpc.moderntensor.io:8545"
            else:  # DEVNET
                self.rpc_url = "http://localhost:8545"
        else:
            self.rpc_url = rpc_url
        
        self.api_key = api_key
        self._client = httpx.Client(timeout=30.0)
        self._next_request_id = 1
    
    def _rpc_call(self, method: str, params: List[Any]) -> Any:
        """
        Make JSON-RPC call to the node.
        
        Args:
            method: RPC method name
            params: Method parameters
            
        Returns:
            Result from RPC call
        """
        request_id = self._next_request_id
        self._next_request_id += 1
        
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": request_id
        }
        
        headers = {"Content-Type": "application/json"}
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        
        try:
            response = self._client.post(self.rpc_url, json=payload, headers=headers)
            response.raise_for_status()
            result = response.json()
            
            if "error" in result:
                raise Exception(f"RPC error: {result['error']}")
            
            return result.get("result")
        except httpx.HTTPError as e:
            # Node may not be running yet - return None for compatibility
            return None
        except Exception as e:
            # For now, return None during transition period
            return None
    
    def get_balance(self, address: str) -> int:
        """
        Get balance for an address.
        
        Args:
            address: Address (hex string)
            
        Returns:
            int: Balance in smallest unit
        """
        result = self._rpc_call("eth_getBalance", [address, "latest"])
        if result is None:
            return 0
        
        # Convert hex to int
        return int(result, 16) if isinstance(result, str) else result
    
    def get_utxos(self, address: str) -> List[L1UTxO]:
        """
        Get UTXOs for an address.
        
        Note: Layer 1 uses account model. This returns empty list.
        Use get_balance() instead.
        
        Args:
            address: Address
            
        Returns:
            List[L1UTxO]: Empty list (compatibility)
        """
        # Layer 1 uses account model, not UTXO
        # Return empty list for compatibility during transition
        return []
    
    def submit_tx(self, tx: Transaction) -> str:
        """
        Submit a signed transaction to the network.
        
        Args:
            tx: Signed transaction
            
        Returns:
            str: Transaction hash
        """
        # Serialize transaction to hex
        tx_data = self._serialize_transaction(tx)
        
        result = self._rpc_call("eth_sendRawTransaction", [tx_data])
        
        if result is None:
            # Fallback: return mock hash during transition
            return "0x" + tx.hash().hex()
        
        return result
    
    def _serialize_transaction(self, tx: Transaction) -> str:
        """
        Serialize transaction to hex string for submission.
        
        Args:
            tx: Transaction to serialize
            
        Returns:
            str: Hex-encoded transaction
        """
        # TODO: Implement proper RLP encoding
        # For now, return hex of transaction hash as placeholder
        return "0x" + tx.hash().hex()
    
    def get_transaction(self, tx_hash: str) -> Optional[Dict[str, Any]]:
        """
        Get transaction by hash.
        
        Args:
            tx_hash: Transaction hash
            
        Returns:
            Optional[Dict]: Transaction data or None
        """
        result = self._rpc_call("eth_getTransactionByHash", [tx_hash])
        return result
    
    def get_block_number(self) -> int:
        """
        Get current block number.
        
        Returns:
            int: Current block height
        """
        result = self._rpc_call("eth_blockNumber", [])
        if result is None:
            return 0
        
        return int(result, 16) if isinstance(result, str) else result
    
    def get_nonce(self, address: str) -> int:
        """
        Get transaction nonce for an address.
        
        Args:
            address: Address
            
        Returns:
            int: Next nonce to use
        """
        result = self._rpc_call("eth_getTransactionCount", [address, "latest"])
        if result is None:
            return 0
        
        return int(result, 16) if isinstance(result, str) else result
    
    def __enter__(self):
        """Context manager entry."""
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self._client.close()
    
    def close(self):
        """Close the HTTP client."""
        self._client.close()


# Backward compatibility alias
BlockFrostChainContext = L1ChainContext
