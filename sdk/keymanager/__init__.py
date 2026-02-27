"""
Key Manager Module

Handles cryptographic key generation, derivation, transaction signing,
and data encryption for ModernTensor wallets.

Uses native Python cryptography matching Luxtensor's Rust implementation.
"""

from .key_generator import KeyGenerator
from .encryption import encrypt_data, decrypt_data, EncryptionError, DecryptionError
from .transaction_signer import TransactionSigner

__all__ = [
    "KeyGenerator",
    "TransactionSigner",
    "encrypt_data",
    "decrypt_data",
    "EncryptionError",
    "DecryptionError",
]
