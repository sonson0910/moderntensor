"""
AI Agent Mixin for LuxtensorClient

Wraps L1 agent_* RPC methods for managing autonomous AI agents:
registration, gas management, and info queries.
"""

import logging
import time
from typing import TYPE_CHECKING, Any, Dict, List, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class AgentMixin:
    """
    Mixin providing AI agent management methods.

    Methods:
        - agent_register()      — Register a new AI agent (signed)
        - agent_deregister()    — Deregister an agent (signed)
        - agent_deposit_gas()   — Deposit gas for an agent
        - agent_withdraw_gas()  — Withdraw unused gas (signed)
        - agent_get_info()      — Query agent info
        - agent_list_all()      — List all registered agents
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            return self

    def agent_register(
        self,
        address: str,
        endpoint: str,
        signature: str,
        capabilities: Optional[List[str]] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        """
        Register a new AI agent on-chain.

        Args:
            address: Agent address (0x...)
            endpoint: Agent HTTP/WS endpoint URL
            signature: Secp256k1 signature over `agent_register:{address}:{timestamp}`
            capabilities: Optional list of capability strings
            metadata: Optional key-value metadata dict

        Returns:
            Registration result with agent_id and status
        """
        try:
            timestamp = int(time.time())
            params: Dict[str, Any] = {
                "address": address,
                "endpoint": endpoint,
                "signature": signature,
                "timestamp": timestamp,
            }
            if capabilities:
                params["capabilities"] = capabilities
            if metadata:
                params["metadata"] = metadata
            return self._rpc()._call_rpc("agent_register", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to register agent {address}: {e}")
            raise

    def agent_deregister(
        self, address: str, signature: str
    ) -> Dict[str, Any]:
        """
        Deregister an AI agent (requires owner signature).

        Args:
            address: Agent address (0x...)
            signature: Secp256k1 signature

        Returns:
            Result dict with success status
        """
        try:
            timestamp = int(time.time())
            params = {
                "address": address,
                "signature": signature,
                "timestamp": timestamp,
            }
            return self._rpc()._call_rpc("agent_deregister", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to deregister agent {address}: {e}")
            return {"success": False, "error": str(e)}

    def agent_deposit_gas(
        self, address: str, amount: str
    ) -> Dict[str, Any]:
        """
        Deposit gas budget for an agent.

        Args:
            address: Agent address (0x...)
            amount: Amount to deposit (u128 as decimal string)

        Returns:
            Result dict with new gas balance
        """
        try:
            params = {"address": address, "amount": amount}
            return self._rpc()._call_rpc("agent_depositGas", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to deposit gas for agent {address}: {e}")
            return {"success": False, "error": str(e)}

    def agent_withdraw_gas(
        self, address: str, amount: str, signature: str
    ) -> Dict[str, Any]:
        """
        Withdraw unused gas from an agent's gas budget (requires signature).

        Args:
            address: Agent address (0x...)
            amount: Amount to withdraw (u128 as decimal string)
            signature: Secp256k1 signature

        Returns:
            Result dict with withdrawn amount and new balance
        """
        try:
            timestamp = int(time.time())
            params = {
                "address": address,
                "amount": amount,
                "signature": signature,
                "timestamp": timestamp,
            }
            return self._rpc()._call_rpc("agent_withdrawGas", [params]) or {}
        except Exception as e:
            logger.error(f"Failed to withdraw gas for agent {address}: {e}")
            return {"success": False, "error": str(e)}

    def agent_get_info(self, address: str) -> Optional[Dict[str, Any]]:
        """
        Get information about a registered agent.

        Args:
            address: Agent address (0x...)

        Returns:
            Agent info dict or None if not found
        """
        try:
            return self._rpc()._call_rpc("agent_getInfo", [address])
        except Exception as e:
            logger.warning(f"Failed to get agent info for {address}: {e}")
            return None

    def agent_list_all(self) -> List[Dict[str, Any]]:
        """
        List all registered AI agents.

        Returns:
            List of agent info dicts
        """
        try:
            result = self._rpc()._call_rpc("agent_listAll", [])
            return result if isinstance(result, list) else []
        except Exception as e:
            logger.warning(f"Failed to list agents: {e}")
            return []
