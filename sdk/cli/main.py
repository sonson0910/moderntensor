# sdk/cli/main.py

import click
import logging
from .wallet_cli import wallet_cli

# from .metagraph_cli import metagraph_cli  # If you have

logging.basicConfig(level=logging.INFO)


@click.group()
def cli():
    """
    Welcome to the Cardano SDK CLI!
    
    CLI SDK, grouping coldkey, hotkey, metagraph commands, etc...
    

    """
    pass


# Thêm group con:
cli.add_command(wallet_cli, name="w")
# cli.add_command(metagraph_cli, name="metagraph")

# If you want, you can place the original command here:
@cli.command("version")
def version_cmd():
    click.echo("SDK version 0.1.0")
