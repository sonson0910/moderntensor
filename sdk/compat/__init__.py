"""
Luxtensor Layer 1 Blockchain Compatibility Module

This module provides native Luxtensor Layer 1 blockchain data structures.
Pure Luxtensor implementation - no Cardano/pycardano dependencies.
"""

# Re-export all types from luxtensor_types
from sdk.compat.luxtensor_types import *

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
    'PaymentVerificationKey',
    'StakeVerificationKey',
    'ExtendedSigningKey',
    
    # Deprecated Cardano types (for import compatibility)
    'PlutusV3Script',
    'Asset',
    'Value',
    'plutus_script_hash',
    'hash',
    
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
