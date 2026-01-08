"""
Balance Utilities Module

Provides utilities for token calculations, balance formatting, and unit conversions.
Supports TAO token operations with RAO (smallest unit) conversions.
"""

from decimal import Decimal, ROUND_DOWN, InvalidOperation
from typing import Union, Optional
import re


# Constants
RAO_PER_TAO = 10**9  # 1 TAO = 1,000,000,000 RAO
MIN_BALANCE = 0
MAX_BALANCE = 21_000_000 * RAO_PER_TAO  # Maximum supply in RAO


class BalanceError(Exception):
    """Exception raised for balance-related errors."""
    pass


class Balance:
    """
    Represents a token balance with precise decimal arithmetic.
    
    Internally stores balance in RAO (smallest unit) to avoid floating point errors.
    Provides methods for arithmetic operations and conversions.
    
    Examples:
        >>> balance = Balance.from_tao(100.5)
        >>> print(balance.tao)
        100.5
        >>> print(balance.rao)
        100500000000
        
        >>> b1 = Balance.from_tao(50)
        >>> b2 = Balance.from_tao(25.5)
        >>> total = b1 + b2
        >>> print(total.tao)
        75.5
    """
    
    def __init__(self, rao: int = 0):
        """
        Initialize Balance with RAO amount.
        
        Args:
            rao: Amount in RAO (smallest unit)
            
        Raises:
            BalanceError: If rao is negative or exceeds maximum supply
        """
        if rao < MIN_BALANCE:
            raise BalanceError(f"Balance cannot be negative: {rao}")
        if rao > MAX_BALANCE:
            raise BalanceError(f"Balance exceeds maximum supply: {rao}")
        self._rao = int(rao)
    
    @classmethod
    def from_tao(cls, tao: Union[float, str, Decimal]) -> "Balance":
        """
        Create Balance from TAO amount.
        
        Args:
            tao: Amount in TAO
            
        Returns:
            Balance instance
            
        Examples:
            >>> Balance.from_tao(1.5)
            Balance(1500000000 RAO)
            >>> Balance.from_tao("0.000000001")
            Balance(1 RAO)
        """
        try:
            decimal_tao = Decimal(str(tao))
            rao = int(decimal_tao * RAO_PER_TAO)
            return cls(rao)
        except (InvalidOperation, ValueError) as e:
            raise BalanceError(f"Invalid TAO amount: {tao}") from e
    
    @classmethod
    def from_rao(cls, rao: int) -> "Balance":
        """
        Create Balance from RAO amount.
        
        Args:
            rao: Amount in RAO
            
        Returns:
            Balance instance
        """
        return cls(rao)
    
    @property
    def rao(self) -> int:
        """Get balance in RAO."""
        return self._rao
    
    @property
    def tao(self) -> Decimal:
        """Get balance in TAO with full precision."""
        return Decimal(self._rao) / Decimal(RAO_PER_TAO)
    
    @property
    def tao_float(self) -> float:
        """Get balance in TAO as float (may lose precision)."""
        return float(self.tao)
    
    def __add__(self, other: "Balance") -> "Balance":
        """Add two balances."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot add Balance with {type(other)}")
        return Balance(self._rao + other._rao)
    
    def __sub__(self, other: "Balance") -> "Balance":
        """Subtract two balances."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot subtract {type(other)} from Balance")
        return Balance(self._rao - other._rao)
    
    def __mul__(self, multiplier: Union[int, float, Decimal]) -> "Balance":
        """Multiply balance by a scalar."""
        try:
            result_rao = int(Decimal(self._rao) * Decimal(str(multiplier)))
            return Balance(result_rao)
        except (InvalidOperation, ValueError) as e:
            raise BalanceError(f"Invalid multiplier: {multiplier}") from e
    
    def __truediv__(self, divisor: Union[int, float, Decimal]) -> "Balance":
        """Divide balance by a scalar."""
        try:
            if Decimal(str(divisor)) == 0:
                raise BalanceError("Cannot divide by zero")
            result_rao = int(Decimal(self._rao) / Decimal(str(divisor)))
            return Balance(result_rao)
        except (InvalidOperation, ValueError) as e:
            raise BalanceError(f"Invalid divisor: {divisor}") from e
    
    def __eq__(self, other: object) -> bool:
        """Check equality."""
        if not isinstance(other, Balance):
            return False
        return self._rao == other._rao
    
    def __lt__(self, other: "Balance") -> bool:
        """Less than comparison."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot compare Balance with {type(other)}")
        return self._rao < other._rao
    
    def __le__(self, other: "Balance") -> bool:
        """Less than or equal comparison."""
        return self == other or self < other
    
    def __gt__(self, other: "Balance") -> bool:
        """Greater than comparison."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot compare Balance with {type(other)}")
        return self._rao > other._rao
    
    def __ge__(self, other: "Balance") -> bool:
        """Greater than or equal comparison."""
        return self == other or self > other
    
    def __str__(self) -> str:
        """String representation in TAO."""
        return format_balance(self._rao, unit="TAO")
    
    def __repr__(self) -> str:
        """Detailed representation."""
        return f"Balance({self._rao} RAO)"
    
    def __hash__(self) -> int:
        """Hash for use in sets and dicts."""
        return hash(self._rao)


