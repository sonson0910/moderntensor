"""
Encryption Module

Provides encryption and decryption functionality for sensitive data
using industry-standard algorithms:

- **KDF:** PBKDF2-HMAC-SHA256 (600 000 iterations)
- **Cipher:** Fernet (AES-128-CBC + HMAC-SHA256)

Security Notes:
    - Passwords should be at least 8 characters.
    - A 16-byte random salt is prepended to the ciphertext.
    - The salt is NOT secret; it prevents rainbow-table attacks.
"""

from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives import hashes
from cryptography.fernet import Fernet, InvalidToken
import base64
import os


# ---------------------------------------------------------------------------
# Typed exceptions
# ---------------------------------------------------------------------------

class EncryptionError(Exception):
    """Raised when encryption fails."""


class DecryptionError(Exception):
    """Raised when decryption fails (wrong password or corrupted data)."""


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_MIN_PASSWORD_LENGTH = 8
_SALT_LENGTH = 16
_PBKDF2_ITERATIONS = 600_000


def derive_key(password: str, salt: bytes) -> bytes:
    """
    Derive encryption key from password using PBKDF2

    Args:
        password: User password
        salt: Salt bytes

    Returns:
        Derived key bytes
    """
    kdf = PBKDF2HMAC(
        algorithm=hashes.SHA256(),
        length=32,
        salt=salt,
        iterations=600000,
    )
    key = base64.urlsafe_b64encode(kdf.derive(password.encode()))
    return key


def _validate_password(password: str) -> None:
    """Validate password meets minimum security requirements."""
    if not password:
        raise ValueError("Password must not be empty")
    if len(password) < _MIN_PASSWORD_LENGTH:
        raise ValueError(
            f"Password must be at least {_MIN_PASSWORD_LENGTH} characters, "
            f"got {len(password)}"
        )


def encrypt_data(data: bytes, password: str) -> bytes:
    """
    Encrypt data with a password.

    The output format is ``salt (16 bytes) || Fernet ciphertext``.

    Args:
        data: Data to encrypt (must be non-empty bytes).
        password: Encryption password (≥ 8 characters).

    Returns:
        Encrypted data (salt + ciphertext).

    Raises:
        ValueError: If *password* or *data* is invalid.
        EncryptionError: If the encryption operation fails.
    """
    _validate_password(password)
    if not isinstance(data, bytes) or len(data) == 0:
        raise ValueError("Data must be non-empty bytes")

    try:
        salt = os.urandom(_SALT_LENGTH)
        key = derive_key(password, salt)
        f = Fernet(key)
        encrypted_data = f.encrypt(data)
        return salt + encrypted_data
    except ValueError:
        raise
    except Exception as exc:
        raise EncryptionError(f"Encryption failed: {exc}") from exc


def decrypt_data(encrypted_data: bytes, password: str) -> bytes:
    """
    Decrypt data with a password.

    Expects the format produced by :func:`encrypt_data`:
    ``salt (16 bytes) || Fernet ciphertext``.

    Args:
        encrypted_data: Encrypted data (salt + ciphertext).
        password: Decryption password.

    Returns:
        Decrypted plaintext bytes.

    Raises:
        ValueError: If inputs are invalid.
        DecryptionError: If the password is wrong or data is corrupted.
    """
    _validate_password(password)
    if not isinstance(encrypted_data, bytes) or len(encrypted_data) <= _SALT_LENGTH:
        raise ValueError(
            f"Encrypted data must be bytes longer than {_SALT_LENGTH} bytes"
        )

    salt = encrypted_data[:_SALT_LENGTH]
    encrypted_content = encrypted_data[_SALT_LENGTH:]

    key = derive_key(password, salt)
    f = Fernet(key)
    try:
        return f.decrypt(encrypted_content)
    except InvalidToken as exc:
        raise DecryptionError(
            "Decryption failed: wrong password or corrupted data"
        ) from exc
    except Exception as exc:
        raise DecryptionError(f"Decryption failed: {exc}") from exc
