# file: sdk/cli/wallet_cli.py

import click
from rich.tree import Tree
from rich.console import Console
import os  # Import os for path operations
import traceback  # Import traceback for detailed error logging
import hashlib  # Import the standard hashlib library
from typing import Optional  # Import Optional

# --- Move settings import near the top ---
# This ensures logging is configured before other modules get loggers
from sdk.config.settings import settings  # Load settings for defaults

from pycardano import (
    Network,
    PlutusV3Script,
    ScriptHash,
    Address,
    PaymentSigningKey,
    StakeSigningKey,
    plutus_script_hash,
    hash,
)
import cbor2
from sdk.keymanager.wallet_manager import WalletManager
from sdk.service.register_key import register_key
from sdk.service.context import get_chain_context  # Needed for context

# Import function to read validator details
from sdk.smartcontract.validator import read_validator

# Import Datum definition
from sdk.metagraph.metagraph_datum import MinerDatum, STATUS_ACTIVE

# Import helper to get current slot
# from sdk.utils.cardano_utils import get_current_slot # Assume this exists
# Import hash_data for empty history hash
from sdk.metagraph.hash.hash_datum import hash_data
import hashlib  # Import the standard hashlib library


@click.group()
def wallet_cli():
    """
    ✨ CLI for managing ModernTensor wallets (Coldkeys & Hotkeys). ✨
    """
    pass