def format_balance(
    amount: Union[int, float, Decimal, Balance],
    unit: str = "TAO",
    decimals: int = 9,
    include_unit: bool = True,
    thousands_separator: bool = True
) -> str:
    """
    Format balance for display.
    
    Args:
        amount: Amount to format (RAO if int, TAO if float/Decimal, or Balance object)
        unit: Unit to display ("TAO" or "RAO")
        decimals: Number of decimal places (only for TAO)
        include_unit: Whether to include unit suffix
        thousands_separator: Whether to use comma separator
        
    Returns:
        Formatted balance string
        
    Examples:
        >>> format_balance(1500000000)
        "1.500000000 TAO"
        >>> format_balance(1500000000, decimals=2)
        "1.50 TAO"
        >>> format_balance(1500000000, unit="RAO", include_unit=False)
        "1,500,000,000"
    """
    # Convert to Balance object
    if isinstance(amount, Balance):
        balance = amount
    elif isinstance(amount, int):
        balance = Balance.from_rao(amount)
    elif isinstance(amount, (float, Decimal)):
        balance = Balance.from_tao(amount)
    else:
        raise BalanceError(f"Unsupported amount type: {type(amount)}")
    
    if unit.upper() == "TAO":
        # Format as TAO
        decimal_amount = balance.tao
        # Round to specified decimals
        rounded = decimal_amount.quantize(
            Decimal(10) ** -decimals, 
            rounding=ROUND_DOWN
        )
        
        if thousands_separator:
            # Format with thousands separator
            formatted = f"{rounded:,.{decimals}f}"
        else:
            formatted = f"{rounded:.{decimals}f}"
        
        if include_unit:
            formatted += " TAO"
    
    elif unit.upper() == "RAO":
        # Format as RAO (integer)
        if thousands_separator:
            formatted = f"{balance.rao:,}"
        else:
            formatted = str(balance.rao)
        
        if include_unit:
            formatted += " RAO"
    
    else:
        raise BalanceError(f"Unknown unit: {unit}. Use 'TAO' or 'RAO'")
    
    return formatted


def parse_balance(balance_str: str) -> Balance:
    """
    Parse balance from string.
    
    Supports formats:
    - "1.5 TAO" or "1.5TAO"
    - "1500000000 RAO" or "1500000000RAO"
    - "1,500,000,000" (assumes RAO if no decimal point)
    - "1.5" (assumes TAO)
    
    Args:
        balance_str: Balance string to parse
        
    Returns:
        Balance object
        
    Examples:
        >>> parse_balance("1.5 TAO")
        Balance(1500000000 RAO)
        >>> parse_balance("1,500,000,000 RAO")
        Balance(1500000000 RAO)
    """
    # Remove whitespace and convert to uppercase
    balance_str = balance_str.strip()
    upper_str = balance_str.upper()
    
    # Check for explicit unit
    if "TAO" in upper_str:
        # Extract number part
        number_str = re.sub(r'[^\d.]', '', balance_str)
        return Balance.from_tao(number_str)
    
    elif "RAO" in upper_str:
        # Extract number part
        number_str = re.sub(r'[^\d]', '', balance_str)
        return Balance.from_rao(int(number_str))
    
    else:
        # No explicit unit - infer from format
        # Remove commas
        clean_str = balance_str.replace(',', '')
        
        if '.' in clean_str:
            # Has decimal point - assume TAO
            return Balance.from_tao(clean_str)
        else:
            # No decimal point - assume RAO
            return Balance.from_rao(int(clean_str))


