"""
Luxtensor Layer 1 Blockchain Compatibility Module

This module provides native Luxtensor Layer 1 blockchain data structures.
All Cardano/pycardano references have been completely removed and replaced with pure Luxtensor logic.
"""

from sdk.compat.luxtensor_types import (
    # Native Luxtensor Layer 1 classes
    L1Data,
    L1TransactionData,
    L1TransactionOutput,
    L1ContractAddress,
    L1Address,
    L1Network,
    L1ChainContext,
    L1UTxO,
    Transaction,
    
    # Aliases for backward compatibility during migration
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
    # Native Luxtensor Layer 1 classes
    'L1Data',
    'L1TransactionData',
    'L1TransactionOutput',
    'L1ContractAddress',
    'L1Address',
    'L1Network',
    'L1ChainContext',
    'L1UTxO',
    'Transaction',
    
    # Aliases for backward compatibility during migration
    'PlutusData',
    'Redeemer',
    'Address',
    'TransactionOutput',
    'ScriptHash',
    'Network',
    'BlockFrostChainContext',
    'UTxO',
]
