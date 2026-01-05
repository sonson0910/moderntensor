# sdk/cli/stake_cli.py
import click
from rich.console import Console
from rich.panel import Panel
from sdk.compat.pycardano import Network, Address
# REMOVED: from blockfrost import ApiError  # Layer 1 does not use Blockfrost
from typing import Any

from sdk.config.settings import settings, logger
from sdk.service.stake_service import Wallet, StakingService


# ------------------------------------------------------------------------------
# STAKE COMMAND GROUP
# ------------------------------------------------------------------------------
@click.group()
def stake_cli():
    """
    🛡️ Commands for Cardano staking operations (delegation, withdrawal). 🛡️
    """
    pass


# Helper function to instantiate Wallet and StakingService
def _get_stake_services(coldkey, hotkey, password, network_str, base_dir):
    console = Console()
    try:
        # Network resolution is handled inside Wallet __init__ now
        wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
        staking_service = StakingService(wallet)
        console.print(
            f"✅ Wallet loaded for [magenta]{coldkey}[/magenta]/[cyan]{hotkey}[/cyan]. Address: [blue]{wallet.main_address}[/blue]"
        )
        return wallet, staking_service
    except FileNotFoundError as e:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Coldkey '{coldkey}' not found at '{base_dir}'. Details: {e}"
        )
        return None, None
    except ValueError as e:
        # Catch errors from Wallet init (e.g., key decode fail)
        console.print(
            f":cross_mark: [bold red]Error initializing wallet:[/bold red] {e}"
        )
        return None, None
    except Exception as e:
        console.print(
            f":cross_mark: [bold red]Unexpected error initializing wallet/service:[/bold red] {e}"
        )
        logger.exception("Wallet/StakingService initialization failed")
        return None, None


# ------------------------------------------------------------------------------
# DELEGATE STAKE COMMAND
# ------------------------------------------------------------------------------
@stake_cli.command("delegate")
@click.option("--coldkey", required=True, help="Coldkey name controlling the stake.")
@click.option("--hotkey", required=True, help="Hotkey whose stake key will be used.")
@click.option("--password", prompt=True, hide_input=True, help="Coldkey password.")
@click.option("--pool-id", required=True, help="Bech32 or Hex Pool ID to delegate to.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if str(settings.CARDANO_NETWORK).lower() == "mainnet" else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory where wallets reside.",
)
@click.option("--yes", is_flag=True, help="Skip confirmation prompt.")
def delegate_cmd(coldkey, hotkey, password, pool_id, network, base_dir, yes):
    """
    📜 Register stake key (if needed) and delegate to a stake pool.
    """
    console = Console()
    wallet, staking_service = _get_stake_services(
        coldkey, hotkey, password, network, base_dir
    )
    if not wallet or not staking_service:
        return

    if not wallet.stake_key_hash or not wallet.stake_sk:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Hotkey '{hotkey}' does not seem to have an associated stake key."
        )
        return

    console.print(
        f"❓ Attempting to delegate stake from [cyan]{hotkey}[/cyan] (Stake Address: [blue]{wallet.stake_address}[/blue]) to Pool ID: [yellow]{pool_id}[/yellow]"
    )

    if not yes:
        click.confirm("This will submit a transaction. Proceed?", abort=True)

    console.print("⏳ Submitting delegation transaction...")
    try:
        tx_id = staking_service.delegate_stake(pool_id)
        if tx_id:
            console.print(
                f":heavy_check_mark: [bold green]Delegation transaction submitted![/bold green]"
            )
            console.print(f"  Transaction ID: [bold blue]{tx_id}[/bold blue]")
        else:
            console.print(
                ":cross_mark: [bold red]Delegation failed. Check logs for details.[/bold red]"
            )
    except Exception as e:
        console.print(f":cross_mark: [bold red]Error during delegation:[/bold red] {e}")
        logger.exception("Delegation command failed")


