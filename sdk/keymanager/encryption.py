"""
Encryption Module

Provides encryption and decryption functionality for sensitive data.
"""

from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.backends import default_backend
from cryptography.fernet import Fernet
import base64
import os


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


def encrypt_data(data: bytes, password: str) -> bytes:
    """
    Encrypt data with password

    Args:
        data: Data to encrypt
        password: Encryption password

    Returns:
        Encrypted data (salt + encrypted_data)
    """
    # Generate random salt
    salt = os.urandom(16)

    # Derive key from password
    key = derive_key(password, salt)

    # Encrypt data
    f = Fernet(key)
    encrypted_data = f.encrypt(data)

    # Prepend salt to encrypted data
    return salt + encrypted_data


def decrypt_data(encrypted_data: bytes, password: str) -> bytes:
    """
    Decrypt data with password

    Args:
        encrypted_data: Encrypted data (salt + encrypted_data)
        password: Decryption password

    Returns:
        Decrypted data

    Raises:
        Exception: If decryption fails (wrong password or corrupted data)
    """
    # Extract salt (first 16 bytes)
    salt = encrypted_data[:16]
    encrypted_content = encrypted_data[16:]

    # Derive key from password
    key = derive_key(password, salt)

    # Decrypt data
    f = Fernet(key)
    try:
        decrypted_data = f.decrypt(encrypted_content)
        return decrypted_data
    except Exception as e:
        raise Exception("Decryption failed. Wrong password or corrupted data.") from e
