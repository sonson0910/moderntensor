import click
from sdk.service.tx_service import send_ada, send_token
from sdk.service.stake_service import Wallet
from sdk.config.settings import settings
from pycardano import Network, BlockFrostChainContext, ExtendedSigningKey
from typing import Optional

@click.group()
def tx_cli():
    """Command line interface for transaction operations."""
    pass

def validate_address(ctx, param, value):
    """Validate Cardano address format."""
    if not value:
        return None
    try:
        return str(value)
    except Exception:
        raise click.BadParameter("Invalid address format")

@tx_cli.command("send-ada")
@click.option("--coldkey", prompt="Coldkey name", help="Name of the coldkey wallet")
@click.option("--hotkey", prompt="Hotkey name", help="Name of the hotkey")
@click.option("--password", prompt=True, hide_input=True, help="Wallet password")
@click.option("--to-address", prompt="Destination address", 
              callback=validate_address, help="Recipient address")
@click.option("--amount", prompt="Amount in lovelace", type=int,
              help="Amount to send (1 ADA = 1,000,000 lovelace)")
@click.option("--network", default="testnet", 
              type=click.Choice(["testnet", "mainnet"]), help="Network to use")
def send_ada_command(coldkey, hotkey, password, to_address, amount, network):
    """Send ADA to another address with interactive prompts."""
    try:
        # Initialize wallet
        wallet = Wallet(coldkey_name=coldkey, hotkey_name=hotkey, password=password)
        
        # Setup network
        cardano_network = Network.TESTNET if network == "testnet" else Network.MAINNET
        
        # Setup chain context
        chain_context = BlockFrostChainContext(
            settings.BLOCKFROST_PROJECT_ID,
            network=cardano_network
        )
        
        # Prepare signing keys
        payment_xsk = ExtendedSigningKey.from_cbor(wallet.payment_sk.to_cbor())
        stake_xsk = (ExtendedSigningKey.from_cbor(wallet.stake_sk.to_cbor()) 
                    if wallet.stake_sk else None)
        
        # Send transaction
        tx_id = send_ada(
            chain_context=chain_context,
            payment_xsk=payment_xsk,
            stake_xsk=stake_xsk,
            to_address_str=to_address,
            lovelace_amount=amount,
            network=cardano_network
        )
        
        click.echo(click.style(f"\n✅ Transaction successful!", fg="green"))
        click.echo(f"Transaction ID: {tx_id}")
        click.echo(f"From: {wallet.main_address}")
        click.echo(f"To: {to_address}")
        click.echo(f"Amount: {amount/1000000} ADA")
        
    except Exception as e:
        click.echo(click.style(f"\n❌ Error: {str(e)}", fg="red"))
        if "decryption failed" in str(e).lower():
            click.echo("Please verify your password and try again")

if __name__ == "__main__":
    tx_cli()