# ------------------------------------------------------------------------------
# RE-DELEGATE STAKE COMMAND
# ------------------------------------------------------------------------------
@stake_cli.command("redelegate")
@click.option("--coldkey", required=True, help="Coldkey name controlling the stake.")
@click.option("--hotkey", required=True, help="Hotkey whose stake key will be used.")
@click.option("--password", prompt=True, hide_input=True, help="Coldkey password.")
@click.option("--pool-id", required=True, help="Bech32 or Hex Pool ID of the NEW pool.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if str(settings.CARDANO_NETWORK).lower() == "mainnet" else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory where wallets reside.",
)
@click.option("--yes", is_flag=True, help="Skip confirmation prompt.")
def redelegate_cmd(coldkey, hotkey, password, pool_id, network, base_dir, yes):
    """
    🔁 Change delegation to a different stake pool.
    """
    console = Console()
    wallet, staking_service = _get_stake_services(
        coldkey, hotkey, password, network, base_dir
    )
    if not wallet or not staking_service:
        return

    if not wallet.stake_key_hash or not wallet.stake_sk:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Hotkey '{hotkey}' does not seem to have an associated stake key."
        )
        return

    console.print(
        f"❓ Attempting to re-delegate stake from [cyan]{hotkey}[/cyan] to NEW Pool ID: [yellow]{pool_id}[/yellow]"
    )

    if not yes:
        click.confirm("This will submit a transaction. Proceed?", abort=True)

    console.print("⏳ Submitting re-delegation transaction...")
    try:
        tx_id = staking_service.re_delegate_stake(pool_id)
        if tx_id:
            console.print(
                f":heavy_check_mark: [bold green]Re-delegation transaction submitted![/bold green]"
            )
            console.print(f"  Transaction ID: [bold blue]{tx_id}[/bold blue]")
        else:
            console.print(
                ":cross_mark: [bold red]Re-delegation failed. Check logs for details.[/bold red]"
            )
    except Exception as e:
        console.print(
            f":cross_mark: [bold red]Error during re-delegation:[/bold red] {e}"
        )
        logger.exception("Re-delegation command failed")


# ------------------------------------------------------------------------------
# WITHDRAW REWARDS COMMAND
# ------------------------------------------------------------------------------
@stake_cli.command("withdraw")
@click.option("--coldkey", required=True, help="Coldkey name controlling the stake.")
@click.option(
    "--hotkey", required=True, help="Hotkey whose stake address rewards to withdraw."
)
@click.option("--password", prompt=True, hide_input=True, help="Coldkey password.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if str(settings.CARDANO_NETWORK).lower() == "mainnet" else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory where wallets reside.",
)
@click.option("--yes", is_flag=True, help="Skip confirmation prompt.")
def withdraw_cmd(coldkey, hotkey, password, network, base_dir, yes):
    """
    💸 Withdraw available staking rewards to the main wallet address.
    """
    console = Console()
    wallet, staking_service = _get_stake_services(
        coldkey, hotkey, password, network, base_dir
    )
    if not wallet or not staking_service:
        return

    if not wallet.stake_address:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Hotkey '{hotkey}' does not seem to have an associated stake address."
        )
        return

    console.print(
        f"❓ Attempting to withdraw staking rewards for stake address: [blue]{wallet.stake_address}[/blue]"
    )
    try:
        account_info_any: Any = wallet.api.accounts(str(wallet.stake_address))
        reward_amount = int(getattr(account_info_any, "withdrawable_amount", 0))
        console.print(
            f"  Available Rewards: [yellow]{reward_amount / 1_000_000:.6f} ADA[/yellow] ({reward_amount:,} Lovelace)"
        )
        if reward_amount == 0:
            console.print(":information_source: No rewards available to withdraw.")
            return
    except ApiError as e:
        if e.status_code == 404:
            console.print(
                f":information_source: Stake address {wallet.stake_address} not found on the blockchain or no reward history."
            )
            return
        else:
            console.print(
                f":cross_mark: [bold red]API Error checking rewards:[/bold red] Status {e.status_code} - {e.message}"
            )
            return
    except Exception as e:
        console.print(f":cross_mark: [bold red]Error checking rewards:[/bold red] {e}")
        logger.exception("Reward check failed before withdrawal")
        return

    if not yes:
        click.confirm(
            f"Withdraw {reward_amount / 1_000_000:.6f} ADA rewards? This will submit a transaction.",
            abort=True,
        )

    console.print("⏳ Submitting withdrawal transaction...")
    try:
        tx_id = staking_service.withdrawal_reward()
        if tx_id:
            console.print(
                f":heavy_check_mark: [bold green]Withdrawal transaction submitted![/bold green]"
            )
            console.print(f"  Transaction ID: [bold blue]{tx_id}[/bold blue]")
        else:
            console.print(
                ":cross_mark: [bold red]Withdrawal failed unexpectedly. Check logs.[/bold red]"
            )
    except Exception as e:
        console.print(f":cross_mark: [bold red]Error during withdrawal:[/bold red] {e}")
        logger.exception("Withdrawal command failed")


