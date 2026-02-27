"""
LuxTensor Chain Constants & Configuration

Centralized constants module, mirror of ``luxtensor-core/src/constants.rs``.

All values in this file MUST stay in sync with the Rust source.
When updating, grep for the constant name in the Rust crate to verify.

Reference: ``luxtensor/crates/luxtensor-core/src/constants.rs``
"""

# ═══════════════════════════════════════════════════════════════
# Chain IDs
# ═══════════════════════════════════════════════════════════════

#: Canonical LuxTensor chain ID used across all crates/SDKs.
LUXTENSOR_CHAIN_ID: int = 8898

CHAIN_ID_MAINNET: int = 8898
CHAIN_ID_TESTNET: int = 9999
#: DEVNET == MAINNET (8898) — ensures tx signatures are consistent.
CHAIN_ID_DEVNET: int = 8898


# ═══════════════════════════════════════════════════════════════
# Official Addresses
# ═══════════════════════════════════════════════════════════════

#: DAO Treasury address for reward distribution (mainnet)
DAO_TREASURY_MAINNET: str = "0xDA00000000000000000000000000000000000001"
#: DAO Treasury address (testnet)
DAO_TREASURY_TESTNET: str = "0xDA00000000000000000000000000000000000002"
#: System address for protocol operations
SYSTEM_ADDRESS: str = "0x0000000000000000000000000000000000000000"
#: Burn address (tokens sent here are permanently destroyed)
BURN_ADDRESS: str = "0x000000000000000000000000000000000000dEaD"


# ═══════════════════════════════════════════════════════════════
# Precompile Contract Addresses
# ═══════════════════════════════════════════════════════════════

#: Metagraph precompile — handles neuron/validator/subnet/weights ops.
#: Address ends with ASCII "META" (0x4d455441).
METAGRAPH_PRECOMPILE: str = "0x000000000000000000000000000000004d455441"


# ═══════════════════════════════════════════════════════════════
# Block & Consensus Parameters
# ═══════════════════════════════════════════════════════════════

#: Target block time in seconds
BLOCK_TIME_SECS: int = 12
#: Epoch length in blocks
EPOCH_LENGTH: int = 100
#: Maximum validators in active set
MAX_VALIDATORS: int = 100
#: Minimum stake to become validator (1 000 000 tokens = 10^24 base units)
MIN_STAKE: int = 1_000_000_000_000_000_000_000_000
#: Block gas limit
BLOCK_GAS_LIMIT: int = 30_000_000
#: Maximum transactions per block
MAX_TXS_PER_BLOCK: int = 10_000


# ═══════════════════════════════════════════════════════════════
# Token Economics
# ═══════════════════════════════════════════════════════════════

#: Total supply (21 million tokens, 18 decimals)
TOTAL_SUPPLY: int = 21_000_000_000_000_000_000_000_000
#: Decimals (18, like ETH)
DECIMALS: int = 18
#: One full token in base units
ONE_TOKEN: int = 1_000_000_000_000_000_000
#: Initial block reward — 0.24 MDT (scaled for 12s blocks)
INITIAL_BLOCK_REWARD: int = 240_000_000_000_000_000
#: DAO share of block rewards (10% = 1000 BPS)
DAO_REWARD_SHARE_BPS: int = 1000
#: Validator share of block rewards (90% = 9000 BPS)
VALIDATOR_REWARD_SHARE_BPS: int = 9000


# ═══════════════════════════════════════════════════════════════
# Network Parameters
# ═══════════════════════════════════════════════════════════════

#: Default P2P port
DEFAULT_P2P_PORT: int = 30303
#: Default RPC port
DEFAULT_RPC_PORT: int = 8545
#: Maximum peers
MAX_PEERS: int = 50
#: Sync request rate limit (requests per peer per minute)
SYNC_RATE_LIMIT: int = 10


# ═══════════════════════════════════════════════════════════════
# Transaction Parameters
# ═══════════════════════════════════════════════════════════════

#: Default gas price
DEFAULT_GAS_PRICE: int = 50
#: Minimum gas price
MIN_GAS_PRICE: int = 1
#: Gas for simple transfer
TRANSFER_GAS: int = 21_000
#: Transaction expiration in mempool (30 minutes)
MEMPOOL_EXPIRATION_SECS: int = 1800


# ═══════════════════════════════════════════════════════════════
# Helpers (matching Rust utility functions)
# ═══════════════════════════════════════════════════════════════

def chain_name(chain_id: int) -> str:
    """Return human-readable network name for a chain ID."""
    _MAP = {
        CHAIN_ID_MAINNET: "Mainnet",
        CHAIN_ID_TESTNET: "Testnet",
    }
    return _MAP.get(chain_id, "Unknown")


def dao_treasury_for_chain(chain_id: int) -> str:
    """Return DAO treasury address for the given chain ID."""
    if chain_id == CHAIN_ID_MAINNET:
        return DAO_TREASURY_MAINNET
    return DAO_TREASURY_TESTNET


def tokens_to_base(amount: float) -> int:
    """Convert token amount to base units (18 decimals)."""
    return int(amount * ONE_TOKEN)


def base_to_tokens(base_units: int) -> float:
    """Convert base units to token amount."""
    return base_units / ONE_TOKEN
