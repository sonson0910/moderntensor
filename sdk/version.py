# keymanager/hotkey_manager.py

import os
import json
import logging
from sdk.compat.luxtensor_types import Address, Network, PaymentVerificationKey, StakeVerificationKey
from cryptography.fernet import Fernet

logging.basicConfig(level=logging.INFO)


class HotKeyManager:
    """
    Manage HotKeys for a coldkey (already loaded).
    """

    def __init__(
        self, coldkeys_dict: dict, base_dir="moderntensor", network=Network.TESTNET
    ):
        """
        :param coldkeys_dict: Reference to a dictionary containing loaded coldkeys (name -> wallet info).
        :param base_dir: Directory containing coldkeys.
        :param network: Cardano network (TESTNET/MAINNET). Default: TESTNET.
        """
        self.coldkeys = coldkeys_dict
        self.base_dir = base_dir
        # ==== FIX: If the user passes network=None, set it to TESTNET ====
        if network is None:
            network = Network.TESTNET
        self.network = network

    def generate_hotkey(self, coldkey_name: str, hotkey_name: str):
        if coldkey_name not in self.coldkeys:
            raise ValueError(
                f"[generate_hotkey] Cold Key '{coldkey_name}' does not exist."
            )

        wallet_info = self.coldkeys[coldkey_name]
        coldkey_wallet = wallet_info["wallet"]
        cipher_suite: Fernet = wallet_info["cipher_suite"]
        hotkeys_dict = wallet_info["hotkeys"]

        idx = len(hotkeys_dict)  # hotkey index = number of current hotkeys

        payment_wallet = coldkey_wallet.derive_from_path(f"m/44'/0'/0'/0/{idx}")
        payment_vk = PaymentVerificationKey.from_primitive(payment_wallet.public_key)

        # Use different account for stake to avoid key reuse
        stake_wallet = coldkey_wallet.derive_from_path(f"m/44'/0'/1'/0/{idx}")
        stake_vk = StakeVerificationKey.from_primitive(stake_wallet.public_key)

        # Create address (pycardano Address)
        hotkey_address = Address(
            payment_vk.hash(), stake_vk.hash(), network=self.network
        )

        hotkey_data = {
            "name": hotkey_name,
            "address": str(hotkey_address),
            "payment_pub_key_hex": payment_wallet.public_key.hex(),
            "stake_pub_key_hex": stake_wallet.public_key.hex(),
        }

        encrypted_hotkey = cipher_suite.encrypt(
            json.dumps(hotkey_data).encode("utf-8")
        ).decode("utf-8")

        hotkeys_dict[hotkey_name] = encrypted_hotkey

        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        with open(hotkey_path, "w") as f:
            json.dump({"hotkeys": hotkeys_dict}, f)

        logging.info(
            f"[generate_hotkey] Hot Key '{hotkey_name}' has been successfully created."
        )
        return encrypted_hotkey

    def import_hotkey(
        self,
        coldkey_name: str,
        encrypted_hotkey: str,
        hotkey_name: str,
        overwrite=False,
    ):
        if coldkey_name not in self.coldkeys:
            raise ValueError(
                f"[import_hotkey] Cold Key '{coldkey_name}' does not exist."
            )

        wallet_info = self.coldkeys[coldkey_name]
        cipher_suite: Fernet = wallet_info["cipher_suite"]
        hotkeys_dict = wallet_info["hotkeys"]

        hotkey_data = json.loads(
            cipher_suite.decrypt(encrypted_hotkey.encode("utf-8")).decode("utf-8")
        )
        hotkey_data["name"] = hotkey_name

        if hotkey_name in hotkeys_dict:
            if not overwrite:
                response = (
                    input(f"Hot Key '{hotkey_name}' already exists. Overwrite? (yes/no): ")
                    .strip()
                    .lower()
                )
                if response not in ("yes", "y"):
                    # ==== FIX: change from logging.info => logging.warning ====
                    logging.warning(
                        "[import_hotkey] User canceled overwrite => import aborted."
                    )
                    return
            logging.warning(f"[import_hotkey] Overwriting Hot Key '{hotkey_name}'.")

        hotkeys_dict[hotkey_name] = encrypted_hotkey

        coldkey_dir = os.path.join(self.base_dir, coldkey_name)
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
        with open(hotkey_path, "w") as f:
            json.dump({"hotkeys": hotkeys_dict}, f)

        logging.info(
            f"[import_hotkey] Hot Key '{hotkey_name}' has been successfully imported."
        )
__version__ = "0.1.0"