# ------------------------------------------------------------------------------
# STAKE INFO COMMAND
# ------------------------------------------------------------------------------
@stake_cli.command("info")
@click.option("--coldkey", required=True, help="Coldkey name.")
@click.option("--hotkey", required=True, help="Hotkey whose staking info to show.")
@click.option("--password", prompt=True, hide_input=True, help="Coldkey password.")
@click.option(
    "--network",
    default=lambda: (
        "mainnet" if str(settings.CARDANO_NETWORK).lower() == "mainnet" else "testnet"
    ),
    type=click.Choice(["testnet", "mainnet"]),
    help="Select Cardano network.",
)
@click.option(
    "--base-dir",
    default=lambda: settings.HOTKEY_BASE_DIR,
    help="Base directory where wallets reside.",
)
def info_cmd(coldkey, hotkey, password, network, base_dir):
    """
    ℹ️  Show current staking status (delegation, rewards) for a hotkey.
    """
    console = Console()
    wallet, _ = _get_stake_services(coldkey, hotkey, password, network, base_dir)
    if not wallet:
        return

    if not wallet.stake_address:
        console.print(
            f":cross_mark: [bold red]Error:[/bold red] Hotkey '{hotkey}' does not seem to have an associated stake address."
        )
        return

    console.print(
        f"🔍 Querying staking info for stake address: [blue]{wallet.stake_address}[/blue]"
    )

    try:
        account_info_any: Any = wallet.api.accounts(str(wallet.stake_address))

        pool_id = getattr(account_info_any, "pool_id", None)
        rewards = int(getattr(account_info_any, "withdrawable_amount", 0))
        active = getattr(account_info_any, "active", False)

        content = f"[bold]Stake Address:[/bold] [blue]{wallet.stake_address}[/blue]\n"
        content += f"[bold]Active:[/bold] {'[bold green]Yes[/bold green]' if active else '[dim]No[/dim]'}\n"
        content += f"[bold]Available Rewards:[/bold] [yellow]{rewards / 1_000_000:.6f} ADA[/yellow] ({rewards:,} Lovelace)\n"

        if pool_id:
            content += f"[bold]Delegated Pool ID:[/bold] [yellow]{pool_id}[/yellow]\n"
            try:
                pool_info = wallet.api.pools(pool_id) # type: ignore
                ticker = getattr(pool_info, "ticker", "N/A")
                name = getattr(pool_info, "name", "N/A")
                content += f"  Pool Ticker: [cyan]{ticker}[/cyan]\n"
                content += f"  Pool Name:   [cyan]{name}[/cyan]\n"
            except ApiError as pool_err:
                if pool_err.status_code != 404:
                    logger.warning(
                        f"Could not fetch details for pool {pool_id}: {pool_err.message}"
                    )
                content += "  (Could not fetch pool details)\n"
            except Exception as pool_err_gen:
                logger.warning(
                    f"Error fetching details for pool {pool_id}: {pool_err_gen}"
                )
                content += "  (Error fetching pool details)\n"
        else:
            content += "[bold]Delegated Pool ID:[/bold] [dim]Not Delegated[/dim]\n"

        console.print(
            Panel(
                content,
                title=f"Staking Info for {coldkey}/{hotkey}",
                border_style="blue",
            )
        )

    except ApiError as e:
        if e.status_code == 404:
            console.print(
                f":information_source: Stake address {wallet.stake_address} not found on the blockchain or no history."
            )
        else:
            console.print(
                f":cross_mark: [bold red]API Error querying staking info:[/bold red] Status {e.status_code} - {e.message}"
            )
    except Exception as e:
        console.print(
            f":cross_mark: [bold red]Error querying staking info:[/bold red] {e}"
        )
        logger.exception("Staking info query failed")
