# sdk/cli/main.py

import click
import logging
import importlib.metadata
from rich.console import Console
from rich.panel import Panel
from rich.text import Text
from rich import box

from .wallet_cli import wallet_cli
from .tx_cli import tx_cli
from .query_cli import query_cli
from .stake_cli import stake_cli
from .l1_stake_cli import l1_stake_cli  # Import Layer 1 staking CLI
from .subnet_cli import subnet_cli  # Import Subnet CLI
from .validator_cli import run_validator  # Import Validator Runner
from .miner_cli import run_miner  # Import Miner Runner
from .simulation_cli import simulate_subnet  # Import Simulator

# from .metagraph_cli import metagraph_cli  # If you have

logging.basicConfig(level=logging.INFO)

# ASCII Art for ModernTensor
ASCII_ART = r"""
███╗   ███╗ ██████╗ ██████╗ ███████╗██████╗ ███╗   ██╗████████╗███████╗███╗   ██╗███████╗ ██████╗ ██████╗
████╗ ████║██╔═══██╗██╔══██╗██╔════╝██╔══██╗████╗  ██║╚══██╔══╝██╔════╝████╗  ██║██╔════╝██╔═══██╗██╔══██╗
██╔████╔██║██║   ██║██║  ██║█████╗  ██████╔╝██╔██╗ ██║   ██║   █████╗  ██╔██╗ ██║███████╗██║   ██║██████╔╝
██║╚██╔╝██║██║   ██║██║  ██║██╔══╝  ██╔══██╗██║╚██╗██║   ██║   ██╔══╝  ██║╚██╗██║╚════██║██║   ██║██╔══██╗
██║ ╚═╝ ██║╚██████╔╝██████╔╝███████╗██║  ██║██║ ╚████║   ██║   ███████╗██║ ╚████║███████║╚██████╔╝██║  ██║
╚═╝     ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝   ╚══════╝╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝


"""

# Colorful scheme v2
PROJECT_DESCRIPTION = """[bright_yellow]⭐ Moderntensor is a decentralized model training project built on the Cardano blockchain platform.
The project is developed by Vietnamese 🇻🇳  engineers from the Moderntensor Foundation.[/bright_yellow]"""
REPO_URL = "https://github.com/sonson0910/moderntensor.git"  # Replace
DOCS_URL = "https://github.com/sonson0910/moderntensor/blob/development_consensus/docs/WhitePaper.pdf"  # Replace
CHAT_URL = "https://t.me/+pDRlNXTi1wY2NTY1"  # Replace
CONTRIBUTE_URL = f"https://github.com/sonson0910/moderntensor/blob/main/docs/README.md"  # Adjust if needed


@click.group(invoke_without_command=True)
@click.pass_context  # Need context to check for subcommands
def cli(ctx):
    """ModernTensor CLI - Manage wallets, transactions, and subnets."""

    # Display splash screen only if no subcommand is invoked
    if ctx.invoked_subcommand is None:
        console = Console()

        try:
            version = importlib.metadata.version(
                "moderntensor"
            )  # Replace 'moderntensor' if package name is different
        except importlib.metadata.PackageNotFoundError:
            version = "[yellow]unknown[/yellow]"

        info_text = Text.assemble(
            ("🐙 repo:       ", "bold blue"),
            (REPO_URL, "link " + REPO_URL),
            "\n",
            ("📚 docs:       ", "bold green"),
            (DOCS_URL, "link " + DOCS_URL),
            "\n",
            ("💬 chat:       ", "bold magenta"),
            (CHAT_URL, "link " + CHAT_URL),
            "\n",
            ("✨ contribute: ", "bold yellow"),
            (CONTRIBUTE_URL, "link " + CONTRIBUTE_URL),
            "\n",
            ("📦 version:    ", "bold cyan"),
            (version, "yellow"),
        )

        console.print(
            f"[bold bright_white]{ASCII_ART}[/bold bright_white]", justify="center"
        )
        console.print(PROJECT_DESCRIPTION, justify="center")
        console.print(" ")  # Spacer
        console.print(
            Panel(
                info_text,
                title="[bold bright_yellow on bright_red] Project Links [/]",
                border_style="bright_yellow",
                box=box.HEAVY,
                padding=(1, 2),
            )
        )
        console.print(" ")  # Spacer
        ctx.exit()  # Exit after showing splash screen


# Thêm group con:
cli.add_command(wallet_cli, name="w")
cli.add_command(tx_cli, name="tx")
cli.add_command(query_cli, name="query")
cli.add_command(stake_cli, name="stake")
cli.add_command(l1_stake_cli, name="l1-stake")  # Layer 1 staking commands
cli.add_command(subnet_cli, name="subnet")  # Register subnet command
cli.add_command(run_validator, name="run_validator")  # Register validator runner
cli.add_command(run_miner, name="run_miner")  # Register miner runner
cli.add_command(simulate_subnet, name="simulate")  # Register simulator
# cli.add_command(metagraph_cli, name="metagraph")

# If you want, you can place the original command here:
# Remove the old version command if displaying version in splash screen
# @cli.command("version")
# def version_cmd():
#     click.echo("SDK version 0.1.0")
