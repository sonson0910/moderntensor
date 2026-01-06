"""
Layer 1 Blockchain Compatibility Module

This module provides native Layer 1 blockchain data structures.
All Cardano/pycardano references have been replaced with pure Layer 1 logic.
"""

from sdk.compat.pycardano import (
    # Native Layer 1 classes
    L1Data,
    L1TransactionData,
    L1TransactionOutput,
    L1ContractAddress,
    L1Address,
    L1Network,
    L1ChainContext,
    L1UTxO,
    Transaction,
    
    # Aliases for backward compatibility
    PlutusData,
    Redeemer,
    Address,
    TransactionOutput,
    ScriptHash,
    Network,
    BlockFrostChainContext,
    UTxO,
)

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
