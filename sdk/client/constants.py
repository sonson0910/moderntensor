"""
SDK Client Constants

Centralized constants for SDK client to avoid magic numbers.
Re-exports chain-level constants from ``sdk.constants`` (single source of truth).
"""

# ── Re-exports from centralized constants ──────────────────────
from sdk.constants import (
    CHAIN_ID_MAINNET,
    CHAIN_ID_TESTNET,
    CHAIN_ID_DEVNET,
    DEFAULT_RPC_PORT,
    DEFAULT_GAS_PRICE,
    TRANSFER_GAS,
)

# Query Defaults
DEFAULT_QUERY_LIMIT = 100
DEFAULT_TIMEOUT_SECONDS = 30

# Hex Constants
HEX_ZERO = "0x0"
GENESIS_HASH = "0x0000000000000000000000000000000000000000000000000000000000000000"

# Example IP (used in docstrings/examples)
EXAMPLE_IP_ADDRESS = "192.168.1.100"

# Network Defaults
DEFAULT_NETWORK = "testnet"
DEFAULT_RPC_URL = "http://localhost:8545"

# Pagination
MAX_BATCH_SIZE = 1000

# Validation
MIN_SUBNET_ID = 0
MAX_SUBNET_ID = 2**32 - 1  # uint32 max

# WebSocket
DEFAULT_WS_URL = "ws://localhost:8546"

# ── Version Compatibility ───────────────────────────────────────
# SDK (pyproject.toml) and LuxTensor engine (Cargo.toml) use
# *independent* versioning because they have different release
# cadences.  This constant records the minimum engine version
# that this SDK is tested against.
LUXTENSOR_MIN_VERSION = "0.1.0"


