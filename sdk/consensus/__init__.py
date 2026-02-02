"""
ModernTensor SDK - Consensus Module

Consensus-related types and managers matching luxtensor-consensus crate.

Modules:
- slashing: Validator penalty system
- circuit_breaker: Fault tolerance for AI/ML operations
- liveness: Network health monitoring
- fork_choice: GHOST fork selection algorithm
- fast_finality: BFT-style fast finality
"""

from .slashing import (
    SlashReason,
    SlashingConfig,
    SlashingEvidence,
    SlashEvent,
    JailStatus,
    SlashingManager,
)

from .circuit_breaker import (
    CircuitState,
    CircuitBreakerConfig,
    CircuitBreakerStats,
    CircuitBreakerError,
    CircuitOpenError,
    OperationTimeoutError,
    CircuitBreaker,
    CircuitBreakerRegistry,
    get_circuit_breaker,
)

from .liveness import (
    LivenessAction,
    LivenessConfig,
    LivenessStats,
    LivenessMonitor,
)

from .fork_choice import (
    BlockInfo,
    ForkChoice,
    ForkChoiceError,
)

from .fast_finality import (
    ValidatorInfo,
    BlockSignatures,
    FastFinalityStats,
    FastFinality,
    FastFinalityError,
)

__all__ = [
    # Slashing
    "SlashReason",
    "SlashingConfig",
    "SlashingEvidence",
    "SlashEvent",
    "JailStatus",
    "SlashingManager",
    # Circuit Breaker
    "CircuitState",
    "CircuitBreakerConfig",
    "CircuitBreakerStats",
    "CircuitBreakerError",
    "CircuitOpenError",
    "OperationTimeoutError",
    "CircuitBreaker",
    "CircuitBreakerRegistry",
    "get_circuit_breaker",
    # Liveness
    "LivenessAction",
    "LivenessConfig",
    "LivenessStats",
    "LivenessMonitor",
    # Fork Choice
    "BlockInfo",
    "ForkChoice",
    "ForkChoiceError",
    # Fast Finality
    "ValidatorInfo",
    "BlockSignatures",
    "FastFinalityStats",
    "FastFinality",
    "FastFinalityError",
]

