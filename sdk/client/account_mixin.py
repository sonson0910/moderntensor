"""
Account Mixin for LuxtensorClient

Provides account query methods.
"""

from typing import Dict, Any
import logging

logger = logging.getLogger(__name__)


class AccountMixin:
    """
    Mixin providing account query methods.

    Methods:
        - get_account()
        - get_balance()
        - get_nonce()
    """

    def get_account(self, address: str):
        """
        Get account information.

        Args:
            address: Account address

        Returns:
            Account object with balance, nonce, stake
        """
        from .base import Account

        balance_hex = self._call_rpc("eth_getBalance", [address, "latest"])
        nonce_hex = self._call_rpc("eth_getTransactionCount", [address, "latest"])

        balance = int(balance_hex, 16) if isinstance(balance_hex, str) else balance_hex
        nonce = int(nonce_hex, 16) if isinstance(nonce_hex, str) else nonce_hex

        # Get stake if staking mixin is available
        stake = 0
        if hasattr(self, 'get_stake'):
            stake = self.get_stake(address)

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
            address: Account address

        Returns:
            Balance in LTS (smallest unit)
        """
        result = self._call_rpc("eth_getBalance", [address, "latest"])
        return int(result, 16) if isinstance(result, str) else result

    def get_nonce(self, address: str) -> int:
        """
        Get account nonce (transaction count).

        Args:
            address: Account address

        Returns:
            Current nonce
        """
        result = self._call_rpc("eth_getTransactionCount", [address, "latest"])
        return int(result, 16) if isinstance(result, str) else result
