"""
ModernTensor SDK Utilities

Re-exports token conversion utilities from the parent utils module
and BPS (basis points) utilities.
"""

import importlib
import sys
from pathlib import Path

# Import the sibling utils.py module directly (it's shadowed by this package)
_utils_file = Path(__file__).parent.parent / "utils.py"
_spec = importlib.util.spec_from_file_location("sdk._utils_module", _utils_file)
_utils_mod = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_utils_mod)

# Re-export token conversion utilities
to_mdt = _utils_mod.to_mdt
from_mdt = _utils_mod.from_mdt
format_mdt = _utils_mod.format_mdt
validate_address = _utils_mod.validate_address
shorten_address = _utils_mod.shorten_address
shorten_hash = _utils_mod.shorten_hash
MDT_DECIMALS = _utils_mod.MDT_DECIMALS
MDT_WEI_MULTIPLIER = _utils_mod.MDT_WEI_MULTIPLIER

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
