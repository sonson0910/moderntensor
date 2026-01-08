"""
Compatibility shim for pycardano imports.

This module provides backward compatibility for code that was using pycardano.
All actual implementation is now in luxtensor_types.py which provides native
Luxtensor Layer 1 blockchain data structures.
"""

from sdk.compat.luxtensor_types import (
    # Core types
    L1Address,
    L1Network,
    L1ChainContext,
    L1UTxO,
    L1Data,
    L1TransactionData,
    L1TransactionOutput,
    L1ContractAddress,
    Transaction,
    
    # Verification keys
    PaymentVerificationKey,
    StakeVerificationKey,
    
    # Aliases
    Address,
    Network,
    PlutusData,
    Redeemer,
    TransactionOutput,
    ScriptHash,
    BlockFrostChainContext,
    UTxO,
)

# Add ExtendedSigningKey placeholder
class ExtendedSigningKey:
    """Placeholder for ExtendedSigningKey until full Luxtensor blockchain module implementation."""
    
    def __init__(self, data=None):
        self.data = data
        self.public_key = b"placeholder_public_key"
    
    def derive_from_path(self, path: str):
        """Placeholder derive_from_path method."""
        return ExtendedSigningKey(data=f"derived_{path}")
    
    @staticmethod
    def from_mnemonic(mnemonic: str):
        """Placeholder from_mnemonic method."""
        return ExtendedSigningKey(data=mnemonic)

__all__ = [
    # Core types
    'L1Address',
    'L1Network',
    'L1ChainContext',
    'L1UTxO',
    'L1Data',
    'L1TransactionData',
    'L1TransactionOutput',
    'L1ContractAddress',
    'Transaction',
    
    # Verification keys
    'PaymentVerificationKey',
    'StakeVerificationKey',
    'ExtendedSigningKey',
    
    # Aliases
    'Address',
    'Network',
    'PlutusData',
    'Redeemer',
    'TransactionOutput',
    'ScriptHash',
    'BlockFrostChainContext',
    'UTxO',
]
