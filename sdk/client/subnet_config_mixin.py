"""
Subnet Config Mixin for LuxtensorClient

Provides subnet configuration parameter methods.
"""

import logging
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


class SubnetConfigMixin:
    """
    Mixin providing subnet configuration methods.

    Methods for subnet parameters: immunity, kappa, rho, UIDs, validators,
    adjustment params, scaling factors, bonds, target registrations.
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def get_immunity_period(self, subnet_id: int) -> int:
        """Get immunity period for a subnet (in blocks)."""
        try:
            result = self._rpc()._call_rpc("query_immunityPeriod", [subnet_id])
            return int(result)
        except Exception as e:
            logger.error(f"Error getting immunity period for subnet {subnet_id}: {e}")
            raise

    def get_kappa(self, subnet_id: int) -> float:
        """Get kappa parameter for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_kappa", [subnet_id])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting kappa for subnet {subnet_id}: {e}")
            raise

    def get_rho(self, subnet_id: int) -> float:
        """Get rho parameter for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_rho", [subnet_id])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting rho for subnet {subnet_id}: {e}")
            raise

    def get_max_allowed_uids(self, subnet_id: int) -> int:
        """Get maximum allowed UIDs in a subnet."""
        try:
            result = self._rpc()._call_rpc("query_maxAllowedUids", [subnet_id])
            return int(result)
        except Exception as e:
            logger.error(f"Error getting max allowed UIDs for subnet {subnet_id}: {e}")
            raise

    def get_max_allowed_validators(self, subnet_id: int) -> int:
        """Get maximum allowed validators in a subnet."""
        try:
            result = self._rpc()._call_rpc("query_maxAllowedValidators", [subnet_id])
            return int(result)
        except Exception as e:
            logger.error(f"Error getting max allowed validators for subnet {subnet_id}: {e}")
            raise

    def get_adjustment_alpha(self, subnet_id: int) -> float:
        """Get adjustment alpha parameter for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_adjustmentAlpha", [subnet_id])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting adjustment alpha for subnet {subnet_id}: {e}")
            raise

    def get_adjustment_interval(self, subnet_id: int) -> int:
        """Get adjustment interval for a subnet (in blocks)."""
        try:
            result = self._rpc()._call_rpc("query_adjustmentInterval", [subnet_id])
            return int(result)
        except Exception as e:
            logger.error(f"Error getting adjustment interval for subnet {subnet_id}: {e}")
            raise

    def get_bonds_moving_average(self, subnet_id: int) -> float:
        """Get bonds moving average for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_bondsMovingAverage", [subnet_id])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting bonds moving average for subnet {subnet_id}: {e}")
            raise

    def get_scaling_law_power(self, subnet_id: int) -> float:
        """Get scaling law power parameter for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_scalingLawPower", [subnet_id])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting scaling law power for subnet {subnet_id}: {e}")
            raise

    def get_synergy_scaling_law_power(self, subnet_id: int) -> float:
        """Get synergy scaling law power parameter for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_synergyScalingLawPower", [subnet_id])
            return float(result)
        except Exception as e:
            logger.error(f"Error getting synergy scaling law power for subnet {subnet_id}: {e}")
            raise

    def get_target_registrations_per_interval(self, subnet_id: int) -> int:
        """Get target registrations per interval for a subnet."""
        try:
            result = self._rpc()._call_rpc("query_targetRegistrationsPerInterval", [subnet_id])
            return int(result)
        except Exception as e:
            logger.error(
                f"Error getting target registrations per interval for subnet {subnet_id}: {e}"
            )
            raise
