# tests/conftest.py

import os
import json
import binascii
import pytest
from cryptography.fernet import Fernet
from sdk.compat.pycardano import ExtendedSigningKey
from sdk.keymanager.encryption_utils import get_or_create_salt, generate_encryption_key


def decode_hotkey_skeys(base_dir, coldkey_name, hotkey_name, password):
    """
    Reads the 'hotkeys.json' file for a given coldkey, retrieves the
    encrypted data for a specified hotkey, and decrypts it using a
    Fernet cipher derived from the user's password and salt.

    Steps:
      1) Construct the path to the coldkey directory.
      2) Retrieve or create the salt using get_or_create_salt().
      3) Generate the encryption key via generate_encryption_key(password, salt).
      4) Load 'hotkeys.json' and read the 'encrypted_data' for the target hotkey.
      5) Decrypt to obtain the 'payment_xsk_cbor_hex' and 'stake_xsk_cbor_hex'.
      6) Convert these hex-encoded CBOR strings into ExtendedSigningKey objects.
      7) Return the tuple (payment_xsk, stake_xsk).

    Args:
        base_dir (str): The base directory containing all coldkey folders.
        coldkey_name (str): The specific coldkey folder name where the hotkey is stored.
        hotkey_name (str): The name/key of the hotkey in 'hotkeys.json'.
        password (str): The password required to decrypt the hotkey data.

    Returns:
        (ExtendedSigningKey, ExtendedSigningKey): A tuple containing the
        payment extended signing key and the stake extended signing key.
    """
    coldkey_dir = os.path.join(base_dir, coldkey_name)

    # Retrieve or create the salt file in coldkey_dir, then generate the encryption key
    salt = get_or_create_salt(coldkey_dir)
    enc_key = generate_encryption_key(password, salt)
    cipher = Fernet(enc_key)

    hotkeys_json_path = os.path.join(coldkey_dir, "hotkeys.json")
    with open(hotkeys_json_path, "r") as f:
        data = json.load(f)

    # Decrypt the base64-encoded 'encrypted_data' to get the hotkey info
    enc_data = data["hotkeys"][hotkey_name]["encrypted_data"]
    dec = cipher.decrypt(enc_data.encode("utf-8"))
    hotkey_data = json.loads(dec.decode("utf-8"))

    # Extract the hex-encoded CBOR strings for payment and stake keys
    pay_hex = hotkey_data["payment_xsk_cbor_hex"]
    stk_hex = hotkey_data["stake_xsk_cbor_hex"]

    # Convert CBOR hex to ExtendedSigningKey objects
    payment_xsk = ExtendedSigningKey.from_cbor(binascii.unhexlify(pay_hex))
    stake_xsk = ExtendedSigningKey.from_cbor(binascii.unhexlify(stk_hex))

    return payment_xsk, stake_xsk


@pytest.fixture(scope="session")
def hotkey_config():
    """
    Loads hotkey configuration from environment variables or falls back to defaults.

    This fixture uses a session scope, meaning it is executed once per test session.
    The returned dictionary can be used to configure tests that require a specific hotkey
    for integration or higher-level functional tests.

    Environment Variables:
        HOTKEY_BASE_DIR: Base directory containing coldkeys (default "moderntensor").
        COLDKEY_NAME: Name of the coldkey folder (default "kickoff").
        HOTKEY_NAME: Name of the hotkey inside 'hotkeys.json' (default "hk1").
        HOTKEY_PASSWORD: Password for decrypting the hotkey (default "sonlearn2003").

    Returns:
        dict: A dictionary containing:
            {
                "base_dir": <HOTKEY_BASE_DIR>,
                "coldkey_name": <COLDKEY_NAME>,
                "hotkey_name": <HOTKEY_NAME>,
                "password": <HOTKEY_PASSWORD>
            }
    """
    base_dir = os.getenv("HOTKEY_BASE_DIR", "moderntensor")
    coldkey_name = os.getenv("COLDKEY_NAME", "kickoff")
    hotkey_name = os.getenv("HOTKEY_NAME", "hk1")
    password = os.getenv("HOTKEY_PASSWORD", "sonlearn2003")
    return {
        "base_dir": base_dir,
        "coldkey_name": coldkey_name,
        "hotkey_name": hotkey_name,
        "password": password,
    }


@pytest.fixture(scope="session")
def hotkey_skey_fixture(hotkey_config):
    """
    A session-scoped fixture that decodes and provides the payment and stake
    ExtendedSigningKeys using the configuration from hotkey_config.

    Steps:
        1) Retrieve the hotkey configuration (base_dir, coldkey_name, etc.).
        2) Call decode_hotkey_skeys to decrypt and obtain both signing keys.
        3) Return the signing keys as a tuple (payment_xsk, stake_xsk).

    Returns:
        (ExtendedSigningKey, ExtendedSigningKey): A tuple containing
        the payment and stake extended signing keys for the specified hotkey.
    """
    pmt, stk = decode_hotkey_skeys(
        hotkey_config["base_dir"],
        hotkey_config["coldkey_name"],
        hotkey_config["hotkey_name"],
        hotkey_config["password"],
    )
    return (pmt, stk)