def tao_to_rao(tao: Union[float, str, Decimal]) -> int:
    """
    Convert TAO to RAO.
    
    Args:
        tao: Amount in TAO
        
    Returns:
        Amount in RAO
        
    Examples:
        >>> tao_to_rao(1.5)
        1500000000
        >>> tao_to_rao("0.000000001")
        1
    """
    return Balance.from_tao(tao).rao


def rao_to_tao(rao: int) -> Decimal:
    """
    Convert RAO to TAO.
    
    Args:
        rao: Amount in RAO
        
    Returns:
        Amount in TAO (Decimal)
        
    Examples:
        >>> rao_to_tao(1500000000)
        Decimal('1.5')
        >>> rao_to_tao(1)
        Decimal('0.000000001')
    """
    return Balance.from_rao(rao).tao


def validate_balance(
    balance: Union[int, float, Decimal, Balance],
    min_balance: Optional[Union[int, float, Decimal, Balance]] = None,
    max_balance: Optional[Union[int, float, Decimal, Balance]] = None
) -> bool:
    """
    Validate balance is within acceptable range.
    
    Args:
        balance: Balance to validate
        min_balance: Minimum acceptable balance (None for no minimum)
        max_balance: Maximum acceptable balance (None for no maximum)
        
    Returns:
        True if valid, False otherwise
        
    Examples:
        >>> validate_balance(Balance.from_tao(100), min_balance=Balance.from_tao(10))
        True
        >>> validate_balance(Balance.from_tao(5), min_balance=Balance.from_tao(10))
        False
    """
    # Convert to Balance objects
    if not isinstance(balance, Balance):
        if isinstance(balance, int):
            balance = Balance.from_rao(balance)
        else:
            balance = Balance.from_tao(balance)
    
    if min_balance is not None:
        if not isinstance(min_balance, Balance):
            if isinstance(min_balance, int):
                min_balance = Balance.from_rao(min_balance)
            else:
                min_balance = Balance.from_tao(min_balance)
        if balance < min_balance:
            return False
    
    if max_balance is not None:
        if not isinstance(max_balance, Balance):
            if isinstance(max_balance, int):
                max_balance = Balance.from_rao(max_balance)
            else:
                max_balance = Balance.from_tao(max_balance)
        if balance > max_balance:
            return False
    
    return True


def calculate_percentage(
    amount: Union[int, float, Decimal, Balance],
    total: Union[int, float, Decimal, Balance],
    decimals: int = 2
) -> Decimal:
    """
    Calculate percentage of amount relative to total.
    
    Args:
        amount: Amount to calculate percentage for
        total: Total amount
        decimals: Number of decimal places in result
        
    Returns:
        Percentage (0-100)
        
    Examples:
        >>> calculate_percentage(Balance.from_tao(25), Balance.from_tao(100))
        Decimal('25.00')
        >>> calculate_percentage(Balance.from_tao(33.333), Balance.from_tao(100), decimals=3)
        Decimal('33.333')
    """
    # Convert to Balance objects
    if not isinstance(amount, Balance):
        if isinstance(amount, int):
            amount = Balance.from_rao(amount)
        else:
            amount = Balance.from_tao(amount)
    
    if not isinstance(total, Balance):
        if isinstance(total, int):
            total = Balance.from_rao(total)
        else:
            total = Balance.from_tao(total)
    
    if total.rao == 0:
        return Decimal('0')
    
    percentage = (Decimal(amount.rao) / Decimal(total.rao)) * 100
    return percentage.quantize(Decimal(10) ** -decimals, rounding=ROUND_DOWN)


def sum_balances(balances: list[Balance]) -> Balance:
    """
    Sum a list of balances.
    
    Args:
        balances: List of Balance objects
        
    Returns:
        Total balance
        
    Examples:
        >>> balances = [Balance.from_tao(10), Balance.from_tao(20), Balance.from_tao(30)]
        >>> total = sum_balances(balances)
        >>> print(total.tao)
        60
    """
    total_rao = sum(b.rao for b in balances)
    return Balance.from_rao(total_rao)
