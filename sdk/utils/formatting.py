"""
Formatting Utilities

Helper functions for formatting blockchain data for display.
"""

from datetime import datetime
from typing import Optional


def format_stake(stake: float, decimals: int = 2) -> str:
    """
    Format stake amount for display.
    
    Args:
        stake: Stake amount
        decimals: Number of decimal places
        
    Returns:
        Formatted stake string
        
    Example:
        ```python
        from sdk.utils import format_stake
        
        formatted = format_stake(1234.56789)
        print(formatted)  # "1,234.57 MTAO"
        ```
    """
    formatted_num = f"{stake:,.{decimals}f}"
    return f"{formatted_num} MTAO"


def format_emission(emission: float, per_block: bool = True) -> str:
    """
    Format emission rate for display.
    
    Args:
        emission: Emission rate
        per_block: If True, show "per block", else "per epoch"
        
    Returns:
        Formatted emission string
        
    Example:
        ```python
        from sdk.utils import format_emission
        
        formatted = format_emission(0.123456)
        print(formatted)  # "0.12 MTAO/block"
        ```
    """
    unit = "block" if per_block else "epoch"
    return f"{emission:.2f} MTAO/{unit}"


def format_timestamp(
    timestamp: int,
    format_str: str = "%Y-%m-%d %H:%M:%S"
) -> str:
    """
    Format Unix timestamp for display.
    
    Args:
        timestamp: Unix timestamp (seconds since epoch)
        format_str: strftime format string
        
    Returns:
        Formatted timestamp string
        
    Example:
        ```python
        from sdk.utils import format_timestamp
        
        formatted = format_timestamp(1704801600)
        print(formatted)  # "2024-01-09 12:00:00"
        ```
    """
    dt = datetime.fromtimestamp(timestamp)
    return dt.strftime(format_str)


__all__ = ["format_stake", "format_emission", "format_timestamp"]