# ------------------------------------------------------------------------------
# 1) CREATE COLDKEY
# ------------------------------------------------------------------------------
@wallet_cli.command("create-coldkey")
@click.option("--name", required=True, help="Coldkey name.")
@click.option(
    "--password",
    prompt=True,
    hide_input=True,
    confirmation_prompt=True,
    help="mnemonic encryption password.",
)
@click.option(
    "--base-dir", default="moderntensor", help="The base directory stores coldkeys."
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
def create_coldkey_cmd(name, password, base_dir, network):
    """
    🔑 Generate a new Coldkey mnemonic, encrypt it with a password,
    and save it to the specified base directory.
    Displays the mnemonic phrase upon creation - store it securely!
    """
    net = Network.TESTNET if network == "testnet" else Network.MAINNET
    wm = WalletManager(network=net, base_dir=base_dir)
    wm.create_coldkey(name, password)
    console = Console()
    console.print(
        f":heavy_check_mark: [bold green]Coldkey[/bold green] [magenta]'{name}'[/magenta] [bold green]created in directory[/bold green] '[blue]{base_dir}[/blue]'[bold green].[/bold green]"
    )


# ------------------------------------------------------------------------------
# 2) LOAD COLDKEY -> RESTORE COLDKEY
# ------------------------------------------------------------------------------
@wallet_cli.command("restore-coldkey")
@click.option("--name", required=True, help="Name for the restored coldkey.")
@click.option(
    "--mnemonic",
    prompt="Enter the mnemonic phrase (words separated by spaces)",
    help="The 12/15/18/21/24 word mnemonic phrase.",
)
@click.option(
    "--new-password",
    prompt="Set a new password for this coldkey",
    hide_input=True,
    confirmation_prompt=True,
    help="New password to encrypt the restored mnemonic.",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory to restore the coldkey into (e.g., ./moderntensor).",
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
@click.option(
    "--force",
    is_flag=True,
    help="Overwrite existing coldkey folder and its contents if it exists.",
)
def restore_coldkey_cmd(name, mnemonic, new_password, base_dir, network, force):
    """
    Restore a coldkey from its 12/15/18/21/24 word mnemonic phrase.

    This command recreates the coldkey's encrypted mnemonic file (`mnemonic.enc`)
    using a NEW password you provide. It also generates the associated salt file
    and an empty hotkeys file (`hotkeys.json`). Useful for recovery if you only
    have the mnemonic phrase.
    """
    print(f"DEBUG MNEMONIC RECEIVED: {mnemonic!r}")
    console = Console()
    console.print(
        f":recycle: Attempting to restore coldkey [magenta]'{name}'[/magenta] from mnemonic..."
    )

    try:
        net = Network.TESTNET if network == "testnet" else Network.MAINNET
        wm = WalletManager(network=net, base_dir=base_dir)

        # Validate mnemonic basic format (simple check)
        if not mnemonic or len(mnemonic.split()) not in [12, 15, 18, 21, 24]:
            console.print(
                ":cross_mark: [bold red]Error:[/bold red] Invalid mnemonic phrase format. Please provide words separated by spaces."
            )
            return

        # Call the new restore method (to be implemented in WalletManager/ColdKeyManager)
        wm.restore_coldkey_from_mnemonic(name, mnemonic, new_password, force)

        # Confirmation message handled by the manager method now
        # console.print(f":heavy_check_mark: [bold green]Coldkey '{name}' restored successfully in '{base_dir}'.[/bold green]")

    except FileExistsError as e:
        console.print(f":stop_sign: [bold red]Error:[/bold red] {e}")
        console.print("Use the --force flag to overwrite.")
    except Exception as e:
        console.print(f":cross_mark: [bold red]Error restoring coldkey:[/bold red] {e}")
        console.print_exception(show_locals=True)


# ------------------------------------------------------------------------------
# 3) GENERATE HOTKEY
# ------------------------------------------------------------------------------
@wallet_cli.command("generate-hotkey")
@click.option("--coldkey", required=True, help="Coldkey name.")
@click.option("--hotkey-name", required=True, help="Hotkey name.")
@click.option("--base-dir", default="moderntensor", help="Base directory.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Network.",
)
def generate_hotkey_cmd(coldkey, hotkey_name, base_dir, network):
    """
    Create a new Hotkey derived from a loaded Coldkey.

    Derives a new payment/stake key pair using the next available HD wallet
    derivation index for the specified coldkey. It encrypts the signing keys
    and saves the hotkey info (name, address, encrypted data) to the
    `hotkeys.json` file within the coldkey's directory.

    The derivation index used will be printed upon successful creation.
    Make sure to save this index for potential recovery using `regen-hotkey`.
    """
    net = Network.TESTNET if network == "testnet" else Network.MAINNET
    wm = WalletManager(network=net, base_dir=base_dir)

    password = click.prompt("Enter the coldkey password", hide_input=True)
    wm.load_coldkey(coldkey, password)
    encrypted_data = wm.generate_hotkey(coldkey, hotkey_name)
    console = Console()
    console.print(
        f":sparkles: [bold green]Hotkey[/bold green] [cyan]'{hotkey_name}'[/cyan] [bold green]created for coldkey[/bold green] [magenta]'{coldkey}'[/magenta][bold green].[/bold green]"
    )


# ------------------------------------------------------------------------------
# 4) IMPORT HOTKEY
# ------------------------------------------------------------------------------
@wallet_cli.command("import-hotkey")
@click.option("--coldkey", required=True, help="Coldkey name.")
@click.option(
    "--encrypted-hotkey",
    required=True,
    help="The exported encrypted hotkey data string (base64 format).",
)
@click.option(
    "--hotkey-name", required=True, help="Name to assign to the imported Hotkey."
)
@click.option(
    "--overwrite",
    is_flag=True,
    default=False,
    help="Overwrite the existing hotkey entry if a key with the same name exists.",
)
@click.option(
    "--base-dir",
    default="moderntensor",
    help="Base directory where the parent coldkey resides.",
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select the Cardano network (testnet/mainnet).",
)
def import_hotkey_cmd(
    coldkey, encrypted_hotkey, hotkey_name, overwrite, base_dir, network
):
    """
    Import an exported/generated encrypted hotkey string.

    This decrypts the provided string using the coldkey's password to verify it,
    then saves the entry (name, address, original encrypted data) into the
    coldkey's `hotkeys.json` file.
    """
    net = Network.TESTNET if network == "testnet" else Network.MAINNET
    wm = WalletManager(network=net, base_dir=base_dir)

    password = click.prompt("Enter the coldkey password", hide_input=True)
    wm.load_coldkey(coldkey, password)

    wm.import_hotkey(coldkey, encrypted_hotkey, hotkey_name, overwrite=overwrite)
    console = Console()
    console.print(
        f":inbox_tray: [bold green]Hotkey[/bold green] [cyan]'{hotkey_name}'[/cyan] [bold green]imported successfully for coldkey[/bold green] [magenta]'{coldkey}'[/magenta][bold green].[/bold green]"
    )


# ------------------------------------------------------------------------------
# 4.5) REGENERATE HOTKEY (from index)
# ------------------------------------------------------------------------------
@wallet_cli.command("regen-hotkey")
@click.option("--coldkey", required=True, help="Name of the parent Coldkey.")
@click.option(
    "--hotkey-name",
    required=True,
    help="Name to assign/find for the regenerated hotkey.",
)
@click.option(
    "--index",
    required=True,
    type=int,
    help="The derivation index (e.g., 0, 1, 2...) used when the hotkey was originally generated.",
)
@click.option(
    "--password",
    prompt=True,
    hide_input=True,
    help="Password for the parent Coldkey.",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory where the parent coldkey resides.",
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Overwrite existing hotkey entry in hotkeys.json if it exists.",
)
@click.option(
    "--force", is_flag=True, help="Overwrite existing hotkey entry if it exists."
)
def regen_hotkey_cmd(coldkey, hotkey_name, index, password, base_dir, network, force):
    """
    Regenerate a hotkey's data (keys, address, encrypted entry) from its parent
    coldkey and derivation index. Useful for recovery if hotkeys.json is lost
    or to ensure consistency.
    """
    console = Console()
    console.print(
        f":repeat: Attempting to regenerate hotkey [cyan]'{hotkey_name}'[/cyan] from coldkey [magenta]'{coldkey}'[/magenta] at index [yellow]{index}[/yellow]..."
    )

    if index < 0:
        console.print(
            ":cross_mark: [bold red]Error:[/bold red] Derivation index must be a non-negative integer."
        )
        return

    try:
        net = Network.TESTNET if network == "testnet" else Network.MAINNET
        wm = WalletManager(network=net, base_dir=base_dir)

        # Load the necessary coldkey first
        console.print(f"⏳ Loading parent coldkey [magenta]'{coldkey}'[/magenta]...")
        wm.load_coldkey(coldkey, password)

        # Call the new regenerate method
        wm.regenerate_hotkey(coldkey, hotkey_name, index, force)

        # Success message is handled within the manager method

    except FileNotFoundError as e:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Coldkey '{coldkey}' or its files not found: {e}"
        )
    except KeyError as e:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Coldkey '{coldkey}' not loaded or issue with its data: {e}"
        )
    except ValueError as e:
        console.print(f":cross_mark: [bold red]Error:[/bold red] {e}")
    except Exception as e:
        console.print(
            f":cross_mark: [bold red]Error regenerating hotkey:[/bold red] {e}"
        )
        console.print_exception(show_locals=True)


