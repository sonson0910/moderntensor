# sdk/metagraph/remove_fake_utxo.py
"""
DEPRECATED: UTXO removal functions are incompatible with Luxtensor

Luxtensor uses an account-based model, not UTXO. This module has been deprecated.

In an account-based system, there are no "fake UTXOs" to remove - accounts simply have balances.

See CARDANO_DEPRECATION.md for full migration guide.
"""

import logging

logger = logging.getLogger(__name__)


class UTxODeprecationError(Exception):
    """Raised when trying to use deprecated UTXO functions."""
    pass


def remove_fake_utxos(*args, **kwargs):
    """DEPRECATED: This is a Cardano UTXO function and doesn't work with Luxtensor."""
    raise UTxODeprecationError(
        "remove_fake_utxos() is deprecated. "
        "Luxtensor uses an account-based model without UTXOs. "
        "This concept doesn't apply to account-based blockchains. "
        "See CARDANO_DEPRECATION.md for migration guide."
    )


__all__ = [
    'UTxODeprecationError',
    'remove_fake_utxos',
]
