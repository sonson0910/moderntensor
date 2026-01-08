import click
import os
from sdk.config.settings import settings
from sdk.service.subnet_manager import SubnetManager
from sdk.keymanager.wallet_manager import WalletManager
from sdk.compat.luxtensor_types import BlockFrostChainContext, Network


@click.group(name="subnet")
def subnet_cli():
    """Manage Dynamic Subnets."""
    pass


@subnet_cli.command(name="create")
@click.option("--name", required=True, help="Name of the new subnet.")
@click.option("--coldkey", required=True, help="Name of the coldkey to use as owner.")
@click.option("--stake", default=100, help="Initial stake in ADA (default: 100).")
@click.option(
    "--network", default="testnet", help="Network to deploy to (mainnet/testnet)."
)
def create_subnet(name, coldkey, stake, network):
    """Create a new dynamic subnet on the network."""

    # Setup Context
    net_env = Network.MAINNET if network == "mainnet" else Network.TESTNET
    context = BlockFrostChainContext(
        project_id=settings.BLOCKFROST_PROJECT_ID, network=net_env
    )

    # Load Wallet
    wm = WalletManager(base_dir=settings.HOTKEY_BASE_DIR)
    password = click.prompt("Enter coldkey password", hide_input=True)
    wallet_info = wm.load_coldkey(coldkey, password)

    if not wallet_info:
        click.echo("Failed to load coldkey.")
        return

    signing_key = wallet_info["payment_xsk"]  # Assuming this key is returned

    # Create Subnet
    manager = SubnetManager(context, net_env)
    click.echo(f"Creating subnet '{name}' with {stake} ADA stake...")

    tx_id = manager.create_subnet(
        signing_key=signing_key,
        subnet_name=name,
        subnet_metadata={"description": f"Subnet {name}"},
        initial_stake=stake * 1_000_000,  # Convert to Lovelace
    )

    if tx_id:
        click.echo(f"Success! Subnet creation transaction: {tx_id}")
    else:
        click.echo("Failed to create subnet.")


if __name__ == "__main__":
    subnet_cli()
