import click
import importlib
import asyncio
from sdk.agent.miner_agent import MinerAgent
from sdk.config.settings import settings
from sdk.keymanager.wallet_manager import WalletManager
from sdk.compat.pycardano import Network


@click.command(name="run_miner")
@click.option(
    "--subnet",
    required=True,
    help="Path to the subnet module (e.g., sdk.subnets.text_gen.TextGenerationSubnet).",
)
@click.option("--coldkey", required=True, help="Name of the coldkey.")
@click.option("--uid", required=True, help="Miner UID (hex string).")
@click.option("--network", default="testnet", help="Network (mainnet/testnet).")
@click.option("--port", default=8001, help="Port to run the miner API server.")
@click.option(
    "--validator-url", default="http://localhost:8000", help="URL of the validator API."
)
def run_miner(subnet, coldkey, uid, network, port, validator_url):
    """Run a Miner Agent for a specific Subnet."""

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

    # 2. Setup Wallet
    wm = WalletManager(base_dir=settings.HOTKEY_BASE_DIR)
    password = click.prompt("Enter coldkey password", hide_input=True)
    wallet_info = wm.load_coldkey(coldkey, password)

    if not wallet_info:
        click.echo("Failed to load coldkey.")
        return

    signing_key = wallet_info["payment_xsk"]

    # 3. Initialize Miner Agent
    agent = MinerAgent(
        miner_uid_hex=uid,
        config=settings,
        miner_skey=signing_key,
        subnet_protocol=subnet_protocol,
    )

    # 4. Run Agent
    click.echo(f"Starting Miner Agent on port {port}...")
    asyncio.run(agent.run(validator_api_url=validator_url, port=port))
