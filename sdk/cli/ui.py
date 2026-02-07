import sys
from rich.console import Console
from rich.theme import Theme
from rich.panel import Panel
from rich.table import Table
from rich.text import Text
from rich.box import MINIMAL

# Tensor Cyberpunk Theme
theme = Theme({
    "brand": "#F23F5D",     # Neon Crimson
    "brand.bold": "bold #F23F5D",
    "success": "#00FF9D",   # Matrix Green
    "warning": "#FFD600",   # Cyber Yellow
    "error": "#FF003C",     # Reactor Red
    "info": "#00E5FF",      # Holo Blue
    "muted": "#4C4F69",     # Deep Space
    "header": "bold #F23F5D",
    "key": "bold #00E5FF",
})

console = Console(theme=theme, highlight=False)

def print_header(version: str, network: str = "mainnet"):
    """Prints the Tensor-Style ASCII Header."""
    ascii_art = r"""
 █▀▄▀█ █▀█ █▀▄ █▀▀ █▀█ █▄ █ ▀█▀ █▀▀ █▄ █ █▀▀ █▀█ █▀█
 █ ▀ █ █ █ █ █ ██▄ █▀▄ █ ▀█  █  ██▄ █ ▀█ ▄█▄ █ █ █▀▄
 ▀   ▀ ▀▀▀ ▀▀  ▀▀▀ ▀ ▀ ▀  ▀  ▀  ▀▀▀ ▀  ▀ ▀▀▀ ▀▀▀ ▀ ▀
"""
    console.print(Text(ascii_art, style="brand"))

    # Status Bar
    grid = Table.grid(expand=True)
    grid.add_column(justify="left")
    grid.add_column(justify="right")
    grid.add_row(
        Text(f" CLI v{version}", style="muted"),
        Text(f"Network: {network} ● ", style="success")
    )
    console.print(grid)
    console.print(Text("─" * console.width, style="muted"))
    console.print()

def print_success(message: str):
    """Prints a success message."""
    console.print(f"[success]✅ {message}[/success]")

def print_error(message: str, exit_code: int = 0):
    """Prints an error message. Does not exit by default."""
    console.print(f"[error]❌ {message}[/error]")

def print_info(message: str):
    """Prints an info message."""
    console.print(f"[info]ℹ {message}[/info]")

def print_kv(key: str, value: str):
    """Prints a Key-Value pair aligned."""
    console.print(f" [key]{key:<15}[/key] {value}")

def print_table(columns: list, rows: list, title: str = None):
    """Prints a standardized table with data."""
    table = create_table(title, columns)
    for row in rows:
        table.add_row(*row)
    console.print(table)

def create_table(title: str = None, columns: list = None) -> Table:
    """Creates a standardized table."""
    table = Table(box=MINIMAL, header_style="brand.bold", border_style="muted", show_lines=False)
    if title:
        table.title = f"[brand]{title}[/brand]"

    if columns:
        for col in columns:
            table.add_column(col)

    return table

def print_panel(message: str, title: str = "Alert", style: str = "brand", border_style: str = None):
    """Prints a prominent panel."""
    bs = border_style or style
    console.print(Panel(message, title=f"[{style}]{title}[/{style}]", border_style=bs))

def spinner(message: str):
    """Returns a spinner context manager."""
    return console.status(f"[bold cyan]{message}...", spinner="dots")

def print_warning(message: str):
    """Prints a warning message."""
    console.print(f"[warning]⚠ {message}[/warning]")

def confirm_action(message: str) -> bool:
    """Prompts for user confirmation."""
    from rich.prompt import Confirm
    return Confirm.ask(f"[bold brand]{message}[/]")
