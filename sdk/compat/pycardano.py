"""
Layer 1 Blockchain Data Structures

This module provides native Layer 1 blockchain data structures for ModernTensor.
Pure Layer 1 implementation - no Cardano/pycardano references.

These classes are used for on-chain data storage and serialization in the
ModernTensor custom blockchain.
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Union
import json

# Import Layer 1 primitives
# Temporarily commented out to fix missing module issue
# from sdk.blockchain.l1_keymanager import L1Address, L1Network
# from sdk.blockchain.l1_context import L1ChainContext, L1UTxO
# from sdk.blockchain.transaction import Transaction

# Placeholder types until blockchain module is created
class L1Address:
    """Placeholder for L1Address until blockchain module implementation."""
    pass

class L1Network:
    """Placeholder for L1Network until blockchain module implementation."""
    TESTNET = "testnet"
    MAINNET = "mainnet"

class L1ChainContext:
    """Placeholder for L1ChainContext until blockchain module implementation."""
    pass

class L1UTxO:
    """Placeholder for L1UTxO until blockchain module implementation."""
    pass

class Transaction:
    """Placeholder for Transaction until blockchain module implementation."""
    pass

# Placeholder verification key classes
class PaymentVerificationKey:
    """Placeholder for PaymentVerificationKey until blockchain module implementation."""
    @staticmethod
    def from_primitive(data):
        return PaymentVerificationKey()
    
    def hash(self):
        return b"payment_hash"

class StakeVerificationKey:
    """Placeholder for StakeVerificationKey until blockchain module implementation."""
    @staticmethod
    def from_primitive(data):
        return StakeVerificationKey()
    
    def hash(self):
        return b"stake_hash"

# Re-export common names
Address = L1Address
Network = L1Network


class L1Data:
    """
    Base class for Layer 1 blockchain data structures.
    
    Used for on-chain data storage in ModernTensor Layer 1 blockchain.
    - JSON serialization for state database storage
    - Simple dataclass pattern
    - No legacy Cardano/Plutus constructs
    
    Note: Not using @dataclass decorator to avoid field ordering issues.
    Child classes should use @dataclass and inherit from this.
    """
    
    CONSTR_ID: int = 0  # Constructor ID for data versioning
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        result = {}
        for key, value in self.__dict__.items():
            if key.startswith('_') or key == 'CONSTR_ID':
                continue
            
            # Handle nested L1Data
            if isinstance(value, L1Data):
                result[key] = value.to_dict()
            # Handle lists
            elif isinstance(value, list):
                result[key] = [
                    item.to_dict() if isinstance(item, L1Data) else item
                    for item in value
                ]
            # Handle bytes
            elif isinstance(value, bytes):
                result[key] = value.hex()
            else:
                result[key] = value
        
        return result
    
    def to_json(self) -> str:
        """Convert to JSON string for Layer 1 storage."""
        return json.dumps(self.to_dict())
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'L1Data':
        """Create instance from dictionary."""
        # Convert hex strings back to bytes for byte fields
        processed_data = {}
        for key, value in data.items():
            if isinstance(value, str) and (key.endswith('_hash') or key.endswith('_root')):
                try:
                    processed_data[key] = bytes.fromhex(value)
                except (ValueError, TypeError):
                    processed_data[key] = value
            else:
                processed_data[key] = value
        
        return cls(**processed_data)
    
    @classmethod
    def from_json(cls, json_str: str) -> 'L1Data':
        """Create instance from JSON string."""
        return cls.from_dict(json.loads(json_str))


@dataclass
class L1TransactionData:
    """
    Layer 1 transaction additional data.
    
    Used for contract calls and data payloads in transactions.
    """
    
    tag: int = 0  # Data tag for categorization
    index: int = 0  # Index for ordering
    data: Any = None  # Payload data (can be L1Data or simple types)
    gas_limit: Optional[int] = None  # Gas limit for execution
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        result = {
            'tag': self.tag,
            'index': self.index,
        }
        
        if self.data is not None:
            if isinstance(self.data, L1Data):
                result['data'] = self.data.to_dict()
            else:
                result['data'] = self.data
        
        if self.gas_limit is not None:
            result['gas_limit'] = self.gas_limit
        
        return result
    
    def to_json(self) -> str:
        """Convert to JSON string."""
        return json.dumps(self.to_dict())


@dataclass
class L1TransactionOutput:
    """
    Layer 1 transaction output.
    
    Represents destination and amount in account-based model.
    """
    
    address: L1Address
    amount: int = 0
    data: Optional[L1Data] = None
    data_hash: Optional[bytes] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary."""
        result = {
            'address': str(self.address),
            'amount': self.amount,
        }
        
        if self.data is not None:
            result['data'] = self.data.to_dict()
        
        if self.data_hash is not None:
            result['data_hash'] = self.data_hash.hex()
        
        return result


@dataclass  
class L1ContractAddress:
    """
    Layer 1 contract address.
    
    Identifies smart contracts on Layer 1 blockchain.
    """
    
    address: bytes
    
    def __str__(self) -> str:
        return '0x' + self.address.hex()
    
    def __bytes__(self) -> bytes:
        return self.address
    
    @classmethod
    def from_hex(cls, hex_str: str) -> 'L1ContractAddress':
        """Create from hex string."""
        if hex_str.startswith('0x'):
            hex_str = hex_str[2:]
        return cls(address=bytes.fromhex(hex_str))
    
    @classmethod
    def from_primitive(cls, value: Union[str, bytes]) -> 'L1ContractAddress':
        """Create from hex string or bytes."""
        if isinstance(value, str):
            return cls.from_hex(value)
        return cls(address=value)


# Aliases for backward compatibility during transition
# These allow existing code to use old names while we migrate
PlutusData = L1Data  # Alias for gradual migration
Redeemer = L1TransactionData  # Alias for gradual migration
Address = L1Address  # Use native Layer 1 address
TransactionOutput = L1TransactionOutput  # Use native Layer 1 output
ScriptHash = L1ContractAddress  # Contract address alias
Network = L1Network  # Use native Layer 1 network
BlockFrostChainContext = L1ChainContext  # Use native Layer 1 context
UTxO = L1UTxO  # Use native Layer 1 UTXO (account model)


# Export all classes
__all__ = [
    # Native Layer 1 classes
    'L1Data',
    'L1TransactionData',
    'L1TransactionOutput',
    'L1ContractAddress',
    'L1Address',
    'L1Network',
    'L1ChainContext',
    'L1UTxO',
    'Transaction',
    
    # Aliases for backward compatibility
    'PlutusData',
    'Redeemer',
    'Address',
    'TransactionOutput',
    'ScriptHash',
    'Network',
    'BlockFrostChainContext',
    'UTxO',
]
