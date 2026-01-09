"""
Balance Utilities

Helper functions for balance operations and address validation.
"""

import re
from typing import Optional


def format_balance(balance: float, decimals: int = 9, symbol: str = "MTAO") -> str:
    """
    Format balance with proper decimals and symbol.
    
    Args:
        balance: Balance amount
        decimals: Number of decimal places
        symbol: Token symbol
        
    Returns:
        Formatted balance string
        
    Example:
        ```python
        from sdk.utils import format_balance
        
        formatted = format_balance(1000.123456789)
        print(formatted)  # "1,000.123456789 MTAO"
        ```
    """
    formatted_num = f"{balance:,.{decimals}f}".rstrip('0').rstrip('.')
    return f"{formatted_num} {symbol}"


def convert_balance(
    balance: float,
    from_unit: str = "MTAO",
    to_unit: str = "RAO"
) -> float:
    """
    Convert balance between different units.
    
    Args:
        balance: Balance amount
        from_unit: Source unit (MTAO, RAO)
        to_unit: Target unit (MTAO, RAO)
        
    Returns:
        Converted balance
        
    Example:
        ```python
        from sdk.utils import convert_balance
        
        # Convert 1 MTAO to RAO
        rao = convert_balance(1.0, from_unit="MTAO", to_unit="RAO")
        print(rao)  # 1000000000.0
        ```
    """
    # Conversion factors (1 MTAO = 10^9 RAO)
    units = {
        "RAO": 1,
        "MTAO": 1_000_000_000,
    }
    
    if from_unit not in units or to_unit not in units:
        raise ValueError(f"Invalid unit. Use: {list(units.keys())}")
    
    # Convert to base unit (RAO) then to target unit
    rao_amount = balance * units[from_unit]
    return rao_amount / units[to_unit]


def validate_address(address: str) -> bool:
    """
    Validate a blockchain address format.
    
    Args:
        address: Address string to validate
        
    Returns:
        True if valid, False otherwise
        
    Example:
        ```python
        from sdk.utils import validate_address
        
        is_valid = validate_address("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY")
        print(is_valid)  # True
        ```
    """
    # Substrate-style SS58 address validation
    # Starts with alphanumeric, length 46-48
    if not address or not isinstance(address, str):
        return False
    
    pattern = r'^[1-9A-HJ-NP-Za-km-z]{46,48}$'
    return bool(re.match(pattern, address))


__all__ = ["format_balance", "convert_balance", "validate_address"]
