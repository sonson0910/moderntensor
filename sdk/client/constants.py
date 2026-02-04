"""
SDK Client Constants

Centralized constants for SDK client to avoid magic numbers.
"""

# Query Defaults
DEFAULT_QUERY_LIMIT = 100
DEFAULT_TIMEOUT_SECONDS = 30
DEFAULT_RPC_PORT = 8545

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
