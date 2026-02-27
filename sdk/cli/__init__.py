"""
ModernTensor CLI Package

Command-line interface for interacting with the ModernTensor/Luxtensor network.

Sub-modules:
    - ``commands`` — Query, stake, subnet, tx, validator, wallet commands
    - ``config``   — Network presets and persistent configuration
    - ``ui``       — Rich terminal formatting helpers
    - ``utils``    — Common CLI utilities
    - ``wallet_utils`` — Wallet creation and management helpers
"""

from .config import CLIConfig, NetworkConfig, WalletConfig, get_network_config, NETWORKS

__version__ = "0.1.0"

__all__ = [
    "CLIConfig",
    "NetworkConfig",
    "WalletConfig",
    "get_network_config",
    "NETWORKS",
]
