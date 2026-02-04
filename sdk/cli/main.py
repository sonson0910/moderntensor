#!/usr/bin/env python3
"""
ModernTensor CLI - Main Entry Point

Command-line interface for interacting with the Luxtensor blockchain.
Inspired by Bittensor's btcli but adapted for ModernTensor's architecture.
"""

import sys
import click
from typing import Optional

from sdk.cli import __version__
from sdk.cli.commands import wallet, stake, query, tx, subnet, validator, utils


from sdk.cli import ui

@click.group(invoke_without_command=True)
@click.option('--version', '-v', is_flag=True, help='Show version information')
@click.option('--config', '-c', type=click.Path(), help='Path to configuration file')
@click.pass_context
def cli(ctx, version: bool, config: Optional[str]):
    """
    ModernTensor CLI (mtcli)

    Command-line interface for the ModernTensor/Luxtensor network.
    """
    # Initialize UI - Always print header on startup (consistent with Rust CLI)
    ui.print_header(version=__version__)

    # Ensure context object exists
    ctx.ensure_object(dict)

    if version:
        sys.exit(0)

    # Load configuration if provided
    if config:
        ctx.obj['config_path'] = config

    # Show help if no command provided
    if ctx.invoked_subcommand is None:
        click.echo(ctx.get_help())


# Register command groups
cli.add_command(wallet.wallet)
cli.add_command(stake.stake)
cli.add_command(query.query)
cli.add_command(tx.tx)
cli.add_command(subnet.subnet)
cli.add_command(validator.validator)
cli.add_command(utils.utils)


def main():
    """Main entry point for the CLI"""
    try:
        cli(obj={})
    except KeyboardInterrupt:
        ui.print_error("Interrupted by user", exit_code=130)
    except Exception as e:
        ui.print_error(f"Error: {str(e)}", exit_code=1)


if __name__ == '__main__':
    main()
