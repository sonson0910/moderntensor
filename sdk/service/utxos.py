# sdk/service/utxos.py
"""
DEPRECATED: UTXO functions are incompatible with Luxtensor

Luxtensor uses an account-based model (like Ethereum), not a UTXO model (like Cardano).
This module has been deprecated and these functions no longer work.

For Luxtensor blockchain interaction, use the account-based API:

    from sdk.luxtensor_client import LuxtensorClient
    
    client = LuxtensorClient("http://localhost:9944")
    
    # Get account balance instead of querying UTxOs
    balance = client.get_balance(address)
    
    # Query neuron state instead of finding UTXO by UID
    neuron = client.get_neuron(subnet_id=1, neuron_uid=0)
    
    # Get validator/miner info from chain state
    validators = client.get_validators(subnet_id=1)
    miners = client.get_miners(subnet_id=1)

See CARDANO_DEPRECATION.md for full migration guide.
"""

import logging
from typing import Type, Optional, Any

logger = logging.getLogger(__name__)


class UTxODeprecationError(Exception):
    """Raised when trying to use deprecated UTXO functions."""
    pass


def get_utxo_from_str(*args, **kwargs) -> None:
    """
    DEPRECATED: This function is for Cardano UTXO model and doesn't work with Luxtensor.
    
    Use LuxtensorClient.get_neuron() or get_account_state() instead.
    """
    raise UTxODeprecationError(
        "get_utxo_from_str() is deprecated. "
        "Luxtensor uses an account-based model, not UTXO. "
        "Use LuxtensorClient.get_neuron(subnet_id, neuron_uid) instead. "
        "See CARDANO_DEPRECATION.md for migration guide."
    )


def get_utxo_with_lowest_performance(*args, **kwargs) -> None:
    """
    DEPRECATED: This function is for Cardano UTXO model and doesn't work with Luxtensor.
    
    Use LuxtensorClient to query miners/validators by performance metrics instead.
    """
    raise UTxODeprecationError(
        "get_utxo_with_lowest_performance() is deprecated. "
        "Luxtensor uses an account-based model, not UTXO. "
        "Use LuxtensorClient.get_miners() and filter by performance instead. "
        "See CARDANO_DEPRECATION.md for migration guide."
    )


def get_all_utxos(*args, **kwargs) -> None:
    """
    DEPRECATED: This function is for Cardano UTXO model and doesn't work with Luxtensor.
    
    Use LuxtensorClient to query account states instead.
    """
    raise UTxODeprecationError(
        "get_all_utxos() is deprecated. "
        "Luxtensor uses an account-based model, not UTXO. "
        "Use LuxtensorClient.get_all_neurons() or similar methods instead. "
        "See CARDANO_DEPRECATION.md for migration guide."
    )


# Add any other UTXO functions that might be imported
__all__ = [
    'UTxODeprecationError',
    'get_utxo_from_str',
    'get_utxo_with_lowest_performance',
    'get_all_utxos',
]
