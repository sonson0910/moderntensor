"""
Balance Utilities Module

Provides utilities for token calculations, balance formatting, and unit conversions.
Supports MDT (ModernTensor Token) operations with wei (smallest unit) conversions.
"""

from decimal import Decimal, ROUND_DOWN, InvalidOperation
from typing import Union, Optional
import re


# Constants
WEI_PER_MDT = 10**9  # 1 MDT = 1,000,000,000 wei
MIN_BALANCE = 0
MAX_BALANCE = 21_000_000 * WEI_PER_MDT  # Maximum supply in wei


class BalanceError(Exception):
    """Exception raised for balance-related errors."""
    pass


class Balance:
    """
    Represents a token balance with precise decimal arithmetic.
    
    Internally stores balance in wei (smallest unit) to avoid floating point errors.
    Provides methods for arithmetic operations and conversions.
    
    Examples:
        >>> balance = Balance.from_mdt(100.5)
        >>> print(balance.mdt)
        100.5
        >>> print(balance.wei)
        100500000000
        
        >>> b1 = Balance.from_mdt(50)
        >>> b2 = Balance.from_mdt(25.5)
        >>> total = b1 + b2
        >>> print(total.mdt)
        75.5
    """
    
    def __init__(self, wei: int = 0):
        """
        Initialize Balance with wei amount.
        
        Args:
            wei: Amount in wei (smallest unit)
            
        Raises:
            BalanceError: If wei is negative or exceeds maximum supply
        """
        if wei < MIN_BALANCE:
            raise BalanceError(f"Balance cannot be negative: {wei}")
        if wei > MAX_BALANCE:
            raise BalanceError(f"Balance exceeds maximum supply: {wei}")
        self._wei = int(wei)
    
    @classmethod
    def from_mdt(cls, mdt: Union[float, str, Decimal]) -> "Balance":
        """
        Create Balance from MDT amount.
        
        Args:
            mdt: Amount in MDT
            
        Returns:
            Balance instance
            
        Examples:
            >>> Balance.from_mdt(1.5)
            Balance(1500000000 wei)
            >>> Balance.from_mdt("0.000000001")
            Balance(1 wei)
        """
        try:
            decimal_mdt = Decimal(str(mdt))
            wei = int(decimal_mdt * WEI_PER_MDT)
            return cls(wei)
        except (InvalidOperation, ValueError) as e:
            raise BalanceError(f"Invalid MDT amount: {mdt}") from e
    
    @classmethod
    def from_wei(cls, wei: int) -> "Balance":
        """
        Create Balance from wei amount.
        
        Args:
            wei: Amount in wei
            
        Returns:
            Balance instance
        """
        return cls(wei)
    
    @property
    def wei(self) -> int:
        """Get balance in wei."""
        return self._wei
    
    @property
    def mdt(self) -> Decimal:
        """Get balance in MDT with full precision."""
        return Decimal(self._wei) / Decimal(WEI_PER_MDT)
    
    @property
    def mdt_float(self) -> float:
        """Get balance in MDT as float (may lose precision)."""
        return float(self.mdt)
    
    def __add__(self, other: "Balance") -> "Balance":
        """Add two balances."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot add Balance with {type(other)}")
        return Balance(self._wei + other._wei)
    
    def __sub__(self, other: "Balance") -> "Balance":
        """Subtract two balances."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot subtract {type(other)} from Balance")
        return Balance(self._wei - other._wei)
    
    def __mul__(self, multiplier: Union[int, float, Decimal]) -> "Balance":
        """Multiply balance by a scalar."""
        try:
            result_wei = int(Decimal(self._wei) * Decimal(str(multiplier)))
            return Balance(result_wei)
        except (InvalidOperation, ValueError) as e:
            raise BalanceError(f"Invalid multiplier: {multiplier}") from e
    
    def __truediv__(self, divisor: Union[int, float, Decimal]) -> "Balance":
        """Divide balance by a scalar."""
        try:
            if Decimal(str(divisor)) == 0:
                raise BalanceError("Cannot divide by zero")
            result_wei = int(Decimal(self._wei) / Decimal(str(divisor)))
            return Balance(result_wei)
        except (InvalidOperation, ValueError) as e:
            raise BalanceError(f"Invalid divisor: {divisor}") from e
    
    def __eq__(self, other: object) -> bool:
        """Check equality."""
        if not isinstance(other, Balance):
            return False
        return self._wei == other._wei
    
    def __lt__(self, other: "Balance") -> bool:
        """Less than comparison."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot compare Balance with {type(other)}")
        return self._wei < other._wei
    
    def __le__(self, other: "Balance") -> bool:
        """Less than or equal comparison."""
        return self == other or self < other
    
    def __gt__(self, other: "Balance") -> bool:
        """Greater than comparison."""
        if not isinstance(other, Balance):
            raise TypeError(f"Cannot compare Balance with {type(other)}")
        return self._wei > other._wei
    
    def __ge__(self, other: "Balance") -> bool:
        """Greater than or equal comparison."""
        return self == other or self > other
    
    def __str__(self) -> str:
        """String representation in MDT."""
        return format_balance(self._wei, unit="MDT")
    
    def __repr__(self) -> str:
        """Detailed representation."""
        return f"Balance({self._wei} wei)"
    
    def __hash__(self) -> int:
        """Hash for use in sets and dicts."""
        return hash(self._wei)


def format_balance(
    amount: Union[int, float, Decimal, Balance],
    unit: str = "MDT",
    decimals: int = 9,
    include_unit: bool = True,
    thousands_separator: bool = True
) -> str:
    """
    Format balance for display.
    
    Args:
        amount: Amount to format (wei if int, MDT if float/Decimal, or Balance object)
        unit: Unit to display ("MDT" or "wei")
        decimals: Number of decimal places (only for MDT)
        include_unit: Whether to include unit suffix
        thousands_separator: Whether to use comma separator
        
    Returns:
        Formatted balance string
        
    Examples:
        >>> format_balance(1500000000)
        "1.500000000 MDT"
        >>> format_balance(1500000000, decimals=2)
        "1.50 MDT"
        >>> format_balance(1500000000, unit="wei", include_unit=False)
        "1,500,000,000"
    """
    # Convert to Balance object
    if isinstance(amount, Balance):
        balance = amount
    elif isinstance(amount, int):
        balance = Balance.from_wei(amount)
    elif isinstance(amount, (float, Decimal)):
        balance = Balance.from_mdt(amount)
    else:
        raise BalanceError(f"Unsupported amount type: {type(amount)}")
    
    if unit.upper() == "MDT":
        # Format as MDT
        decimal_amount = balance.mdt
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
            formatted += " MDT"
    
    elif unit.upper() == "wei":
        # Format as wei (integer)
        if thousands_separator:
            formatted = f"{balance.wei:,}"
        else:
            formatted = str(balance.wei)
        
        if include_unit:
            formatted += " wei"
    
    else:
        raise BalanceError(f"Unknown unit: {unit}. Use 'MDT' or 'wei'")
    
    return formatted


def parse_balance(balance_str: str) -> Balance:
    """
    Parse balance from string.
    
    Supports formats:
    - "1.5 MDT" or "1.5MDT"
    - "1500000000 wei" or "1500000000wei"
    - "1,500,000,000" (assumes wei if no decimal point)
    - "1.5" (assumes MDT)
    
    Args:
        balance_str: Balance string to parse
        
    Returns:
        Balance object
        
    Examples:
        >>> parse_balance("1.5 MDT")
        Balance(1500000000 wei)
        >>> parse_balance("1,500,000,000 wei")
        Balance(1500000000 wei)
    """
    # Remove whitespace and convert to uppercase
    balance_str = balance_str.strip()
    upper_str = balance_str.upper()
    
    # Check for explicit unit
    if "MDT" in upper_str:
        # Extract number part
        number_str = re.sub(r'[^\d.]', '', balance_str)
        return Balance.from_mdt(number_str)
    
    elif "wei" in upper_str:
        # Extract number part
        number_str = re.sub(r'[^\d]', '', balance_str)
        return Balance.from_wei(int(number_str))
    
    else:
        # No explicit unit - infer from format
        # Remove commas
        clean_str = balance_str.replace(',', '')
        
        if '.' in clean_str:
            # Has decimal point - assume MDT
            return Balance.from_mdt(clean_str)
        else:
            # No decimal point - assume wei
            return Balance.from_wei(int(clean_str))


def tao_to_wei(tao: Union[float, str, Decimal]) -> int:
    """
    Convert MDT to wei.
    
    Args:
        tao: Amount in MDT
        
    Returns:
        Amount in wei
        
    Examples:
        >>> tao_to_wei(1.5)
        1500000000
        >>> tao_to_wei("0.000000001")
        1
    """
    return Balance.from_mdt(tao).wei


def rao_to_mdt(rao: int) -> Decimal:
    """
    Convert wei to MDT.
    
    Args:
        rao: Amount in wei
        
    Returns:
        Amount in MDT (Decimal)
        
    Examples:
        >>> rao_to_mdt(1500000000)
        Decimal('1.5')
        >>> rao_to_mdt(1)
        Decimal('0.000000001')
    """
    return Balance.from_wei(rao).mdt


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
        >>> validate_balance(Balance.from_mdt(100), min_balance=Balance.from_mdt(10))
        True
        >>> validate_balance(Balance.from_mdt(5), min_balance=Balance.from_mdt(10))
        False
    """
    # Convert to Balance objects
    if not isinstance(balance, Balance):
        if isinstance(balance, int):
            balance = Balance.from_wei(balance)
        else:
            balance = Balance.from_mdt(balance)
    
    if min_balance is not None:
        if not isinstance(min_balance, Balance):
            if isinstance(min_balance, int):
                min_balance = Balance.from_wei(min_balance)
            else:
                min_balance = Balance.from_mdt(min_balance)
        if balance < min_balance:
            return False
    
    if max_balance is not None:
        if not isinstance(max_balance, Balance):
            if isinstance(max_balance, int):
                max_balance = Balance.from_wei(max_balance)
            else:
                max_balance = Balance.from_mdt(max_balance)
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
        >>> calculate_percentage(Balance.from_mdt(25), Balance.from_mdt(100))
        Decimal('25.00')
        >>> calculate_percentage(Balance.from_mdt(33.333), Balance.from_mdt(100), decimals=3)
        Decimal('33.333')
    """
    # Convert to Balance objects
    if not isinstance(amount, Balance):
        if isinstance(amount, int):
            amount = Balance.from_wei(amount)
        else:
            amount = Balance.from_mdt(amount)
    
    if not isinstance(total, Balance):
        if isinstance(total, int):
            total = Balance.from_wei(total)
        else:
            total = Balance.from_mdt(total)
    
    if total.wei == 0:
        return Decimal('0')
    
    percentage = (Decimal(amount.wei) / Decimal(total.wei)) * 100
    return percentage.quantize(Decimal(10) ** -decimals, rounding=ROUND_DOWN)


def sum_balances(balances: list[Balance]) -> Balance:
    """
    Sum a list of balances.
    
    Args:
        balances: List of Balance objects
        
    Returns:
        Total balance
        
    Examples:
        >>> balances = [Balance.from_mdt(10), Balance.from_mdt(20), Balance.from_mdt(30)]
        >>> total = sum_balances(balances)
        >>> print(total.mdt)
        60
    """
    total_wei = sum(b.wei for b in balances)
    return Balance.from_wei(total_wei)
