"""
Luxtensor Token & Address Utilities

Token unit conversion utilities and common address helpers.

Migrated from sdk/utils.py into the sdk/utils/ package to eliminate
the importlib hack that was needed when utils.py coexisted with the
utils/ package directory.
"""

from typing import Union
from decimal import Decimal, ROUND_DOWN

# Token constants
MDT_DECIMALS = 18
MDT_WEI_MULTIPLIER = 10 ** MDT_DECIMALS


def to_mdt(wei: Union[int, str]) -> Decimal:
    """
    Convert base units (wei) to MDT tokens.

    Args:
        wei: Amount in base units (integer or hex string)

    Returns:
        Decimal amount in MDT

    Example:
        >>> to_mdt(1000000000000000000)
        Decimal('1.0')
        >>> to_mdt("0xde0b6b3a7640000")  # 1 MDT in hex
        Decimal('1.0')
    """
    if isinstance(wei, str):
        if wei.startswith("0x"):
            wei = int(wei, 16)
        else:
            wei = int(wei)

    return Decimal(wei) / Decimal(MDT_WEI_MULTIPLIER)


def from_mdt(mdt: Union[int, float, str, Decimal]) -> int:
    """
    Convert MDT tokens to base units (wei).

    Args:
        mdt: Amount in MDT tokens

    Returns:
        Integer amount in base units (wei)

    Example:
        >>> from_mdt(1.0)
        1000000000000000000
        >>> from_mdt("1.5")
        1500000000000000000
        >>> from_mdt(Decimal("0.001"))
        1000000000000000
    """
    if isinstance(mdt, str):
        mdt = Decimal(mdt)
    elif isinstance(mdt, (int, float)):
        mdt = Decimal(str(mdt))

    wei = mdt * Decimal(MDT_WEI_MULTIPLIER)
    return int(wei.quantize(Decimal('1'), rounding=ROUND_DOWN))


def format_mdt(wei: Union[int, str], decimals: int = 4) -> str:
    """
    Format wei amount as human-readable MDT string.

    Args:
        wei: Amount in base units
        decimals: Number of decimal places to show (default: 4)

    Returns:
        Formatted string like "1.2345 MDT"

    Example:
        >>> format_mdt(1234567890000000000)
        '1.2346 MDT'
        >>> format_mdt(1000000000000000000, decimals=2)
        '1.00 MDT'
    """
    mdt = to_mdt(wei)
    formatted = f"{mdt:.{decimals}f}"
    return f"{formatted} MDT"


def validate_address(address: str) -> bool:
    """
    Validate Ethereum-style address format.

    Args:
        address: Address string to validate

    Returns:
        True if valid address format, False otherwise

    Example:
        >>> validate_address("0x1234567890abcdef1234567890abcdef12345678")
        True
        >>> validate_address("invalid")
        False
    """
    if not address:
        return False
    if not address.startswith("0x"):
        return False
    if len(address) != 42:
        return False
    try:
        int(address, 16)
        return True
    except ValueError:
        return False


def shorten_address(address: str, chars: int = 6) -> str:
    """
    Shorten address for display.

    Args:
        address: Full address
        chars: Number of characters to show at start and end

    Returns:
        Shortened address like "0x1234...5678"

    Example:
        >>> shorten_address("0x1234567890abcdef1234567890abcdef12345678")
        '0x1234...5678'
    """
    if len(address) <= chars * 2 + 2:
        return address
    return f"{address[:chars+2]}...{address[-chars:]}"


def shorten_hash(tx_hash: str, chars: int = 8) -> str:
    """
    Shorten transaction hash for display.

    Args:
        tx_hash: Full transaction hash
        chars: Number of characters to show at start

    Returns:
        Shortened hash like "0x12345678..."

    Example:
        >>> shorten_hash("0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef")
        '0x12345678...'
    """
    if len(tx_hash) <= chars + 5:
        return tx_hash
    return f"{tx_hash[:chars+2]}..."


# Export all utilities
__all__ = [
    "MDT_DECIMALS",
    "MDT_WEI_MULTIPLIER",
    "to_mdt",
    "from_mdt",
    "format_mdt",
    "validate_address",
    "shorten_address",
    "shorten_hash",
]
