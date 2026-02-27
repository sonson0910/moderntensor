"""
ModernTensor SDK Utilities

Token conversion, address helpers, and BPS (basis points) utilities.
"""

# Token conversion & address helpers
from ._token_utils import (
    to_mdt,
    from_mdt,
    format_mdt,
    validate_address,
    shorten_address,
    shorten_hash,
    MDT_DECIMALS,
    MDT_WEI_MULTIPLIER,
)

# BPS (Basis Points) utilities for deterministic consensus arithmetic
from .bps_utils import (
    float_to_bps,
    bps_to_float,
    bps_to_percent,
    percent_to_bps,
    calculate_proportional_share,
    distribute_by_scores,
    validate_bps,
    MAX_BPS,
    BPS_PRECISION,
)

__all__ = [
    # Token conversion
    "to_mdt",
    "from_mdt",
    "format_mdt",
    "validate_address",
    "shorten_address",
    "shorten_hash",
    "MDT_DECIMALS",
    "MDT_WEI_MULTIPLIER",
    # BPS
    "float_to_bps",
    "bps_to_float",
    "bps_to_percent",
    "percent_to_bps",
    "calculate_proportional_share",
    "distribute_by_scores",
    "validate_bps",
    "MAX_BPS",
    "BPS_PRECISION",
]
