"""
Governance Mixin for LuxtensorClient

Provides DAO, proposal, and senate governance methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, List, cast

from .constants import HEX_ZERO

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class GovernanceMixin:
    """
    Mixin providing governance and DAO methods.

    Methods:
        - get_dao_balance() - Get DAO treasury balance
        - get_proposal() - Get specific proposal details
        - get_proposals() - Get all active proposals
        - get_senate_members() - Get senate member addresses
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_dao_balance(self) -> int:
        """
        Get DAO treasury balance.

        Returns:
            DAO balance in base units
        """
        try:
            result = self._rpc()._call_rpc("rewards_getDaoBalance", [])
            if isinstance(result, dict):
                balance = result.get("balance", HEX_ZERO)
                return int(balance, 16) if balance.startswith("0x") else int(balance)
            return 0
        except Exception as e:
            logger.warning(f"Failed to get DAO balance: {e}")
            return 0

    def get_proposal(self, proposal_id: int) -> Dict[str, Any]:
        """
        Get details of a specific proposal.

        Args:
            proposal_id: Proposal identifier

        Returns:
            Proposal details
        """
        try:
            return self._rpc()._call_rpc("governance_getProposal", [proposal_id])
        except Exception as e:
            logger.error(f"Error getting proposal {proposal_id}: {e}")
            raise

    def get_proposals(self) -> List[Dict[str, Any]]:
        """
        Get list of active governance proposals.

        Returns:
            List of proposal objects
        """
        try:
            return self._rpc()._call_rpc("governance_getProposals")
        except Exception as e:
            logger.error(f"Error getting proposals: {e}")
            raise

    def get_senate_members(self) -> List[str]:
        """
        Get senate members (if applicable).

        Returns:
            List of senate member addresses
        """
        try:
            result = self._rpc()._call_rpc("query_senateMembers")
            return result
        except Exception as e:
            logger.error(f"Error getting senate members: {e}")
            raise
