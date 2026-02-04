"""
Account Mixin for LuxtensorClient

Provides account query methods.
"""

import logging
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class AccountMixin:
    """
    Mixin providing account query methods.

    Requires:
        RPCProvider protocol (provided by BaseClient)

    Methods:
        - get_account() - Get account information by address
        - get_balance()
        - get_nonce()
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_account(self, address: str):
        """
        Get account information.

        Args:
            address: Account address

        Returns:
            Account object with balance, nonce, stake
        """
        from .base import Account

        balance_hex = self._rpc()._call_rpc("eth_getBalance", [address, "latest"])
        nonce_hex = self._rpc()._call_rpc("eth_getTransactionCount", [address, "latest"])

        balance = int(balance_hex, 16) if isinstance(balance_hex, str) else balance_hex
        nonce = int(nonce_hex, 16) if isinstance(nonce_hex, str) else nonce_hex

        # Get stake if staking mixin is available
        stake = 0
        if hasattr(self, "get_stake"):
            stake = self.get_stake(address)

        return Account(address=address, balance=balance, nonce=nonce, stake=stake)

    def get_balance(self, address: str) -> int:
        """
        Get account balance.

        Args:
            address: Account address

        Returns:
            Balance in LTS (smallest unit)
        """
        result = self._rpc()._call_rpc("eth_getBalance", [address, "latest"])
        return int(result, 16) if isinstance(result, str) else result

    def get_nonce(self, address: str) -> int:
        """
        Get account nonce (transaction count).

        Args:
            address: Account address

        Returns:
            Current nonce
        """
        result = self._rpc()._call_rpc("eth_getTransactionCount", [address, "latest"])
        return int(result, 16) if isinstance(result, str) else result
