import click
import importlib
import sys
import os
from sdk.consensus.node import ValidatorNode
from sdk.config.settings import settings
from sdk.keymanager.wallet_manager import WalletManager
from pycardano import BlockFrostChainContext, Network, ExtendedSigningKey


@click.command(name="run_validator")
@click.option(
    "--subnet",
    required=True,
    help="Path to the subnet module (e.g., sdk.subnets.text_gen.TextGenerationSubnet).",
)
@click.option("--coldkey", required=True, help="Name of the coldkey.")
@click.option("--network", default="testnet", help="Network (mainnet/testnet).")
def run_validator(subnet, coldkey, network):
    """Run a Validator Node for a specific Subnet."""

    # 1. Load Subnet Protocol
    try:
        module_path, class_name = subnet.rsplit(".", 1)
        module = importlib.import_module(module_path)
        SubnetClass = getattr(module, class_name)
        subnet_protocol = SubnetClass()
        click.echo(f"Loaded Subnet Protocol: {subnet_protocol.get_metadata()['name']}")
    except Exception as e:
        click.echo(f"Error loading subnet protocol: {e}")
        return

    # 2. Setup Context & Wallet
    net_env = Network.MAINNET if network == "mainnet" else Network.TESTNET
    context = BlockFrostChainContext(
        project_id=settings.BLOCKFROST_PROJECT_ID, network=net_env
    )

    wm = WalletManager(base_dir=settings.HOTKEY_BASE_DIR)
    password = click.prompt("Enter coldkey password", hide_input=True)
    wallet_info = wm.load_coldkey(coldkey, password)

    if not wallet_info:
        click.echo("Failed to load coldkey.")
        return

    signing_key = wallet_info["payment_xsk"]

    # 3. Initialize Validator Node
    # Mock ValidatorInfo for now (in reality, fetch from chain/metagraph)
    from sdk.core.datatypes import ValidatorInfo

    v_info = ValidatorInfo(
        uid=wallet_info["payment_vkey"].hash().to_primitive().hex(),
        address=str(wallet_info["payment_addr"]),
        api_endpoint="http://localhost:8000",
    )

    node = ValidatorNode(
        validator_info=v_info,
        cardano_context=context,
        signing_key=signing_key,
        subnet_protocol=subnet_protocol,  # Inject Protocol
    )

    # 4. Run Node
    import asyncio

    click.echo("Starting Validator Node...")
    asyncio.run(node.run())
