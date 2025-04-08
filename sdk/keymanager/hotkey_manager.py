# sdk/keymanager/hotkey_manager.py

import os
import json
from cryptography.fernet import Fernet
from pycardano import Address, Network, ExtendedSigningKey

from sdk.config.settings import settings, logger

class HotKeyManager:
    """
    Manages hotkeys by deriving two ExtendedSigningKeys (payment and stake),
    converting them to CBOR (hex format), and then encrypting that data 
    into 'hotkeys.json'. 
    """

    def __init__(
        self,
        coldkeys_dict: dict,  # {coldkey_name -> {"wallet": HDWallet, "cipher_suite": ..., "hotkeys": {...}}}
        base_dir: str = None,
        network: Network = Network.TESTNET
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
        self.base_dir = base_dir or settings.HOTKEY_BASE_DIR
        self.network = Network.TESTNET

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
        import binascii

        # Verify the coldkey exists in memory
        if coldkey_name not in self.coldkeys:
            raise ValueError(f"Coldkey '{coldkey_name}' does not exist in self.coldkeys.")

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
            network=self.network
        )

        # 4) Convert the extended signing keys to CBOR bytes and then to hex
        pay_cbor_hex = binascii.hexlify(payment_xsk.to_cbor()).decode("utf-8")
        stk_cbor_hex = binascii.hexlify(stake_xsk.to_cbor()).decode("utf-8")

        # Prepare the hotkey data for encryption
        hotkey_data = {
            "name": hotkey_name,
            "address": str(hotkey_address),
            "payment_xsk_cbor_hex": pay_cbor_hex,
            "stake_xsk_cbor_hex": stk_cbor_hex
        }

        # Encrypt the hotkey JSON
        enc_bytes = cipher_suite.encrypt(json.dumps(hotkey_data).encode("utf-8"))
        encrypted_hotkey = enc_bytes.decode("utf-8")

        # Update the in-memory dictionary
        hotkeys_dict[hotkey_name] = {
            "address": str(hotkey_address),
            "encrypted_data": encrypted_hotkey
        }

        # Persist changes to hotkeys.json
        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        os.makedirs(coldkey_dir, exist_ok=True)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        with open(hotkey_path, "w") as f:
            json.dump({"hotkeys": hotkeys_dict}, f, indent=2)

        logger.info(f"[generate_hotkey] => '{hotkey_name}' => address={hotkey_address}")
        return encrypted_hotkey

    def import_hotkey(
        self,
        coldkey_name: str,
        encrypted_hotkey: str,
        hotkey_name: str,
        overwrite=False
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
        import binascii

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
            resp = input(f"Hot Key '{hotkey_name}' exists. Overwrite? (yes/no): ").strip().lower()
            if resp not in ("yes", "y"):
                logger.warning("[import_hotkey] User canceled overwrite => import aborted.")
                return
            logger.warning(f"[import_hotkey] Overwriting '{hotkey_name}'.")

        # Decrypt the provided hotkey data
        dec_bytes = cipher_suite.decrypt(encrypted_hotkey.encode("utf-8"))
        hotkey_data = json.loads(dec_bytes.decode("utf-8"))
        hotkey_data["name"] = hotkey_name

        pay_cbor_hex = hotkey_data["payment_xsk_cbor_hex"]
        stk_cbor_hex = hotkey_data["stake_xsk_cbor_hex"]

        # Convert CBOR hex back to ExtendedSigningKey objects
        payment_xsk = ExtendedSigningKey.from_cbor(binascii.unhexlify(pay_cbor_hex))
        stake_xsk = ExtendedSigningKey.from_cbor(binascii.unhexlify(stk_cbor_hex))

        # Reconstruct the address if needed
        pay_xvk = payment_xsk.to_verification_key()
        stake_xvk = stake_xsk.to_verification_key()
        final_address = hotkey_data["address"]

        # Update the in-memory dictionary
        hotkeys_dict[hotkey_name] = {
            "address": final_address,
            "encrypted_data": encrypted_hotkey
        }

        # Write updated hotkeys to disk
        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        with open(hotkey_path, "w") as f:
            json.dump({"hotkeys": hotkeys_dict}, f, indent=2)

        logger.info(f"[import_hotkey] => '{hotkey_name}' => address={final_address}")
