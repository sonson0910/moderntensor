"""
Backward-compatible wrapper for pycardano interfaces.

This module provides pycardano-like interfaces wrapping Layer 1 primitives
to enable gradual migration from Cardano to the custom Layer 1 blockchain.

As documented in CARDANO_MIGRATION_COMPLETE.md, this compatibility layer
allows existing code (especially metagraph modules) to work without
immediate refactoring while we complete the Layer 1 migration.
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Union
import json


class PlutusData:
    """
    Base class for Plutus datum structures.
    
    In the Layer 1 blockchain, this acts as a simple dataclass
    that can be serialized to JSON for on-chain storage.
    
    Original Cardano/Plutus context:
    - Used for smart contract data structures
    - Serialized using CBOR for on-chain storage
    
    Layer 1 blockchain context:
    - Uses JSON serialization instead of CBOR
    - Stored in the state database
    - Backward compatible interface for existing code
    
    Note: Not using @dataclass decorator to avoid field ordering issues.
    Child classes should use @dataclass and inherit from this.
    """
    
    CONSTR_ID: int = 0  # Plutus constructor ID (preserved for compatibility)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        result = {}
        for key, value in self.__dict__.items():
            if key.startswith('_') or key == 'CONSTR_ID':
                continue
            
            # Handle nested PlutusData
            if isinstance(value, PlutusData):
                result[key] = value.to_dict()
            # Handle lists
            elif isinstance(value, list):
                result[key] = [
                    item.to_dict() if isinstance(item, PlutusData) else item
                    for item in value
                ]
            # Handle bytes
            elif isinstance(value, bytes):
                result[key] = value.hex()
            else:
                result[key] = value
        
        return result
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict())
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'PlutusData':
        """Create instance from dictionary."""
        # Convert hex strings back to bytes for byte fields
        processed_data = {}
        for key, value in data.items():
            if isinstance(value, str) and key.endswith('_hash') or key.endswith('_root'):
                try:
                    processed_data[key] = bytes.fromhex(value)
                except (ValueError, TypeError):
                    processed_data[key] = value
            else:
                processed_data[key] = value
        
        return cls(**processed_data)
    
    @classmethod
    def from_json(cls, json_str: str) -> 'PlutusData':
        """Create instance from JSON string."""
        return cls.from_dict(json.loads(json_str))
    
    def to_cbor(self) -> bytes:
        """
        Compatibility method for CBOR serialization.
        
        In Layer 1 blockchain, we use JSON instead of CBOR,
        so this converts to JSON bytes for now.
        """
        return self.to_json().encode('utf-8')
    
    @classmethod
    def from_cbor(cls, cbor_bytes: bytes) -> 'PlutusData':
        """
        Compatibility method for CBOR deserialization.
        
        In Layer 1 blockchain, we use JSON instead of CBOR,
        so this expects JSON bytes.
        """
        return cls.from_json(cbor_bytes.decode('utf-8'))


@dataclass
class Redeemer:
    """
    Redeemer for Plutus script execution.
    
    In Cardano context:
    - Used to provide arguments to smart contract validators
    - Determines which validation path to execute
    
    In Layer 1 blockchain context:
    - Simplified to just hold data and tag
    - Used for backward compatibility with existing code
    - Actual validation logic is in native Layer 1 contracts
    """
    
    tag: int = 0  # Redeemer tag (e.g., 0=Spend, 1=Mint, 2=Cert, 3=Reward)
    index: int = 0  # Index within the transaction
    data: Any = None  # Redeemer data (can be PlutusData or simple types)
    ex_units: Optional[Dict[str, int]] = None  # Execution units (mem, steps)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        result = {
            'tag': self.tag,
            'index': self.index,
        }
        
        if self.data is not None:
            if isinstance(self.data, PlutusData):
                result['data'] = self.data.to_dict()
            else:
                result['data'] = self.data
        
        if self.ex_units is not None:
            result['ex_units'] = self.ex_units
        
        return result
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict())
    
    def to_cbor(self) -> bytes:
        """Compatibility method - converts to JSON bytes."""
        return self.to_json().encode('utf-8')


# Additional compatibility classes that may be needed

@dataclass  
class Address:
    """
    Simplified address class for Layer 1 blockchain.
    
    In Layer 1, addresses are Ethereum-style 20-byte hex strings.
    This class provides Cardano-like interface for compatibility.
    """
    
    payment_part: bytes = field(default_factory=lambda: b'\x00' * 20)
    staking_part: Optional[bytes] = None
    network: str = "mainnet"
    
    def __str__(self) -> str:
        """Return hex address string."""
        return '0x' + self.payment_part.hex()
    
    @classmethod
    def from_primitive(cls, address_str: str) -> 'Address':
        """Create from address string."""
        if address_str.startswith('0x'):
            address_str = address_str[2:]
        
        try:
            payment_bytes = bytes.fromhex(address_str[:40])  # 20 bytes = 40 hex chars
            return cls(payment_part=payment_bytes)
        except ValueError:
            # Fallback for invalid addresses
            return cls()


@dataclass
class TransactionOutput:
    """
    Transaction output for Layer 1 blockchain.
    
    Provides Cardano UTXO-like interface for compatibility.
    In Layer 1, this maps to account-based transactions.
    """
    
    address: Address
    amount: int = 0
    datum: Optional[PlutusData] = None
    datum_hash: Optional[bytes] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        result = {
            'address': str(self.address),
            'amount': self.amount,
        }
        
        if self.datum is not None:
            result['datum'] = self.datum.to_dict()
        
        if self.datum_hash is not None:
            result['datum_hash'] = self.datum_hash.hex()
        
        return result


class BlockFrostChainContext:
    """
    Compatibility stub for BlockFrost chain context.
    
    In the original Cardano integration, this was used to interact with
    Blockfrost API for querying blockchain data.
    
    In Layer 1 blockchain, this is replaced by L1ChainContext (JSON-RPC).
    This stub exists only for backward compatibility during migration.
    """
    
    def __init__(self, project_id: str = "", network: str = "mainnet", base_url: Optional[str] = None):
        """Initialize stub context."""
        self.project_id = project_id
        self.network = network
        self.base_url = base_url
        
        # For Layer 1 migration, redirect to L1ChainContext if available
        try:
            from sdk.blockchain.l1_context import L1ChainContext
            self._l1_context = L1ChainContext()
        except ImportError:
            self._l1_context = None
    
    def utxos(self, address: str) -> List[Any]:
        """
        Get UTXOs for an address (compatibility method).
        
        In Layer 1, this would query account balance instead.
        """
        if self._l1_context:
            # Delegate to L1 context if available
            try:
                balance = self._l1_context.get_balance(address)
                # Return empty list for now (UTXO model not used in L1)
                return []
            except Exception:
                pass
        
        return []
    
    def submit_tx(self, tx_cbor: bytes) -> str:
        """
        Submit transaction (compatibility method).
        
        In Layer 1, transactions are submitted via JSON-RPC.
        """
        if self._l1_context:
            try:
                # In real implementation, would convert tx_cbor to L1 transaction
                # For now, return placeholder
                return "0x" + "0" * 64
            except Exception:
                pass
        
        raise NotImplementedError("BlockFrost compatibility layer - use L1ChainContext instead")


class Network:
    """
    Network type compatibility class.
    
    In Cardano: MAINNET, TESTNET, etc.
    In Layer 1: Similar network types
    """
    
    MAINNET = "mainnet"
    TESTNET = "testnet"
    DEVNET = "devnet"
    
    def __init__(self, name: str = "mainnet"):
        self.name = name
    
    def __str__(self) -> str:
        return self.name


class ScriptHash:
    """
    Script hash compatibility class.
    
    In Cardano, script hashes identify smart contracts.
    In Layer 1, contract addresses are used instead.
    """
    
    def __init__(self, hash_bytes: bytes):
        self.payload = hash_bytes
    
    def __str__(self) -> str:
        return self.payload.hex()
    
    def __bytes__(self) -> bytes:
        return self.payload
    
    @classmethod
    def from_primitive(cls, value: Union[str, bytes]) -> 'ScriptHash':
        """Create from hex string or bytes."""
        if isinstance(value, str):
            return cls(bytes.fromhex(value))
        return cls(value)


class UTxO:
    """
    UTXO compatibility class.
    
    In Layer 1 blockchain (account-based), UTXOs don't exist.
    This is a compatibility stub.
    """
    
    def __init__(self, input_ref: Any = None, output: Any = None):
        self.input = input_ref
        self.output = output


# Export compatibility types
__all__ = [
    'PlutusData',
    'Redeemer', 
    'Address',
    'TransactionOutput',
    'BlockFrostChainContext',
    'Network',
    'ScriptHash',
    'UTxO',
]
