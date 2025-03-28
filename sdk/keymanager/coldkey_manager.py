# keymanager/coldkey_manager.py

import os
import json
from bip_utils import Bip39MnemonicGenerator, Bip39Languages
from cryptography.fernet import InvalidToken
from pycardano import HDWallet

from sdk.keymanager.encryption_utils import get_cipher_suite
from sdk.config.settings import settings, logger

class ColdKeyManager:
    """
    Manages the creation and loading of ColdKeys. A ColdKey typically stores a 
    mnemonic (encrypted on disk), and is used to derive an HDWallet.
    """

    def __init__(self, base_dir: str = None):
        """
        Initialize the ColdKeyManager.

        Args:
            base_dir (str, optional): Custom base directory for storing coldkeys.
                                      If None, defaults to settings.HOTKEY_BASE_DIR.
        """
        # Determine the base directory
        self.base_dir = base_dir or settings.HOTKEY_BASE_DIR
        os.makedirs(self.base_dir, exist_ok=True)

        # Dictionary to store coldkeys that are loaded or newly created:
        # {
        #   "coldkey_name": {
        #       "wallet": HDWallet_object,
        #       "cipher_suite": Fernet_object,
        #       "hotkeys": {...}
        #   },
        #   ...
        # }
        self.coldkeys = {}

    def create_coldkey(self, name: str, password: str, words_num: int = 24):
        """
        Create a new ColdKey by generating a mnemonic (commonly 24 words).

        This method:
          1. Checks whether the coldkey name already exists (in memory or on disk).
          2. Creates an encryption key (Fernet) derived from `password`.
          3. Generates a mnemonic, encrypts, and saves it to 'mnemonic.enc'.
          4. Initializes a corresponding HDWallet.
          5. Creates a 'hotkeys.json' file to store any future hotkeys.
          6. Stores the resulting data in self.coldkeys.

        Args:
            name (str): Unique name for the coldkey.
            password (str): Password used to encrypt the mnemonic.
            words_num (int): Number of words in the mnemonic (commonly 24).

        Raises:
            Exception: If the coldkey name already exists in memory or on disk.
        """
        # 1) Check if the coldkey name already exists in memory
        if name in self.coldkeys:
            raise Exception(f"Coldkey '{name}' already exists in memory.")

        # 2) Create the path for the coldkey (folder name matches the coldkey name)
        coldkey_dir = os.path.join(self.base_dir, name)
        # Prevent overwriting an existing directory for a coldkey
        if os.path.exists(coldkey_dir):
            raise Exception(f"Coldkey folder '{coldkey_dir}' already exists.")

        os.makedirs(coldkey_dir, exist_ok=True)

        # Create a Fernet cipher suite using the user-provided password + salt
        cipher_suite = get_cipher_suite(password, coldkey_dir)

        # 3) Generate the mnemonic
        mnemonic = str(
            Bip39MnemonicGenerator(lang=Bip39Languages.ENGLISH).FromWordsNumber(words_num)
        )
        logger.info(f"[create_coldkey] Mnemonic for Cold Key '{name}': {mnemonic}")
        logger.warning(
            f"[create_coldkey] Mnemonic for Cold Key '{name}' has been created. "
            "Please store it securely."
        )

        # Encrypt and save the mnemonic in "mnemonic.enc"
        enc_path = os.path.join(coldkey_dir, "mnemonic.enc")
        with open(enc_path, "wb") as f:
            f.write(cipher_suite.encrypt(mnemonic.encode("utf-8")))

        # 4) Initialize an HDWallet from the generated mnemonic
        hdwallet = HDWallet.from_mnemonic(mnemonic)

        # 5) Create an empty "hotkeys.json" file if it doesn't already exist
        hotkeys_path = os.path.join(coldkey_dir, "hotkeys.json")
        if not os.path.exists(hotkeys_path):
            with open(hotkeys_path, "w") as f:
                json.dump({"hotkeys": {}}, f)

        # 6) Store the newly created coldkey data in memory
        self.coldkeys[name] = {
            "wallet": hdwallet,
            "cipher_suite": cipher_suite,
            "hotkeys": {},
        }

        logger.info(f"[create_coldkey] Cold Key '{name}' has been successfully created.")

    def load_coldkey(self, name: str, password: str):
        """
        Load an existing coldkey from disk, decrypt its mnemonic, and store 
        the HDWallet in memory.

        Steps:
          1. Reads 'mnemonic.enc', 'salt.bin', and 'hotkeys.json' from the coldkey directory.
          2. Decrypts the mnemonic using the provided password.
          3. Initializes an HDWallet from the mnemonic.
          4. Loads any existing hotkeys from 'hotkeys.json' into memory.

        Args:
            name (str): The coldkey name (folder) to load.
            password (str): Password used to decrypt the mnemonic.

        Raises:
            FileNotFoundError: If 'mnemonic.enc' is missing.
            Exception: If the mnemonic decryption fails (invalid password).
        """
        # Construct the path to the coldkey folder
        coldkey_dir = os.path.join(self.base_dir, name)
        # The encrypted mnemonic
        mnemonic_path = os.path.join(coldkey_dir, "mnemonic.enc")
        # The hotkeys.json file
        hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")

        # Check if the mnemonic file exists
        if not os.path.exists(mnemonic_path):
            raise FileNotFoundError(f"mnemonic.enc not found for Cold Key '{name}'.")

        # Create the Fernet cipher suite (password + salt)
        cipher_suite = get_cipher_suite(password, coldkey_dir)

        # Read the encrypted mnemonic
        with open(mnemonic_path, "rb") as f:
            encrypted_mnemonic = f.read()

        # Decrypt the mnemonic
        try:
            mnemonic = cipher_suite.decrypt(encrypted_mnemonic).decode("utf-8")
        except InvalidToken:
            # Invalid password => cannot decrypt
            raise Exception("Invalid password: failed to decrypt mnemonic.")

        # Recreate the HDWallet from the decrypted mnemonic
        hdwallet = HDWallet.from_mnemonic(mnemonic)

        # Read and parse "hotkeys.json"
        with open(hotkey_path, "r") as f:
            hotkeys_data = json.load(f)
        if "hotkeys" not in hotkeys_data:
            hotkeys_data["hotkeys"] = {}

        # Store the loaded coldkey data in memory
        self.coldkeys[name] = {
            "wallet": hdwallet,
            "cipher_suite": cipher_suite,
            "hotkeys": hotkeys_data["hotkeys"],
        }

        logger.info(f"[load_coldkey] Cold Key '{name}' has been successfully loaded.")
