# keymanager/coldkey_manager.py

import os
import json
from bip_utils import (
    Bip39MnemonicGenerator,
    Bip39Languages,
)
from cryptography.fernet import InvalidToken
from sdk.blockchain import L1HDWallet, L1Address, L1Network
from rich.console import Console
from typing import cast

from sdk.keymanager.encryption_utils import get_cipher_suite
from sdk.config.settings import settings, logger


class ColdKeyManager:
    """
    Manages the creation and loading of ColdKeys. A ColdKey typically stores a
    mnemonic (encrypted on disk), and is used to derive an HDWallet.
    """

    def __init__(self, base_dir: str = None):  # type: ignore
        """
        Initialize the ColdKeyManager.

        Args:
            base_dir (str, optional): Custom base directory for storing coldkeys.
                                      If None, defaults to settings.HOTKEY_BASE_DIR.
        """
        # Determine the base directory
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
                    "Could not determine the base directory for ColdKeyManager."
                )

        # Use cast to assure the type checker
        self.base_dir = cast(str, final_base_dir)
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
        # Use Console for user-facing messages
        console = Console()

        # 1) Check if the coldkey name already exists in memory
        if name in self.coldkeys:
            # Use console for error message
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Coldkey '{name}' already exists in memory."
            )
            raise Exception(f"Coldkey '{name}' already exists in memory.")

        # 2) Create the path for the coldkey (folder name matches the coldkey name)
        coldkey_dir = os.path.join(self.base_dir, name)
        # Prevent overwriting an existing directory for a coldkey
        if os.path.exists(coldkey_dir):
            # Use console for error message
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Coldkey folder '{coldkey_dir}' already exists."
            )
            raise Exception(f"Coldkey folder '{coldkey_dir}' already exists.")

        os.makedirs(coldkey_dir, exist_ok=True)

        # Create a Fernet cipher suite using the user-provided password + salt
        cipher_suite = get_cipher_suite(password, coldkey_dir)

        # 3) Generate the mnemonic
        mnemonic = str(
            Bip39MnemonicGenerator(lang=Bip39Languages.ENGLISH).FromWordsNumber(
                words_num
            )
        )
        # --- Print mnemonic to console ---
        console.print(f"\n[bold yellow]🔑 Generated Mnemonic:[/bold yellow] {mnemonic}")
        console.print(
            "[bold red]🚨 IMPORTANT: Store this mnemonic phrase securely! It cannot be recovered if lost. 🚨[/bold red]\n"
        )
        # Change logger message to debug level
        logger.debug(
            f":information_source: [dim]Mnemonic generated for Cold Key '{name}'.[/dim]"
        )

        # Encrypt and save the mnemonic in "mnemonic.enc"
        enc_path = os.path.join(coldkey_dir, "mnemonic.enc")
        with open(enc_path, "wb") as f:
            f.write(cipher_suite.encrypt(mnemonic.encode("utf-8")))

        # 4) Initialize an HDWallet from the generated mnemonic
        hdwallet = L1HDWallet.from_mnemonic(mnemonic)

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

        # Use rich markup for final success log message
        logger.info(
            f":heavy_check_mark: [bold green]Cold Key '{name}' created successfully.[/bold green]"
        )

    def load_coldkey(self, name: str, password: str):
        """
        Load an existing coldkey, derive its keys, and return key information.
        Uses Extended Keys based on HotKeyManager pattern.

        Steps:
          1. Reads 'mnemonic.enc', 'salt.bin'.
          2. Decrypts the mnemonic using the provided password.
          3. Initializes an HDWallet from the mnemonic.
          4. Derives standard payment and stake *extended* key pairs (account 0, index 0).
          5. Derives the standard base address using the *extended* verification key hashes.
          6. Returns a dictionary containing essential extended keys and address.

        Args:
            name (str): The coldkey name (folder) to load.
            password (str): Password used to decrypt the mnemonic.

        Returns:
            dict: A dictionary containing 'mnemonic', 'payment_xsk', 'stake_xsk',
                  'payment_address', 'cipher_suite'.
                  Returns None if loading or decryption fails.
        """
        console = Console()
        coldkey_dir = os.path.join(self.base_dir, name)
        mnemonic_path = os.path.join(coldkey_dir, "mnemonic.enc")
        # hotkey_path seems unused in this revised logic, only for internal state update?
        # hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")

        if not os.path.exists(mnemonic_path):
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] mnemonic.enc not found for Cold Key '{name}'."
            )
            return None

        try:
            cipher_suite = get_cipher_suite(password, coldkey_dir)
            with open(mnemonic_path, "rb") as f:
                encrypted_mnemonic = f.read()
            mnemonic = cipher_suite.decrypt(encrypted_mnemonic).decode("utf-8")

            hdwallet = L1HDWallet.from_mnemonic(mnemonic)

            # --- Derive Layer 1 keys using standard BIP44 paths ---
            # m/44'/0'/0'/0/0 - Payment key (receiving/sending)
            payment_keypair = hdwallet.derive_key("m/44'/0'/0'/0/0")
            
            # m/44'/0'/1'/0/0 - Stake key (for staking operations)
            # Using different account (1) for separation
            stake_keypair = hdwallet.derive_key("m/44'/0'/1'/0/0")
            # m/44'/0'/1'/0/0 - Stake key (for staking operations)
            # Using different account (1) for separation
            stake_keypair = hdwallet.derive_key("m/44'/0'/1'/0/0")
            
            # Get Layer 1 address from payment key
            payment_address = L1Address(address=payment_keypair.address())
            
            # For backward compatibility, store keypairs as extended key equivalents
            payment_xsk = payment_keypair  # Store payment keypair
            stake_xsk = stake_keypair  # Store stake keypair separately

            # Update internal state (Optional - Load hotkeys if needed for other methods)
            hotkeys_data = {}
            hotkey_path = os.path.join(coldkey_dir, "hotkeys.json")
            if os.path.exists(hotkey_path):
                try:
                    with open(hotkey_path, "r") as f:
                        hotkeys_data = json.load(f)
                    if "hotkeys" not in hotkeys_data:
                        hotkeys_data["hotkeys"] = {}
                except json.JSONDecodeError:
                    logger.warning(
                        f"Could not decode hotkeys.json for {name}, initializing empty."
                    )
                    hotkeys_data["hotkeys"] = {}
            else:
                hotkeys_data["hotkeys"] = {}

            self.coldkeys[name] = {
                "wallet": hdwallet,
                "cipher_suite": cipher_suite,
                "hotkeys": hotkeys_data.get("hotkeys", {}),
            }

            logger.info(
                f":key: [bold blue]Cold Key '{name}' loaded and keys derived successfully.[/bold blue]"
            )

            # --- Return the derived extended keys and address ---
            return {
                "mnemonic": mnemonic,
                "payment_xsk": payment_xsk,
                "stake_xsk": stake_xsk,
                "payment_address": payment_address,
                "cipher_suite": cipher_suite,
            }
            # --- End Return ---

        except InvalidToken:
            console.print(
                ":cross_mark: [bold red]Error:[/bold red] Invalid password: failed to decrypt mnemonic."
            )
            return None
        except Exception as e:
            logger.error(
                f":cross_mark: [bold red]Error loading coldkey '{name}': {e}[/bold red]"
            )
            console.print_exception(show_locals=False)
            return None

    def restore_coldkey_from_mnemonic(
        self, name: str, mnemonic: str, new_password: str, force: bool = False
    ):
        """
        Restores a coldkey from a mnemonic, creating the necessary files.

        Steps:
          1. Validate the provided mnemonic phrase.
          2. Check if the coldkey directory already exists (handle `force` flag).
          3. Create the directory if needed.
          4. Get/Create a new salt.
          5. Generate a cipher suite using the *new_password* and salt.
          6. Encrypt the *mnemonic* and save to 'mnemonic.enc'.
          7. Initialize HDWallet from the mnemonic.
          8. Create an empty 'hotkeys.json'.
          9. Store the restored coldkey data (wallet, cipher_suite, empty hotkeys) in memory.

        Args:
            name (str): Name for the coldkey.
            mnemonic (str): The mnemonic phrase.
            new_password (str): The new password to encrypt the mnemonic.
            force (bool): Overwrite if the coldkey directory exists.

        Raises:
            Bip39InvalidMnemonicException: If the provided mnemonic is invalid.
            FileExistsError: If the coldkey directory exists and force is False.
            Exception: For other file operation errors.
        """
        console = Console()

        # 1) Normalize and Validate mnemonic using HDWallet.from_mnemonic
        try:
            # Normalize: remove leading/trailing spaces, replace multiple spaces with single space
            normalized_mnemonic = " ".join(mnemonic.strip().split())
            print(
                f"[DEBUG NORMALIZED MNEMONIC]: {normalized_mnemonic!r}"
            )  # Debug normalized

            # Initialize HDWallet - this implicitly validates the mnemonic
            hdwallet = L1HDWallet.from_mnemonic(normalized_mnemonic)
            logger.debug(
                f"Mnemonic phrase for '{name}' passed validation via HDWallet initialization."
            )

        except (
            ValueError
        ) as e:  # HDWallet.from_mnemonic raises ValueError on invalid mnemonic
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] The provided mnemonic phrase is invalid: {e}"
            )
            raise  # Re-raise the ValueError

        # --- Mnemonic is valid if we reach here ---

        # 2) Check if directory exists
        coldkey_dir = os.path.join(self.base_dir, name)
        if os.path.exists(coldkey_dir):
            if force:
                console.print(
                    f":warning: [yellow]Coldkey directory '{coldkey_dir}' already exists. Overwriting due to --force flag.[/yellow]"
                )
                # We might need to remove existing files first, especially salt?
                # Or let get_cipher_suite handle salt potentially
            else:
                raise FileExistsError(
                    f"Coldkey directory '{coldkey_dir}' already exists. Use --force to overwrite."
                )
        else:
            os.makedirs(coldkey_dir, exist_ok=True)

        # 3 & 4) Get/Create salt and cipher suite with NEW password
        # If dir existed and force=True, this will reuse salt unless we delete it first.
        # Let's explicitly remove old salt if overwriting.
        salt_path = os.path.join(coldkey_dir, "salt.bin")
        if force and os.path.exists(salt_path):
            try:
                os.remove(salt_path)
                logger.debug(
                    f":wastebasket: Removed existing salt file at {salt_path} during forced restore."
                )
            except OSError as e:
                logger.error(
                    f":cross_mark: [red]Failed to remove existing salt file {salt_path}: {e}. Proceeding might use old salt.[/red]"
                )
        # Now get_cipher_suite will generate a new salt if needed
        cipher_suite = get_cipher_suite(new_password, coldkey_dir)

        # 5) Encrypt and save the provided mnemonic
        enc_path = os.path.join(coldkey_dir, "mnemonic.enc")
        try:
            with open(enc_path, "wb") as f:
                f.write(cipher_suite.encrypt(normalized_mnemonic.encode("utf-8")))
            logger.debug(f":lock: Encrypted and saved mnemonic to {enc_path}")
        except OSError as e:
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Failed to write encrypted mnemonic to {enc_path}: {e}"
            )
            raise

        # 7) Create/Overwrite 'hotkeys.json'
        hotkeys_path = os.path.join(coldkey_dir, "hotkeys.json")
        try:
            with open(hotkeys_path, "w") as f:
                json.dump({"hotkeys": {}}, f)  # Start with empty hotkeys
            logger.debug(
                f":page_facing_up: Created/overwrote empty hotkeys file at {hotkeys_path}"
            )
        except OSError as e:
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Failed to write hotkeys file to {hotkeys_path}: {e}"
            )
            # Continue? Or raise? Let's continue but log error.
            logger.error(f"Could not create/overwrite hotkeys.json at {hotkeys_path}")

        # 8) Store the restored coldkey data in memory
        self.coldkeys[name] = {
            "wallet": hdwallet,
            "cipher_suite": cipher_suite,
            "hotkeys": {},
        }

        console.print(
            f":heavy_check_mark: [bold green]Cold Key '{name}' restored successfully from mnemonic.[/bold green]"
        )
