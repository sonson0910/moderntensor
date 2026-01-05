"""
Legacy Cardano integration layer.

DEPRECATED: This module contains Cardano-specific code that is being phased out
as ModernTensor transitions to an independent Layer 1 blockchain.

For new development, use the Layer 1 primitives in sdk.blockchain and sdk.consensus.

Migration Timeline:
- Current: Dual mode operation (Cardano + L1)
- 3 months: L1 primary, Cardano bridge only
- 6 months: Cardano fully deprecated

See MIGRATION.md for migration guide.
"""
import warnings

warnings.warn(
    "The sdk.legacy.cardano module is deprecated and will be removed in version 1.0. "
    "Please migrate to Layer 1 blockchain primitives.",
    DeprecationWarning,
    stacklevel=2
)
