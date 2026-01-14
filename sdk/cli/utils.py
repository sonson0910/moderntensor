"""
Common utilities and helpers for ModernTensor CLI
"""

import sys
from typing import Optional, Any
from pathlib import Path
import click
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich import box


# Rich console instances
console = Console()
err_console = Console(stderr=True, style="bold red")


def print_error(message: str) -> None:
    """Print an error message to stderr"""
    err_console.print(f"❌ Error: {message}")


def print_success(message: str) -> None:
    """Print a success message"""
    console.print(f"✅ {message}", style="bold green")


def print_warning(message: str) -> None:
    """Print a warning message"""
    console.print(f"⚠️  {message}", style="bold yellow")


def print_info(message: str) -> None:
    """Print an info message"""
    console.print(f"ℹ️  {message}", style="bold blue")


def confirm_action(message: str, default: bool = False) -> bool:
    """
    Ask user for confirmation

    Args:
        message: Confirmation message
        default: Default value if user just presses Enter

    Returns:
        True if user confirms, False otherwise
    """
    return click.confirm(message, default=default)


def format_balance(amount: int, decimals: int = 9) -> str:
    """
    Format token amount with proper decimals

    Args:
        amount: Amount in LTS (smallest unit)
        decimals: Number of decimal places

    Returns:
        Formatted string
    """
    value = amount / (10 ** decimals)
    return f"{value:.{min(decimals, 6)}f}"


def format_address(address: str, length: int = 10) -> str:
    """
    Format address for display (shortened)

    Args:
        address: Full address
        length: Characters to show on each end

    Returns:
        Formatted address like "0x1234...5678"
    """
    if len(address) <= length * 2:
        return address
    return f"{address[:length]}...{address[-length:]}"


def create_table(title: str, columns: list[str], **kwargs) -> Table:
    """
    Create a rich Table with common styling

    Args:
        title: Table title
        columns: List of column names
        **kwargs: Additional arguments for Table

    Returns:
        Configured Table object
    """
    table = Table(
        title=title,
        box=box.ROUNDED,
        show_header=True,
        header_style="bold magenta",
        **kwargs
    )

    for col in columns:
        table.add_column(col)

    return table


def display_panel(content: str, title: str = "", style: str = "bold") -> None:
    """
    Display content in a panel

    Args:
        content: Content to display
        title: Panel title
        style: Panel style
    """
    panel = Panel(content, title=title, style=style, box=box.ROUNDED)
    console.print(panel)


def validate_network(network: str) -> str:
    """
    Validate network name

    Args:
        network: Network name (mainnet, testnet, local)

    Returns:
        Validated network name

    Raises:
        click.BadParameter: If network is invalid
    """
    valid_networks = ['mainnet', 'testnet', 'local']
    if network not in valid_networks:
        raise click.BadParameter(
            f"Invalid network '{network}'. Must be one of: {', '.join(valid_networks)}"
        )
    return network


def get_default_wallet_path() -> Path:
    """
    Get default wallet storage path

    Returns:
        Path to default wallet directory
    """
    return Path.home() / ".moderntensor" / "wallets"


def get_default_config_path() -> Path:
    """
    Get default configuration file path

    Returns:
        Path to default config file
    """
    return Path.home() / ".moderntensor" / "config.yaml"


def ensure_directory(path: Path) -> None:
    """
    Ensure directory exists, create if not

    Args:
        path: Directory path
    """
    path.mkdir(parents=True, exist_ok=True)


def prompt_password(message: str = "Enter password", confirm: bool = False) -> str:
    """
    Prompt user for password

    Args:
        message: Prompt message
        confirm: Whether to ask for confirmation

    Returns:
        Password string
    """
    return click.prompt(message, hide_input=True, confirmation_prompt=confirm)


def handle_exception(e: Exception, verbose: bool = False) -> None:
    """
    Handle and display exception

    Args:
        e: Exception to handle
        verbose: Whether to show full traceback
    """
    if verbose:
        import traceback
        err_console.print(traceback.format_exc())
    else:
        print_error(str(e))
