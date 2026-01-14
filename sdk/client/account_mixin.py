"""
Account Mixin for LuxtensorClient

Provides account query methods.
"""

from typing import TYPE_CHECKING
from .base import Account

if TYPE_CHECKING:
    from .base import BaseClient


class AccountMixin:
    """
    Mixin providing account query methods.

    Methods:
        - get_account()
        - get_balance()
        - get_nonce()
    """

    # Type hints for mixin
    _call_rpc: callable
    get_stake: callable  # From staking mixin

    def get_account(self, address: str) -> Account:
        """
        Get account information.

        Args:
            address: Account address (0x...)

        Returns:
            Account object with balance, nonce, stake
        """
        balance_hex = self._call_rpc("eth_getBalance", [address, "latest"])
        nonce_hex = self._call_rpc("eth_getTransactionCount", [address, "latest"])

        balance = int(balance_hex, 16) if isinstance(balance_hex, str) else balance_hex
        nonce = int(nonce_hex, 16) if isinstance(nonce_hex, str) else nonce_hex

        # Try to get stake, default to 0 if method not available
        try:
            stake = self.get_stake(address)
        except:
            stake = 0

        return Account(
            address=address,
            balance=balance,
            nonce=nonce,
            stake=stake
        )

    def get_balance(self, address: str) -> int:
        """
        Get account balance.

        Args:
            address: Account address (0x...)

        Returns:
            Balance in LTS (smallest unit)
        """
        result = self._call_rpc("eth_getBalance", [address, "latest"])
        return int(result, 16) if isinstance(result, str) else result

    def get_nonce(self, address: str) -> int:
        """
        Get account nonce (transaction count).

        Args:
            address: Account address (0x...)

        Returns:
            Current nonce
        """
        result = self._call_rpc("eth_getTransactionCount", [address, "latest"])
        return int(result, 16) if isinstance(result, str) else result

    def get_code(self, address: str) -> str:
        """
        Get contract code at address.

        Args:
            address: Contract address (0x...)

        Returns:
            Contract bytecode (hex string)
        """
        return self._call_rpc("eth_getCode", [address, "latest"]) or "0x"

    def is_contract(self, address: str) -> bool:
        """
        Check if address is a contract.

        Args:
            address: Address to check

        Returns:
            True if contract, False if EOA
        """
        code = self.get_code(address)
        return code != "0x" and len(code) > 2
