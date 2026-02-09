"""
Registration Mixin for LuxtensorClient

Provides neuron registration and axon serving methods.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, cast


if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class RegistrationMixin:
    """
    Mixin providing registration-related methods.

    Methods:
        - get_registration_cost() - Get registration cost for subnet
        - get_max_registrations_per_block() - Get max registrations per block
        - is_hotkey_registered() - Check if hotkey is registered
        - serve_axon() - Register or update axon endpoint
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_registration_cost(self, subnet_id: int) -> int:
        """
        Get registration cost for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Registration cost in tokens
        """
        try:
            # Derive from subnet_getHyperparameters
            hp = self._rpc()._call_rpc("subnet_getHyperparameters", [subnet_id])
            if hp:
                cost = hp.get("registration_cost", hp.get("registrationCost",
                       hp.get("burn_cost", hp.get("burnCost", 0))))
                return int(cost) if cost else 0
            return 0
        except Exception as e:
            logger.error(f"Error getting registration cost for subnet {subnet_id}: {e}")
            raise

    def get_max_registrations_per_block(self, subnet_id: int) -> int:
        """
        Get maximum registrations per block for a subnet.

        Args:
            subnet_id: Subnet identifier

        Returns:
            Maximum registrations per block
        """
        try:
            # Derive from subnet_getHyperparameters
            hp = self._rpc()._call_rpc("subnet_getHyperparameters", [subnet_id])
            if hp:
                val = hp.get("max_registrations_per_block",
                      hp.get("maxRegistrationsPerBlock", 0))
                return int(val) if val else 0
            return 0
        except Exception as e:
            logger.error(f"Error getting max registrations per block for subnet {subnet_id}: {e}")
            raise

    def is_hotkey_registered(self, subnet_id: int, hotkey: str) -> bool:
        """
        Check if a hotkey is registered in a subnet.

        Args:
            subnet_id: Subnet identifier
            hotkey: Hotkey address

        Returns:
            True if registered, False otherwise
        """
        try:
            result = self._rpc()._call_rpc("query_isHotkeyRegistered", [subnet_id, hotkey])
            return bool(result)
        except Exception as e:
            logger.error(f"Error checking if hotkey {hotkey} is registered: {e}")
            raise

    def serve_axon(
        self,
        subnet_id: int,
        hotkey: str,
        coldkey: str,
        ip: str,
        port: int,
        protocol: int = 4,
        version: int = 1,
        placeholder1: int = 0,
        placeholder2: int = 0,
    ) -> Dict[str, Any]:
        """
        Register or update axon endpoint information on the blockchain.

        This method submits a transaction to register the miner/validator's
        axon server endpoint, making it discoverable by other network participants.

        Args:
            subnet_id: Subnet identifier where this neuron operates
            hotkey: Hotkey address (used for signing)
            coldkey: Coldkey address (account owner)
            ip: IP address of the axon server
            port: Port number of the axon server
            protocol: Protocol version (default: 4)
            version: Axon version (default: 1)
            placeholder1: Reserved field (default: 0)
            placeholder2: Reserved field (default: 0)

        Returns:
            Transaction result with transaction hash and status

        Example:
            ```python
            result = client.serve_axon(
                subnet_id=1,
                hotkey="5C4hrfjw9DjXZTzV3MwzrrAr9P1MJhSrvWGWqi1eSuyUpnhM",
                coldkey="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
                ip=EXAMPLE_IP_ADDRESS,
                port=8091,
            )
            print(f"Registered axon: {result['tx_hash']}")
            ```
        """
        try:
            # Prepare axon info
            axon_info = {
                "ip": ip,
                "port": port,
                "ip_type": 4,  # IPv4
                "protocol": protocol,
                "hotkey": hotkey,
                "coldkey": coldkey,
                "version": version,
                "placeholder1": placeholder1,
                "placeholder2": placeholder2,
            }

            # Submit transaction to register axon
            result = self._rpc()._call_rpc("neuron_register", [subnet_id, axon_info])

            logger.info(f"Registered axon for subnet {subnet_id} at {ip}:{port}")
            return result

        except Exception as e:
            logger.error(f"Error registering axon: {e}")
            raise
