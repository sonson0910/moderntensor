import click
import logging
from .wallet_cli import wallet_cli
from .stake_cli import stake_cli
# from .metagraph_cli import metagraph_cli  # If you have

logging.basicConfig(level=logging.INFO)

@click.group()
def cli():
    """
    Welcome to the Cardano SDK CLI!
    
    CLI SDK, grouping coldkey, hotkey, staking, and metagraph commands
    
    """
    pass

# Add subcommands:
cli.add_command(wallet_cli, name="w")
cli.add_command(stake_cli, name="stake")
# cli.add_command(metagraph_cli, name="metagraph")

# Version command
@cli.command("version")
def version_cmd():
    click.echo("SDK version 0.1.0")

if __name__ == "__main__":
    cli()
