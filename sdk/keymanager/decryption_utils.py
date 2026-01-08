# keymanager/decryption_utils.py

import os
import json
import binascii
from cryptography.fernet import Fernet, InvalidToken
from sdk.compat.luxtensor_types import ExtendedSigningKey, CBORSerializable
from rich.console import Console
from typing import cast, Tuple, Optional

from sdk.keymanager.encryption_utils import get_or_create_salt, generate_encryption_key
from sdk.config.settings import settings, logger


def decode_hotkey_skey(
    base_dir: Optional[str] = None,
    coldkey_name: str = "",
    hotkey_name: str = "",
    password: str = "",
) -> Tuple[ExtendedSigningKey, ExtendedSigningKey]:
    """
    Decrypts the extended signing keys (payment & stake) for a specific hotkey.

    Retrieves the encrypted data for the specified hotkey from the corresponding
    'hotkeys.json' file within the coldkey's directory. It then uses the provided
    password to derive the encryption key (via Fernet) and decrypts the data.
    Finally, it reconstructs the pycardano `ExtendedSigningKey` objects for both
    payment and stake keys from their CBOR hex representations.

    Steps:
        1. Determine the correct base directory for coldkeys (uses settings if None).
        2. Construct the path to the specific coldkey directory.
        3. Get the salt for the coldkey directory.
        4. Generate the Fernet encryption key using the password and salt.
        5. Read 'hotkeys.json' from the coldkey directory.
        6. Locate the specified hotkey entry and extract its 'encrypted_data'.
        7. Decrypt the data using the Fernet cipher.
        8. Parse the decrypted JSON data.
        9. Extract the hex-encoded CBOR for payment and stake keys.
        10. Convert hex CBOR back into ExtendedSigningKey objects.

    Args:
        base_dir (Optional[str], optional): Base directory path for coldkeys.
            Defaults to `settings.HOTKEY_BASE_DIR` if None.
        coldkey_name (str): The name (folder name) of the coldkey.
        hotkey_name (str): The name of the specific hotkey within `hotkeys.json`.
        password (str): The password associated with the coldkey, required for decryption.

    Returns:
        Tuple[ExtendedSigningKey, ExtendedSigningKey]: A tuple containing
            (payment_extended_signing_key, stake_extended_signing_key).

    Raises:
        ValueError: If the base directory cannot be determined.
        FileNotFoundError: If the `hotkeys.json` file does not exist or the
                           specified `hotkey_name` is not found within the file.
        KeyError: If essential keys ('hotkeys', 'encrypted_data',
                  'payment_xsk_cbor_hex', 'stake_xsk_cbor_hex') are missing
                  in the `hotkeys.json` structure or the decrypted data.
        cryptography.fernet.InvalidToken: If decryption fails, likely due to an
                                         incorrect password or corrupted data.
        Exception: Catches other potential errors during file I/O or JSON parsing.
    """

    # ----------------------------------------------------------------
    # 1) Determine the base directory (use settings if not provided)
    # ----------------------------------------------------------------
    final_base_dir: str
    if base_dir is not None:
        final_base_dir = base_dir
    else:
        resolved_settings_dir = settings.HOTKEY_BASE_DIR
        if resolved_settings_dir:
            final_base_dir = resolved_settings_dir
        else:
            logger.error(
                ":stop_sign: [bold red]CRITICAL: base_dir is None and settings.HOTKEY_BASE_DIR is not set.[/bold red]"
            )
            raise ValueError(
                "Could not determine the base directory for decoding hotkey."
            )

    # Use cast here for os.path.join because final_base_dir is confirmed str
    coldkey_dir = os.path.join(cast(str, final_base_dir), coldkey_name)

    # ----------------------------------------------------------------
    # 2) Retrieve or create the salt for this coldkey directory
    #    and generate the Fernet encryption key
    # ----------------------------------------------------------------
    # Propagates exceptions from get_or_create_salt (IOError, OSError)
    salt = get_or_create_salt(coldkey_dir)
    # Propagates exceptions from generate_encryption_key (ValueError)
    enc_key = generate_encryption_key(password, salt)
    cipher = Fernet(enc_key)

    # ----------------------------------------------------------------
    # 3) Read hotkeys.json, then find the relevant encrypted_data
    # ----------------------------------------------------------------
    hotkeys_json_path = os.path.join(coldkey_dir, "hotkeys.json")
    if not os.path.exists(hotkeys_json_path):
        logger.error(
            f":file_folder: [red]hotkeys.json not found at {hotkeys_json_path}[/red]"
        )
        raise FileNotFoundError(f"hotkeys.json not found at {hotkeys_json_path}")

    try:
        with open(hotkeys_json_path, "r") as f:
            data = json.load(f)
    except json.JSONDecodeError as e:
        logger.error(
            f":warning: [red]Failed to parse JSON from {hotkeys_json_path}: {e}[/red]"
        )
        raise Exception(f"Failed to parse {hotkeys_json_path}")
    except IOError as e:
        logger.error(f":warning: [red]Failed to read {hotkeys_json_path}: {e}[/red]")
        raise

    if "hotkeys" not in data:
        logger.error(
            f":warning: [red]'hotkeys' key missing in {hotkeys_json_path}[/red]"
        )
        raise KeyError("'hotkeys' field is missing in hotkeys.json")

    if hotkey_name not in data["hotkeys"]:
        logger.error(
            f":mag: [red]Hotkey '{hotkey_name}' not found in {hotkeys_json_path}[/red]"
        )
        raise FileNotFoundError(f"Hotkey '{hotkey_name}' not found in hotkeys.json")

    encrypted_data = data["hotkeys"][hotkey_name].get("encrypted_data")
    if not encrypted_data:
        logger.error(
            f":lock: [red]'encrypted_data' missing for hotkey '{hotkey_name}'[/red]"
        )
        raise KeyError(f"'encrypted_data' missing for hotkey '{hotkey_name}'")

    # ----------------------------------------------------------------
    # 4) Decrypt the hotkey data and parse JSON
    # ----------------------------------------------------------------
    try:
        decrypted_bytes = cipher.decrypt(encrypted_data.encode("utf-8"))
    except InvalidToken:
        # This is the specific exception for Fernet decryption failure (wrong password/token)
        logger.error(
            f":cross_mark: [bold red]Decryption failed for hotkey '{hotkey_name}'. Invalid password or corrupted data.[/bold red]"
        )
        raise
    except Exception as e:
        # Catch other potential unexpected errors during decryption
        logger.error(
            f":rotating_light: [red]Unexpected error during decryption for '{hotkey_name}': {e}[/red]"
        )
        raise Exception(f"Unexpected decryption error for hotkey '{hotkey_name}'.")

    try:
        hotkey_data = json.loads(decrypted_bytes.decode("utf-8"))
    except json.JSONDecodeError as e:
        logger.error(
            f":warning: [red]Failed to parse decrypted JSON for hotkey '{hotkey_name}': {e}[/red]"
        )
        raise Exception(f"Failed to parse decrypted data for hotkey '{hotkey_name}'.")
    except UnicodeDecodeError as e:
        logger.error(
            f":warning: [red]Failed to decode decrypted bytes as UTF-8 for hotkey '{hotkey_name}': {e}[/red]"
        )
        raise Exception(f"Failed to decode decrypted data for hotkey '{hotkey_name}'.")

    # ----------------------------------------------------------------
    # 5) Extract the hex-encoded CBOR strings for payment & stake keys
    # ----------------------------------------------------------------
    pay_hex = hotkey_data.get("payment_xsk_cbor_hex")
    stk_hex = hotkey_data.get("stake_xsk_cbor_hex")

    if not pay_hex:
        logger.error(
            f":warning: [red]'payment_xsk_cbor_hex' missing in decrypted data for '{hotkey_name}'[/red]"
        )
        raise KeyError(
            f"Missing 'payment_xsk_cbor_hex' in decrypted data for hotkey '{hotkey_name}'."
        )
    if not stk_hex:
        logger.error(
            f":warning: [red]'stake_xsk_cbor_hex' missing in decrypted data for '{hotkey_name}'[/red]"
        )
        raise KeyError(
            f"Missing 'stake_xsk_cbor_hex' in decrypted data for hotkey '{hotkey_name}'."
        )

    # ----------------------------------------------------------------
    # 6) Convert hex to bytes, then to ExtendedSigningKey objects
    # ----------------------------------------------------------------
    try:
        # Use cast as from_cbor returns CBORSerializable
        payment_xsk_obj: CBORSerializable = ExtendedSigningKey.from_cbor(
            binascii.unhexlify(pay_hex)
        )
        stake_xsk_obj: CBORSerializable = ExtendedSigningKey.from_cbor(
            binascii.unhexlify(stk_hex)
        )

        # Confirm the types before returning
        if not isinstance(payment_xsk_obj, ExtendedSigningKey):
            raise TypeError("Decoded payment key is not an ExtendedSigningKey")
        if not isinstance(stake_xsk_obj, ExtendedSigningKey):
            raise TypeError("Decoded stake key is not an ExtendedSigningKey")

        # Cast is safe now after the isinstance checks
        payment_xsk: ExtendedSigningKey = cast(ExtendedSigningKey, payment_xsk_obj)
        stake_xsk: ExtendedSigningKey = cast(ExtendedSigningKey, stake_xsk_obj)

    except binascii.Error as e:
        logger.error(
            f":warning: [red]Failed to unhexlify CBOR data for hotkey '{hotkey_name}': {e}[/red]"
        )
        raise ValueError(
            f"Invalid hex data in decrypted keys for hotkey '{hotkey_name}'."
        )
    except TypeError as e:
        logger.error(
            f":rotating_light: [red]Type error during key reconstruction for '{hotkey_name}': {e}[/red]"
        )
        raise
    except Exception as e:
        logger.error(
            f":rotating_light: [red]Failed to reconstruct ExtendedSigningKey from CBOR for '{hotkey_name}': {e}[/red]"
        )
        raise Exception(f"Failed to load keys from CBOR for hotkey '{hotkey_name}'.")

    logger.info(
        f":unlock: [green]Successfully decoded hotkey[/green] [bold blue]'{hotkey_name}'[/bold blue] "
        f"[dim]under coldkey[/dim] [bold cyan]'{coldkey_name}'[/bold cyan]."
    )

    return (payment_xsk, stake_xsk)
