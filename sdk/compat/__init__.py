"""
Compatibility layer for ModernTensor Layer 1 blockchain.

This module provides backward compatibility for code that was written
for Cardano integration but needs to work with the custom Layer 1 blockchain.
"""

from sdk.compat.pycardano import PlutusData, Redeemer

__all__ = ['PlutusData', 'Redeemer']
