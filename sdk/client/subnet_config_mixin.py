"""
Subnet Config Mixin for LuxtensorClient

Provides subnet configuration parameter methods.
All methods use subnet_getHyperparameters RPC and extract the relevant field.
"""

import logging
from typing import TYPE_CHECKING, Any, Dict, Optional, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class SubnetConfigMixin:
    """
    Mixin providing subnet configuration methods.

    Methods for subnet parameters: immunity, kappa, rho, UIDs, validators,
    adjustment params, scaling factors, bonds, target registrations.

    All parameters are fetched via subnet_getHyperparameters RPC which returns
    the full hyperparameter dict, then the specific field is extracted.
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def _get_hyperparameter(self, subnet_id: int, field: str, default: Any = None) -> Any:
        """Fetch subnet hyperparameters and extract a single field."""
        try:
            result: Optional[Dict[str, Any]] = self._rpc()._call_rpc(
                "subnet_getHyperparameters", [subnet_id]
            )
            if result and isinstance(result, dict):
                return result.get(field, default)
            return default
        except Exception as e:
            logger.error(f"Error getting {field} for subnet {subnet_id}: {e}")
            raise

    def get_immunity_period(self, subnet_id: int) -> int:
        """Get immunity period for a subnet (in blocks)."""
        return int(self._get_hyperparameter(subnet_id, "immunity_period", 0))

    def get_kappa(self, subnet_id: int) -> float:
        """Get kappa parameter for a subnet."""
        return float(self._get_hyperparameter(subnet_id, "kappa", 0.0))

    def get_rho(self, subnet_id: int) -> float:
        """Get rho parameter for a subnet."""
        return float(self._get_hyperparameter(subnet_id, "rho", 0.0))

    def get_max_allowed_uids(self, subnet_id: int) -> int:
        """Get maximum allowed UIDs in a subnet."""
        return int(self._get_hyperparameter(subnet_id, "max_allowed_uids", 0))

    def get_max_allowed_validators(self, subnet_id: int) -> int:
        """Get maximum allowed validators in a subnet."""
        return int(self._get_hyperparameter(subnet_id, "max_allowed_validators", 0))

    def get_adjustment_alpha(self, subnet_id: int) -> float:
        """Get adjustment alpha parameter for a subnet."""
        return float(self._get_hyperparameter(subnet_id, "adjustment_alpha", 0.0))

    def get_adjustment_interval(self, subnet_id: int) -> int:
        """Get adjustment interval for a subnet (in blocks)."""
        return int(self._get_hyperparameter(subnet_id, "adjustment_interval", 0))

    def get_bonds_moving_average(self, subnet_id: int) -> float:
        """Get bonds moving average for a subnet."""
        return float(self._get_hyperparameter(subnet_id, "bonds_moving_average", 0.0))

    def get_scaling_law_power(self, subnet_id: int) -> float:
        """Get scaling law power parameter for a subnet."""
        return float(self._get_hyperparameter(subnet_id, "scaling_law_power", 0.0))

    def get_synergy_scaling_law_power(self, subnet_id: int) -> float:
        """Get synergy scaling law power parameter for a subnet."""
        return float(self._get_hyperparameter(subnet_id, "synergy_scaling_law_power", 0.0))

    def get_target_registrations_per_interval(self, subnet_id: int) -> int:
        """Get target registrations per interval for a subnet."""
        return int(self._get_hyperparameter(
            subnet_id, "target_registrations_per_interval", 0
        ))
