"""
ModernTensor SDK

Python SDK for interacting with Luxtensor blockchain and building AI/ML subnets.
"""

# Luxtensor client (blockchain interaction)
from .luxtensor_client import (
    LuxtensorClient,
    AsyncLuxtensorClient,
    connect,
    async_connect,
    ChainInfo,
    Account,
    TransactionResult,
)

# Transactions
from .transactions import (
    LuxtensorTransaction,
    create_transfer_transaction,
    sign_transaction,
    verify_transaction_signature,
    encode_transaction_for_rpc,
)

# WebSocket (real-time events)
from .websocket_client import (
    LuxtensorWebSocket,
    SubscriptionType,
    BlockEvent,
    TransactionEvent,
    AccountChangeEvent,
    subscribe_to_blocks,
)

# Caching (moved to core/)
from .core.cache import (
    LuxtensorCache,
    MemoryCache,
    RedisCache,
    CacheBackend,
    cached,
    get_cache,
    set_cache,
)

# Indexer Client
from .indexer_client import (
    IndexerClient,
    AsyncIndexerClient,
    IndexedBlock,
    IndexedTransaction,
    TokenTransfer,
    StakeEvent,
    SyncStatus,
)

# Neuron Checker (registration & activity)
from .neuron_checker import (
    NeuronChecker,
    NeuronRegistrationInfo,
    NeuronStatus,
    NeuronType,
)

# Utilities (unit conversion, validation)
from .utils import (
    to_mdt,
    from_mdt,
    format_mdt,
    validate_address,
    shorten_address,
    shorten_hash,
    MDT_DECIMALS,
    MDT_WEI_MULTIPLIER,
)

# Errors (structured RPC errors)
from .errors import (
    RpcError,
    RpcErrorCode,
    BlockNotFoundError,
    TransactionNotFoundError,
    InsufficientFundsError,
    InvalidSignatureError,
    NonceTooLowError,
    GasLimitExceededError,
    RateLimitedError,
    MempoolFullError,
    parse_rpc_error,
    check_rpc_response,
)

# Consensus module (slashing, circuit breaker, liveness, fork choice, fast finality)
from .consensus import (
    # Slashing
    SlashReason,
    SlashingConfig,
    SlashingEvidence,
    SlashEvent,
    JailStatus,
    SlashingManager,
    # Circuit Breaker
    CircuitState,
    CircuitBreakerConfig,
    CircuitBreaker,
    CircuitOpenError,
    get_circuit_breaker,
    # Liveness
    LivenessAction,
    LivenessConfig,
    LivenessMonitor,
    # Fork Choice
    BlockInfo,
    ForkChoice,
    ForkChoiceError,
    # Fast Finality
    ValidatorInfo,
    BlockSignatures,
    FastFinalityStats,
    FastFinality,
    FastFinalityError,
)


# Version
from .version import __version__


__all__ = [
    # Luxtensor client
    "LuxtensorClient",
    "AsyncLuxtensorClient",
    "connect",
    "async_connect",
    "ChainInfo",
    "Account",
    "TransactionResult",
    # Transactions
    "LuxtensorTransaction",
    "create_transfer_transaction",
    "sign_transaction",
    "verify_transaction_signature",
    "encode_transaction_for_rpc",
    # WebSocket
    "LuxtensorWebSocket",
    "SubscriptionType",
    "BlockEvent",
    "TransactionEvent",
    "AccountChangeEvent",
    "subscribe_to_blocks",
    # Caching
    "LuxtensorCache",
    "MemoryCache",
    "RedisCache",
    "CacheBackend",
    "cached",
    "get_cache",
    "set_cache",
    # Indexer
    "IndexerClient",
    "AsyncIndexerClient",
    "IndexedBlock",
    "IndexedTransaction",
    "TokenTransfer",
    "StakeEvent",
    "SyncStatus",
    # Neuron Checker
    "NeuronChecker",
    "NeuronRegistrationInfo",
    "NeuronStatus",
    "NeuronType",
    # Utilities
    "to_mdt",
    "from_mdt",
    "format_mdt",
    "validate_address",
    "shorten_address",
    "shorten_hash",
    "MDT_DECIMALS",
    "MDT_WEI_MULTIPLIER",
    # Errors
    "RpcError",
    "RpcErrorCode",
    "BlockNotFoundError",
    "TransactionNotFoundError",
    "InsufficientFundsError",
    "InvalidSignatureError",
    "NonceTooLowError",
    "GasLimitExceededError",
    "RateLimitedError",
    "MempoolFullError",
    "parse_rpc_error",
    "check_rpc_response",
    # Consensus - Slashing
    "SlashReason",
    "SlashingConfig",
    "SlashingEvidence",
    "SlashEvent",
    "JailStatus",
    "SlashingManager",
    # Consensus - Circuit Breaker
    "CircuitState",
    "CircuitBreakerConfig",
    "CircuitBreaker",
    "CircuitOpenError",
    "get_circuit_breaker",
    # Consensus - Liveness
    "LivenessAction",
    "LivenessConfig",
    "LivenessMonitor",
    # Consensus - Fork Choice
    "BlockInfo",
    "ForkChoice",
    "ForkChoiceError",
    # Consensus - Fast Finality
    "ValidatorInfo",
    "BlockSignatures",
    "FastFinalityStats",
    "FastFinality",
    "FastFinalityError",
    # Version
    "__version__",
]