# ------------------------------------------------------------------------------
# 5) LIST KEY
# ------------------------------------------------------------------------------


@wallet_cli.command(name="list")
def list_coldkeys():
    """
    List all Coldkeys found in the base directory and their associated Hotkeys
    (names and addresses from `hotkeys.json`).
    """
    manager = WalletManager()
    wallets = manager.load_all_wallets()

    console = Console()

    if not wallets:
        console.print("[bold red]No coldkeys found.[/bold red]")
        return

    root_tree = Tree("[bold cyan]Wallets[/bold cyan]")

    for w in wallets:
        # Tạo node con cho Coldkey
        coldkey_node = root_tree.add(
            f":closed_lock_with_key: [bold yellow]Coldkey[/bold yellow] [magenta]{w['name']}[/magenta]"
        )

        # hotkeys
        if w.get("hotkeys"):
            for hk in w["hotkeys"]:
                hk_addr = hk["address"] or "unknown"
                # Use a different key icon for hotkey
                coldkey_node.add(
                    f":key: Hotkey [green]{hk['name']}[/green] (addr=[dim]{hk_addr}[/dim])"
                )

    console.print(root_tree)


# ------------------------------------------------------------------------------
# 6) REGISTER HOTKEY (via Smart Contract Update)
# ------------------------------------------------------------------------------
@wallet_cli.command("register-hotkey")
@click.option(
    "--coldkey", required=True, help="Name of the Coldkey controlling the Hotkey."
)
@click.option(
    "--hotkey",
    required=True,
    help="Name of the Hotkey to register (will be used as UID).",
)
@click.option(
    "--password",
    prompt=True,
    hide_input=True,
    help="Password for the controlling Coldkey.",
)
@click.option(
    "--subnet-uid",
    required=True,
    type=int,
    help="Base directory where the keys reside.",
)
@click.option(
    "--initial-stake",
    required=True,
    type=int,
    help="Initial stake amount (in Lovelace).",
)
@click.option(
    "--api-endpoint",
    required=True,  # Now required for MinerDatum
    help="Full API endpoint URL for this hotkey (e.g., http://<ip>:<port>).",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory for keys.",
)
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if settings.CARDANO_NETWORK == Network.MAINNET else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
@click.option(
    "--yes",
    is_flag=True,
    help="Skip the final confirmation prompt before submitting.",
)
def register_hotkey_cmd(
    coldkey,
    hotkey,
    password,
    subnet_uid,
    initial_stake,
    api_endpoint,
    base_dir,
    network,
    yes,
):
    """
    Register a specific Hotkey as a Miner on the ModernTensor network.

    This involves creating/updating a UTxO at the validator contract address.
    The UTxO's datum will contain the Miner's information (UID, stake, API endpoint, etc.).
    Requires the Coldkey's password to access the necessary signing keys.
    """
    console = Console()
    console.print(
        f":rocket: Attempting to register hotkey [cyan]'{hotkey}'[/cyan] as a Miner..."
    )

    try:
        net = Network.TESTNET if network == "testnet" else Network.MAINNET
        wm = WalletManager(network=net, base_dir=base_dir)

        # --- Load Keys ---
        console.print(f"⏳ Loading coldkey [magenta]'{coldkey}'[/magenta]...")
        coldkey_data = wm.load_coldkey(coldkey, password)
        if (
            not coldkey_data
            or not coldkey_data.get("payment_skey")
            or not coldkey_data.get("payment_address")
        ):
            console.print(
                f"[bold red]Error:[/bold red] Failed to load coldkey '{coldkey}' or its signing key/address."
            )
            return
        cold_payment_skey: PaymentSigningKey = coldkey_data["payment_skey"]
        cold_address: Address = coldkey_data["payment_address"]
        cold_stake_skey: Optional[StakeSigningKey] = coldkey_data.get("stake_skey")
        console.print(
            f":key: Coldkey '{coldkey}' loaded (Address: [dim]{cold_address}[/dim])."
        )

        console.print(f"⏳ Verifying hotkey [cyan]'{hotkey}'[/cyan]...")
        hotkey_info = wm.get_hotkey_info(coldkey, hotkey)
        if not hotkey_info or not hotkey_info.get("address"):
            console.print(
                f"[bold red]Error:[/bold red] Failed to find hotkey '{hotkey}' info. Generate it first?"
            )
            return
        hot_address_str = hotkey_info["address"]
        console.print(
            f":key: Hotkey '{hotkey}' found (Address: [dim]{hot_address_str}[/dim])."
        )

        # --- Blockchain Interaction (Initialize Context FIRST) ---
        console.print("⏳ Initializing Cardano context...")
        context = get_chain_context(method="blockfrost")  # Assumes blockfrost
        if not context:
            console.print(
                "[bold red]Error:[/bold red] Could not initialize Cardano chain context."
            )
            return
        console.print(
            f":globe_with_meridians: Cardano context initialized (Network: [yellow]{net}[/yellow])."
        )

        # --- Load Script --- (Moved after context init)
        console.print("⏳ Loading validator script details...")
        try:
            validator_details = read_validator()
            if (
                not validator_details
                or "script" not in validator_details
                or "script_hash" not in validator_details
            ):
                console.print(
                    "[bold red]Error:[/bold red] Failed to load valid script details (script or hash missing) using read_validator."
                )
                return
            script: PlutusV3Script = validator_details["script"]
            script_hash: ScriptHash = validator_details["script_hash"]
            contract_address_obj = Address(payment_part=script_hash, network=net)
            console.print(
                f":scroll: Script details loaded. Script Hash: [yellow]{script_hash.to_primitive()}[/yellow]"
            )
            console.print(
                f":link: Derived Contract Address: [blue]{contract_address_obj}[/blue]"
            )
        except Exception as e:
            console.print(
                f"[bold red]Error:[/bold red] Failed to load validator script details: {e}"
            )
            console.print_exception(show_locals=True)
            return

        # --- Prepare Datum --- (Moved after context init)
        console.print("⏳ Preparing new Miner Datum...")
        current_slot: Optional[int] = None
        try:
            current_slot = context.last_block_slot
            if current_slot is None:
                console.print(
                    "[yellow]Warning:[/yellow] context.last_block_slot returned None."
                )
                current_slot = 0
            console.print(
                f":clock1: Using current slot: [yellow]{current_slot}[/yellow]"
            )
        except Exception as slot_err:
            console.print(
                f"[yellow]Warning:[/yellow] Could not get current slot from context: {slot_err}. Using 0."
            )
            current_slot = 0

        # Convert addresses/UIDs/endpoint to bytes for Datum
        try:
            hotkey_uid_bytes = hotkey.encode("utf-8")
            api_endpoint_bytes = api_endpoint.encode("utf-8")
            wallet_addr_hash_bytes = hashlib.blake2b(
                bytes(cold_address), digest_size=28
            ).digest()
            perf_history_hash_bytes = hash_data([])
        except Exception as e:
            console.print(
                f"[bold red]Error:[/bold red] Failed to encode data for Datum: {e}"
            )
            return

        # Create the new datum instance
        new_datum = MinerDatum(
            uid=hotkey_uid_bytes,
            subnet_uid=subnet_uid,
            stake=initial_stake,
            scaled_last_performance=0,
            scaled_trust_score=0,
            accumulated_rewards=0,
            last_update_slot=current_slot,
            performance_history_hash=perf_history_hash_bytes,
            wallet_addr_hash=wallet_addr_hash_bytes,
            status=STATUS_ACTIVE,
            registration_slot=current_slot,
            api_endpoint=api_endpoint_bytes,
        )
        console.print(":clipboard: New Miner Datum prepared:")
        console.print(f"  UID:            [cyan]{hotkey}[/cyan]")
        console.print(f"  Subnet UID:     [yellow]{subnet_uid}[/yellow]")
        console.print(
            f"  Stake:          [bright_blue]{initial_stake} Lovelace[/bright_blue]"
        )
        console.print(f"  API Endpoint:   [link={api_endpoint}]{api_endpoint}[/link]")
        console.print(f"  Status:         [green]ACTIVE[/green]")
        console.print(f"  Reg/Update Slot:[yellow]{current_slot}[/yellow]")

        # Confirm before proceeding
        if not yes:
            click.confirm(
                f"\n❓ Register hotkey [cyan]'{hotkey}'[/cyan] as Miner on subnet [yellow]{subnet_uid}[/yellow] "
                f"by updating UTxO at contract address [blue]{contract_address_obj}[/blue]?",
                abort=True,
            )

        console.print(
            ":arrow_up: Submitting registration transaction via [bold]register_key[/bold] service..."
        )
        try:
            tx_id = register_key(
                payment_xsk=cold_payment_skey.to_extended_signing_key(),
                stake_xsk=(
                    cold_stake_skey.to_extended_signing_key()
                    if cold_stake_skey
                    else None
                ),
                script_hash=script_hash,
                new_datum=new_datum,
                script=script,
                context=context,
                network=net,
                contract_address=contract_address_obj,
            )

            if tx_id:
                console.print(
                    f":heavy_check_mark: [bold green]Miner registration transaction submitted.[/bold green]"
                )
                console.print(f"  Transaction ID: [bold blue]{tx_id}[/bold blue]")
                console.print(
                    ":hourglass_flowing_sand: Please wait for blockchain confirmation."
                )
            else:
                console.print(
                    "[bold red]Error:[/bold red] Failed to submit registration transaction (No TxID returned)."
                )

        except FileNotFoundError as e:
            console.print(
                f"[bold red]Error:[/bold red] Key file not found: {e}. Have you created the coldkey/hotkey?"
            )
        except ValueError as e:
            console.print(f"[bold red]Error:[/bold red] Registration failed: {e}")
        except Exception as e:
            console.print(
                f"[bold red]Error:[/bold red] An unexpected error occurred during registration: {e}"
            )
            console.print_exception(show_locals=True)

    except FileNotFoundError as e:
        console.print(
            f"[bold red]Error:[/bold red] Initial file/key loading error: {e}"
        )
    except ValueError as e:
        console.print(
            f"[bold red]Error:[/bold red] Invalid value encountered during setup: {e}"
        )
    except Exception as e:
        console.print(
            f"[bold red]Error:[/bold red] An overall unexpected error occurred in register_hotkey_cmd: {e}"
        )
        console.print_exception(show_locals=True)
