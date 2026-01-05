# sdk/keymanager/hotkey_manager.py

import os
import json
from cryptography.fernet import Fernet
from sdk.blockchain import L1Address, L1Network, KeyPair
from rich.console import Console
from typing import cast
import binascii

from sdk.config.settings import settings, logger


class HotKeyManager:
    """
    Manages hotkeys by deriving two ExtendedSigningKeys (payment and stake),
    converting them to CBOR (hex format), and then encrypting that data
    into 'hotkeys.json'.
    """

    def __init__(
        self,
        coldkeys_dict: dict,  # {coldkey_name -> {"wallet": L1HDWallet, "cipher_suite": ..., "hotkeys": {...}}}
        base_dir: str = None,  # type: ignore
        network: L1Network = L1Network.TESTNET,
    ):
        """
        Initializes the HotKeyManager.

        Args:
            coldkeys_dict (dict): A dictionary holding data for existing coldkeys,
                                  including the associated HDWallet, a Fernet cipher_suite,
                                  and any existing hotkeys.
            base_dir (str, optional): Base directory for coldkeys. If None,
                                      defaults to settings.HOTKEY_BASE_DIR.
            network (Network, optional): Indicates which Cardano network to use.
                                         Defaults to settings.CARDANO_NETWORK if None.
        """
        self.coldkeys = coldkeys_dict
        # Determine the base directory explicitly for type checking
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
                    "Could not determine the base directory for HotKeyManager."
                )
        # Use cast to satisfy the linter
        self.base_dir = cast(str, final_base_dir)

        self.network = L1Network.TESTNET

    def generate_hotkey(self, coldkey_name: str, hotkey_name: str) -> str:
        """
        Creates a new hotkey by:
          1) Deriving payment_xsk & stake_xsk from the coldkey's HDWallet using child paths.
          2) Converting them to CBOR (hex) for storage.
          3) Generating an address from these keys.
          4) Encrypting and writing the hotkey data (CBOR hex) into 'hotkeys.json'.

        Steps in detail:
          - Uses the next available index (based on how many hotkeys exist) to derive paths.
          - Derives child keys for payment (m/1852'/1815'/0'/0/idx) and stake (m/1852'/1815'/0'/2/idx).
          - Builds an Address object from payment & stake verification keys.
          - Stores extended signing keys in CBOR hex form, encrypts them, and writes to disk.

        Args:
            coldkey_name (str): The name of the coldkey under which this hotkey will be generated.
            hotkey_name (str): A unique identifier for this hotkey.

        Raises:
            ValueError: If the coldkey_name doesn't exist in self.coldkeys.
            Exception: If the hotkey_name already exists for that coldkey.

        Returns:
            str: The encrypted hotkey data (base64-encoded string).
        """

        # Verify the coldkey exists in memory
        if coldkey_name not in self.coldkeys:
            raise ValueError(
                f"Coldkey '{coldkey_name}' does not exist in self.coldkeys."
            )

        # Extract HDWallet, cipher_suite, and hotkeys dict from the coldkeys data
        wallet_info = self.coldkeys[coldkey_name]
        hd_wallet = wallet_info["wallet"]
        cipher_suite: Fernet = wallet_info["cipher_suite"]
        hotkeys_dict = wallet_info["hotkeys"]

        # Check if the hotkey already exists
        if hotkey_name in hotkeys_dict:
            raise Exception(
                f"Hotkey '{hotkey_name}' already exists for coldkey '{coldkey_name}'."
            )

        # Use the count of existing hotkeys as the derivation index
        idx = len(hotkeys_dict)

        # 1) Derive payment extended signing key from HDWallet
        payment_child = hd_wallet.derive_from_path(f"m/1852'/1815'/0'/0/{idx}")
        payment_xsk = ExtendedSigningKey.from_hdwallet(payment_child)
        payment_xvk = payment_xsk.to_verification_key()

        # 2) Derive stake extended signing key
        stake_child = hd_wallet.derive_from_path(f"m/1852'/1815'/0'/2/{idx}")
        stake_xsk = ExtendedSigningKey.from_hdwallet(stake_child)
        stake_xvk = stake_xsk.to_verification_key()

        # 3) Construct the Address using the derived payment & stake verification keys
        hotkey_address = Address(
            payment_part=payment_xvk.hash(),
            staking_part=stake_xvk.hash(),
            network=self.network,
        )

        # 4) Convert the extended signing keys to CBOR bytes and then to hex
        pay_cbor_hex = binascii.hexlify(payment_xsk.to_cbor()).decode("utf-8")
        stk_cbor_hex = binascii.hexlify(stake_xsk.to_cbor()).decode("utf-8")

        # Prepare the hotkey data for encryption
        hotkey_data = {
            "name": hotkey_name,
            "address": str(hotkey_address),
            "payment_xsk_cbor_hex": pay_cbor_hex,
            "stake_xsk_cbor_hex": stk_cbor_hex,
        }

        # Encrypt the hotkey JSON
        enc_bytes = cipher_suite.encrypt(json.dumps(hotkey_data).encode("utf-8"))
        encrypted_hotkey = enc_bytes.decode("utf-8")

        # Update the in-memory dictionary
        hotkeys_dict[hotkey_name] = {
            "address": str(hotkey_address),
            "encrypted_data": encrypted_hotkey,
        }

        # Persist changes to hotkeys.json
        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        os.makedirs(coldkey_dir, exist_ok=True)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        with open(hotkey_path, "w") as f:
            json.dump({"hotkeys": hotkeys_dict}, f, indent=2)

        # Decorated logger info
        logger.info(
            f":sparkles: [green]Generated hotkey[/green] '{hotkey_name}' => address=[blue]{hotkey_address}[/blue]"
        )
        return encrypted_hotkey

    def import_hotkey(
        self,
        coldkey_name: str,
        encrypted_hotkey: str,
        hotkey_name: str,
        overwrite=False,
    ):
        """
        Imports a hotkey by decrypting the provided encrypted hotkey data and storing
        the resulting address info into 'hotkeys.json'.

        Steps:
          - Decrypts the hotkey JSON (which includes payment_xsk_cbor_hex and stake_xsk_cbor_hex).
          - Reconstructs the extended signing keys from the hex-encoded CBOR.
          - (Optionally) prompts the user if a hotkey with the same name already exists
            and overwrite is disabled.
          - Writes the final data to 'hotkeys.json'.

        Args:
            coldkey_name (str): Name of the coldkey under which this hotkey will be stored.
            encrypted_hotkey (str): The encrypted hotkey data (base64-encoded, Fernet).
            hotkey_name (str): The name under which this hotkey will be stored.
            overwrite (bool): If True, overwrite an existing hotkey without prompting.

        Raises:
            ValueError: If the specified coldkey doesn't exist in memory.

        Returns:
            None
        """
        # Ensure the coldkey exists in memory
        if coldkey_name not in self.coldkeys:
            raise ValueError(
                f"[import_hotkey] Cold Key '{coldkey_name}' does not exist in self.coldkeys."
            )

        # Retrieve the cipher suite and hotkeys dictionary
        wallet_info = self.coldkeys[coldkey_name]
        cipher_suite: Fernet = wallet_info["cipher_suite"]
        hotkeys_dict = wallet_info["hotkeys"]

        # If the hotkey already exists and overwrite is False, ask the user
        if hotkey_name in hotkeys_dict and not overwrite:
            # Use Console for interactive prompt (already uses input, but can style message)
            console = Console()
            console.print(
                f":warning: [yellow]Hotkey '{hotkey_name}' already exists.[/yellow]"
            )
            resp = input("Overwrite? (yes/no): ").strip().lower()
            if resp not in ("yes", "y"):
                logger.warning(
                    ":stop_sign: [yellow]User canceled overwrite => import aborted.[/yellow]"
                )
                return
            logger.warning(
                f":warning: [yellow]Overwriting hotkey '{hotkey_name}'.[/yellow]"
            )

        # Decrypt the provided hotkey data
        dec_bytes = cipher_suite.decrypt(encrypted_hotkey.encode("utf-8"))
        hotkey_data = json.loads(dec_bytes.decode("utf-8"))
        hotkey_data["name"] = hotkey_name

        pay_cbor_hex = hotkey_data["payment_xsk_cbor_hex"]
        stk_cbor_hex = hotkey_data["stake_xsk_cbor_hex"]

        # Convert CBOR hex back to ExtendedSigningKey objects
        # Cast the result of from_cbor to ExtendedSigningKey
        payment_xsk_obj = ExtendedSigningKey.from_cbor(binascii.unhexlify(pay_cbor_hex))
        stake_xsk_obj = ExtendedSigningKey.from_cbor(binascii.unhexlify(stk_cbor_hex))

        payment_xsk = cast(ExtendedSigningKey, payment_xsk_obj)
        stake_xsk = cast(ExtendedSigningKey, stake_xsk_obj)

        # Reconstruct the address if needed
        pay_xvk = payment_xsk.to_verification_key()
        stake_xvk = stake_xsk.to_verification_key()
        final_address = hotkey_data["address"]

        # Update the in-memory dictionary
        hotkeys_dict[hotkey_name] = {
            "address": final_address,
            "encrypted_data": encrypted_hotkey,
        }

        # Write updated hotkeys to disk
        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        with open(hotkey_path, "w") as f:
            json.dump({"hotkeys": hotkeys_dict}, f, indent=2)

        # Decorated logger info
        logger.info(
            f":inbox_tray: [green]Imported hotkey[/green] '{hotkey_name}' => address=[blue]{final_address}[/blue]"
        )

    def regenerate_hotkey(
        self, coldkey_name: str, hotkey_name: str, index: int, force: bool = False
    ):
        """
        Regenerates a hotkey's keys and address from the parent coldkey and index,
        then encrypts and saves the entry to hotkeys.json.

        Args:
            coldkey_name (str): Name of the parent coldkey (must be loaded).
            hotkey_name (str): Name to assign to the regenerated hotkey entry.
            index (int): The non-negative derivation index.
            force (bool): Overwrite if the hotkey entry already exists.

        Raises:
            ValueError: If coldkey is not loaded, index is negative, or derivation fails.
            Exception: If the hotkey name already exists and force is False.
        """
        console = Console()

        # Validate index
        if index < 0:
            raise ValueError("Derivation index cannot be negative.")

        # Ensure the coldkey exists in memory (already checked by WalletManager, but good practice)
        if coldkey_name not in self.coldkeys:
            # This shouldn't be reached if called via WalletManager
            raise ValueError(
                f"Coldkey '{coldkey_name}' not found in internal dictionary."
            )

        # Retrieve coldkey info
        wallet_info = self.coldkeys[coldkey_name]
        hd_wallet = wallet_info["wallet"]
        cipher_suite: Fernet = wallet_info["cipher_suite"]
        hotkeys_dict = wallet_info["hotkeys"]

        # Check if hotkey name already exists and handle force flag
        if hotkey_name in hotkeys_dict and not force:
            console.print(
                f":warning: [yellow]Hotkey entry '{hotkey_name}' already exists for coldkey '{coldkey_name}'.[/yellow]"
            )
            raise Exception(
                f"Hotkey entry '{hotkey_name}' already exists. Use --force to overwrite."
            )
        elif hotkey_name in hotkeys_dict and force:
            console.print(
                f":warning: [yellow]Overwriting existing hotkey entry '{hotkey_name}' due to --force flag.[/yellow]"
            )

        # Derive Keys using the provided index
        try:
            logger.debug(f"Deriving keys for index {index}...")
            payment_child = hd_wallet.derive_from_path(f"m/1852'/1815'/0'/0/{index}")
            payment_xsk = ExtendedSigningKey.from_hdwallet(payment_child)
            payment_xvk = payment_xsk.to_verification_key()

            stake_child = hd_wallet.derive_from_path(f"m/1852'/1815'/0'/2/{index}")
            stake_xsk = ExtendedSigningKey.from_hdwallet(stake_child)
            stake_xvk = stake_xsk.to_verification_key()
            logger.debug(f"Keys derived successfully.")
        except Exception as e:
            logger.exception(
                f"Failed to derive keys for coldkey '{coldkey_name}' at index {index}: {e}"
            )
            raise ValueError(
                f"Failed to derive keys for index {index}. Check if index is valid for this wallet."
            ) from e

        # Generate Address
        hotkey_address = Address(
            payment_part=payment_xvk.hash(),
            staking_part=stake_xvk.hash(),
            network=self.network,
        )
        logger.debug(f"Regenerated address: {hotkey_address}")

        # Convert keys to CBOR hex
        pay_cbor_hex = binascii.hexlify(payment_xsk.to_cbor()).decode("utf-8")
        stk_cbor_hex = binascii.hexlify(stake_xsk.to_cbor()).decode("utf-8")

        # Prepare data for encryption
        hotkey_data = {
            "name": hotkey_name,  # Include name in encrypted data? Maybe not necessary.
            "address": str(hotkey_address),
            "payment_xsk_cbor_hex": pay_cbor_hex,
            "stake_xsk_cbor_hex": stk_cbor_hex,
            "derivation_index": index,  # Store the index used!
        }

        # Encrypt data
        enc_bytes = cipher_suite.encrypt(json.dumps(hotkey_data).encode("utf-8"))
        encrypted_hotkey_str = enc_bytes.decode("utf-8")

        # Update in-memory dictionary
        hotkeys_dict[hotkey_name] = {
            "address": str(hotkey_address),
            "encrypted_data": encrypted_hotkey_str,
        }

        # Persist changes to hotkeys.json
        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        # Ensure directory exists (might be needed if hotkeys.json was deleted)
        os.makedirs(coldkey_dir, exist_ok=True)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        try:
            with open(hotkey_path, "w") as f:
                json.dump({"hotkeys": hotkeys_dict}, f, indent=2)
            logger.debug(
                f":floppy_disk: Saved regenerated hotkey entry '{hotkey_name}' to {hotkey_path}"
            )
        except OSError as e:
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Failed to write hotkeys file: {e}"
            )
            # Raise exception as this is critical
            raise

        console.print(
            f":heavy_check_mark: [bold green]Hotkey '{hotkey_name}' (Index: {index}) regenerated successfully for coldkey '{coldkey_name}'.[/bold green]"
        )
        console.print(f"   Address: [blue]{hotkey_address}[/blue]")
