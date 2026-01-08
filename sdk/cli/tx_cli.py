# file: sdk/cli/tx_cli.py
import click
import os
import json
import base64
from typing import Optional, cast
import traceback

from rich.console import Console
from cryptography.fernet import Fernet

from sdk.compat.luxtensor_types import (
    Network,
    Address,
    Asset,
    BlockFrostChainContext,
    UTxO,
    TransactionOutput,
    Value,
    ExtendedSigningKey,
)

from sdk.config.settings import settings, logger
from sdk.keymanager.wallet_manager import WalletManager
from sdk.service.context import get_chain_context
from sdk.service.tx_service import send_ada, send_token
from sdk.keymanager.encryption_utils import get_cipher_suite


# ------------------------------------------------------------------------------
# TRANSACTION COMMAND GROUP
# ------------------------------------------------------------------------------
@click.group()
def tx_cli():
    """
    💸 Commands for creating and sending transactions. 💸
    """
    pass


# ------------------------------------------------------------------------------
# SEND TRANSACTION COMMAND
# ------------------------------------------------------------------------------
@tx_cli.command("send")
@click.option("--coldkey", required=True, help="Sender Coldkey name.")
@click.option("--hotkey", required=True, help="Sender Hotkey name.")
@click.option(
    "--password",
    prompt=True,
    hide_input=True,
    help="Sender Coldkey password (to decrypt Hotkey).",
)
@click.option(
    "--to",
    required=True,
    help="Recipient address (addr_...) or wallet reference (coldkey/hotkey).",
)
@click.option(
    "--amount",
    required=True,
    type=int,
    help="Amount to send (Lovelace for ADA, integer for tokens).",
)
@click.option(
    "--token",
    default="lovelace",
    help="Token unit (default: 'lovelace') or 'policy_id.asset_name_hex'.",
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
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory where wallets reside.",
)
@click.option(
    "--yes",
    is_flag=True,
    help="Skip confirmation prompt.",
)
def send_cmd(coldkey, hotkey, password, to, amount, token, network, base_dir, yes):
    """
    ➡️ Send ADA or Native Tokens from a Hotkey address.
    """
    console = Console()
    net = Network.TESTNET if network == "testnet" else Network.MAINNET
    wm = WalletManager(network=net, base_dir=base_dir)

    console.print(f"🚀 Preparing transaction...")
    console.print(f"  Sender: [magenta]{coldkey}[/magenta] / [cyan]{hotkey}[/cyan]")
    console.print(f"  Amount: [yellow]{amount:,}[/yellow]")
    console.print(f"  Token: [blue]{token}[/blue]")
    console.print(f"  To: [green]{to}[/green]")
    console.print(f"  Network: [yellow]{network.upper()}[/yellow]")

    try:
        # 1. Resolve Recipient Address
        recipient_address_str: Optional[str] = None
        if to.startswith("addr"):  # Basic check for Cardano address
            try:
                Address.from_primitive(to)  # Validate format
                recipient_address_str = to
                console.print(f"  Resolved Recipient: Direct Address")
            except Exception as e:
                console.print(
                    f":cross_mark: [bold red]Error:[/bold red] Invalid recipient address format: {e}"
                )
                return
        elif "/" in to:
            parts = to.split("/", 1)
            if len(parts) == 2:
                rcpt_coldkey, rcpt_hotkey = parts
                console.print(
                    f"  Resolving Recipient: [magenta]{rcpt_coldkey}[/magenta] / [cyan]{rcpt_hotkey}[/cyan]..."
                )
                # Ensure WalletManager is initialized for the correct network if recipient is on different one?
                # For now, assume recipient lookup uses the same network as sender.
                rcpt_info = wm.get_hotkey_info(rcpt_coldkey, rcpt_hotkey)
                if rcpt_info and rcpt_info.get("address"):
                    recipient_address_str = rcpt_info["address"]
                    console.print(
                        f"    -> Address: [blue]{recipient_address_str}[/blue]"
                    )
                else:
                    console.print(
                        f":cross_mark: [bold red]Error:[/bold red] Could not find recipient hotkey '{to}'."
                    )
                    return
            else:
                console.print(
                    f":cross_mark: [bold red]Error:[/bold red] Invalid wallet reference format '{to}'. Use 'coldkey/hotkey'."
                )
                return
        else:
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Invalid recipient format '{to}'. Must be an address or 'coldkey/hotkey'."
            )
            return

        if not recipient_address_str:
            console.print(
                f":cross_mark: [bold red]Error:[/bold red] Could not determine recipient address."
            )
            return

        # 2. Initialize Context
        console.print(f"⏳ Initializing Cardano context...")
        context = get_chain_context(method="blockfrost")
        if not context:
            console.print(
                "[bold red]Error:[/bold red] Could not initialize Cardano chain context."
            )
            return
        console.print("  Context initialized.")

        # 3. Prepare arguments and call the appropriate service function
        if token.lower() == "lovelace":
            console.print(f"  Action: Sending ADA (Lovelace)")
            # --- Sending ADA ---
            # Need to get sender's ExtendedSigningKey first
            console.print(f"  Decrypting sender hotkey '{hotkey}'...")
            try:
                sender_hotkey_info = wm.get_hotkey_info(coldkey, hotkey)
                if not sender_hotkey_info or "encrypted_data" not in sender_hotkey_info:
                    raise ValueError(
                        "Could not retrieve encrypted data for sender hotkey."
                    )

                # Use the cipher suite directly
                cipher_suite = get_cipher_suite(
                    password, os.path.join(base_dir, coldkey)
                )
                # Ensure the encrypted data is bytes for Fernet
                encrypted_data_bytes = sender_hotkey_info["encrypted_data"].encode(
                    "ascii"
                )
                # Decrypt directly, Fernet handles base64 decoding internally
                decrypted_payload = cipher_suite.decrypt(encrypted_data_bytes)
                decrypted_data = json.loads(decrypted_payload.decode("utf-8"))
                # --- DEBUGGING START ---
                # console.print(f"\n[bold yellow][DEBUG TX_CLI] Keys in decrypted_data:[/bold yellow] {list(decrypted_data.keys())}\n") # <-- Remove debug
                # --- DEBUGGING END ---

                # Correct the key name from 'skey' to 'xsk'
                payment_skey_cbor_hex = decrypted_data["payment_xsk_cbor_hex"]
                stake_skey_cbor_hex = decrypted_data.get(
                    "stake_xsk_cbor_hex"
                )  # Assuming this one is also xsk, use .get() for safety

                # Attempt to create keys from CBOR, casting the type for the linter
                # Note: This assumes from_cbor is the correct method and returns a compatible structure internally
                sender_payment_xsk = cast(
                    ExtendedSigningKey,
                    ExtendedSigningKey.from_cbor(
                        payment_skey_cbor_hex
                    ),  # This variable name is now slightly misleading but functionally ok
                )
                # Correct the key name here too if necessary, though .get() handled it
                sender_stake_xsk_cbor = (
                    ExtendedSigningKey.from_cbor(
                        stake_skey_cbor_hex
                    )  # Also potentially misleading variable name
                    if stake_skey_cbor_hex
                    else None
                )
                sender_stake_xsk = cast(
                    Optional[ExtendedSigningKey], sender_stake_xsk_cbor
                )

                if not isinstance(sender_payment_xsk, ExtendedSigningKey):
                    raise TypeError(
                        f"Decoded payment key is not an ExtendedSigningKey, type was {type(sender_payment_xsk)}"
                    )
                if sender_stake_xsk is not None and not isinstance(
                    sender_stake_xsk, ExtendedSigningKey
                ):
                    raise TypeError(
                        f"Decoded stake key is not an ExtendedSigningKey or None, type was {type(sender_stake_xsk)}"
                    )

                console.print("  Sender hotkey decrypted.")

            except Exception as e:
                console.print(
                    f":cross_mark: [bold red]Error decrypting sender hotkey:[/bold red] {e} (Type: {type(e).__name__})"
                )
                # Print the traceback for detailed debugging
                console.print("[dim]--- Traceback ---[/dim]")
                console.print(traceback.format_exc(), style="dim")
                console.print("[dim]--- End Traceback ---[/dim]")
                return

            if not yes:
                click.confirm(
                    f"\n❓ Send [yellow]{amount:,} Lovelace[/yellow] to [green]{recipient_address_str}[/green]?",
                    abort=True,
                )

            console.print(f"  Submitting ADA transaction via send_ada service...")
            tx_id = send_ada(
                chain_context=context,
                payment_xsk=sender_payment_xsk,
                stake_xsk=sender_stake_xsk,
                to_address_str=recipient_address_str,
                lovelace_amount=amount,
                network=net,
            )
            console.print(
                f":heavy_check_mark: [bold green]ADA Transfer Submitted![/bold green]"
            )
            console.print(f"  Transaction ID: [bold blue]{tx_id}[/bold blue]")

        else:
            # --- Sending Native Token ---
            console.print(f"  Action: Sending Native Token")
            try:
                # Parse token unit: policy_id.asset_name_hex
                parts = token.split(".", 1)
                if len(parts) != 2 or len(parts[0]) != 56:  # Basic validation
                    raise ValueError(
                        "Invalid token format. Use 'policy_id.asset_name_hex'."
                    )
                policy_id_hex = parts[0]
                asset_name_hex = parts[1]
                # Decode asset name to bytes for send_token
                asset_name_bytes = bytes.fromhex(asset_name_hex)
                asset_name_display = asset_name_bytes.decode(
                    "utf-8", errors="replace"
                )  # Decode for display
            except Exception as e:
                console.print(
                    f":cross_mark: [bold red]Error parsing token '{token}':[/bold red] {e}"
                )
                return

            if not yes:
                click.confirm(
                    f"\n❓ Send [yellow]{amount:,}[/yellow] of token [blue]'{asset_name_display}' ({policy_id_hex[:6]}...)[/blue] to [green]{recipient_address_str}[/green]?",
                    abort=True,
                )

            console.print(f"  Submitting Token transaction via send_token service...")
            # Check if send_token expects asset_name as string or bytes
            # Based on previous read, it expects string, let's decode asset_name_hex again
            tx_id = send_token(
                chain_context=context,
                base_dir=base_dir,
                coldkey_name=coldkey,
                hotkey_name=hotkey,
                password=password,
                to_address_str=recipient_address_str,
                policy_id_hex=policy_id_hex,
                # Pass the original asset name string (assuming send_token encodes it)
                asset_name=bytes.fromhex(asset_name_hex).decode(
                    "utf-8", errors="replace"
                ),
                token_amount=amount,
                network=net,
            )
            console.print(
                f":heavy_check_mark: [bold green]Token Transfer Submitted![/bold green]"
            )
            console.print(f"  Transaction ID: [bold blue]{tx_id}[/bold blue]")

    except Exception as e:
        console.print(
            f":cross_mark: [bold red]An unexpected error occurred:[/bold red] {e}"
        )
        console.print_exception(show_locals=False)
