"""
ModernTensor SDK - Consensus Module

Full-parity Python implementation of LuxTensor core consensus
algorithms, ported from luxtensor-consensus crate.

All implementations are thread-safe using RLock.
"""

# Core safety modules (P0)
from .slashing import (
    SlashReason,
    SlashingConfig,
    SlashingEvidence,
    SlashEvent,
    JailStatus,
    SlashingManager,
)

from .liveness import (
    LivenessAction,
    LivenessConfig,
    LivenessStats,
    LivenessMonitor,
)

from .circuit_breaker import (
    CircuitState,
    CircuitBreakerConfig,
    CircuitBreakerStats,
    CircuitBreaker,
    CircuitBreakerRegistry,
)

# Fork handling modules (P1)
from .fork_choice import (
    BlockInfo,
    ForkChoice,
    ForkChoiceError,
)

from .fast_finality import (
    ValidatorInfo as FinalityValidatorInfo,
    BlockSignatures,
    FastFinalityStats,
    FastFinality,
    FastFinalityError,
)

from .fork_resolution import (
    BlockInfo as ResolutionBlockInfo,
    ReorgInfo,
    FinalityStatus,
    FinalityStats,
    ForkResolver,
    ForkResolutionError,
)

# PoS and validator modules (P2)
from .long_range_protection import (
    Checkpoint,
    LongRangeConfig,
    CheckpointStatus,
    LongRangeProtection,
    LongRangeProtectionError,
)

from .rotation import (
    ValidatorInfo as RotationValidatorInfo,
    RotationConfig,
    PendingValidator,
    EpochTransitionResult,
    RotationStats,
    ValidatorRotation,
    ValidatorRotationError,
)

from .pos import (
    ValidatorInfo as PosValidatorInfo,
    HalvingSchedule as PosHalvingSchedule,
    ConsensusConfig,
    HalvingInfo as PosHalvingInfo,
    ProofOfStake,
    ProofOfStakeError,
)

from .halving import (
    INITIAL_BLOCK_REWARD,
    HALVING_INTERVAL,
    MINIMUM_REWARD,
    MAX_HALVINGS,
    HalvingInfo,
    HalvingSchedule,
)

__all__ = [
    # Slashing
    "SlashReason",
    "SlashingConfig",
    "SlashingEvidence",
    "SlashEvent",
    "JailStatus",
    "SlashingManager",
    # Liveness
    "LivenessAction",
    "LivenessConfig",
    "LivenessStats",
    "LivenessMonitor",
    # Circuit Breaker
    "CircuitState",
    "CircuitBreakerConfig",
    "CircuitBreakerStats",
    "CircuitBreaker",
    "CircuitBreakerRegistry",
    # Fork Choice
    "BlockInfo",
    "ForkChoice",
    "ForkChoiceError",
    # Fast Finality
    "FinalityValidatorInfo",
    "BlockSignatures",
    "FastFinalityStats",
    "FastFinality",
    "FastFinalityError",
    # Fork Resolution
    "ResolutionBlockInfo",
    "ReorgInfo",
    "FinalityStatus",
    "FinalityStats",
    "ForkResolver",
    "ForkResolutionError",
    # Long Range Protection
    "Checkpoint",
    "LongRangeConfig",
    "CheckpointStatus",
    "LongRangeProtection",
    "LongRangeProtectionError",
    # Rotation
    "RotationValidatorInfo",
    "RotationConfig",
    "PendingValidator",
    "EpochTransitionResult",
    "RotationStats",
    "ValidatorRotation",
    "ValidatorRotationError",
    # PoS
    "PosValidatorInfo",
    "PosHalvingSchedule",
    "ConsensusConfig",
    "PosHalvingInfo",
    "ProofOfStake",
    "ProofOfStakeError",
    # Halving
    "INITIAL_BLOCK_REWARD",
    "HALVING_INTERVAL",
    "MINIMUM_REWARD",
    "MAX_HALVINGS",
    "HalvingInfo",
    "HalvingSchedule",
]
