# sdk/metagraph/create_utxo.py
"""
DEPRECATED: UTXO creation functions are incompatible with Luxtensor

Luxtensor uses an account-based model, not UTXO. This module has been deprecated.

For creating transactions in Luxtensor, use:

    from sdk.luxtensor_client import LuxtensorClient
    from sdk.transactions import create_transfer_transaction
    
    client = LuxtensorClient("http://localhost:9944")
    
    # Create and submit transaction
    tx = create_transfer_transaction(
        from_address=sender,
        to_address=recipient,
        amount=value,
        nonce=client.get_nonce(sender)
    )
    
    tx_hash = client.submit_transaction(tx)

See CARDANO_DEPRECATION.md for full migration guide.
"""

import logging

logger = logging.getLogger(__name__)


class UTxODeprecationError(Exception):
    """Raised when trying to use deprecated UTXO functions."""
    pass


def find_suitable_ada_input(*args, **kwargs):
    """DEPRECATED: This is a Cardano UTXO function and doesn't work with Luxtensor."""
    raise UTxODeprecationError(
        "find_suitable_ada_input() is deprecated. "
        "Luxtensor uses an account-based model. "
        "Use LuxtensorClient.get_balance() to check account balance instead. "
        "See CARDANO_DEPRECATION.md for migration guide."
    )


def create_utxo(*args, **kwargs):
    """DEPRECATED: This is a Cardano UTXO function and doesn't work with Luxtensor."""
    raise UTxODeprecationError(
        "create_utxo() is deprecated. "
        "Luxtensor uses an account-based model. "
        "Use LuxtensorClient.submit_transaction() to send transactions instead. "
        "See CARDANO_DEPRECATION.md for migration guide."
    )


__all__ = [
    'UTxODeprecationError',
    'find_suitable_ada_input',
    'create_utxo',
]
