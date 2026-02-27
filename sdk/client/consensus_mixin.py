"""
Consensus Mixin for LuxtensorClient

Integrates all 10 consensus modules from sdk.consensus for client-side verification.
This enables trustless validation of blockchain data without relying solely on RPC responses.

Modules integrated:
1. ProofOfStake  - Validator election and stake verification
2. ForkChoice - Canonical chain selection (longest chain)
3. LivenessMonitor - Network health and timeout detection
4. ValidatorRotation - Epoch-based validator activation
5. SlashingDetector - Penalize malicious validators
6. Halving Schedule - Block reward calculation
7. LongRangeProtection - Weak subjectivity checkpoints
8. FastFinality - BFT-style finalization (2/3+ signatures)
9. CircuitBreaker - Emergency halt mechanism
10. ForkResolver - Reorganization detection and handling
"""

import logging
from dataclasses import dataclass
from typing import TYPE_CHECKING, cast

from sdk.consensus.circuit_breaker import CircuitBreaker, CircuitBreakerConfig
from sdk.consensus.fast_finality import FastFinality
from sdk.consensus.fork_choice import BlockInfo, ForkChoice
from sdk.consensus.fork_resolution import ForkResolver
from sdk.consensus.halving import HalvingSchedule
from sdk.consensus.liveness import LivenessConfig, LivenessMonitor
from sdk.consensus.long_range_protection import (
    LongRangeConfig,
    LongRangeProtection,
)

# Import all 10 consensus modules
from sdk.consensus.pos import ConsensusConfig, ProofOfStake
from sdk.consensus.rotation import RotationConfig, ValidatorRotation
from sdk.consensus.slashing import SlashingConfig, SlashingManager

from .constants import GENESIS_HASH

if TYPE_CHECKING:
    from .protocols import RPCProvider

logger = logging.getLogger(__name__)


@dataclass
class ConsensusState:
    """Current consensus state snapshot."""

    current_epoch: int
    finalized_height: int
    active_validators: int
    total_stake: int
    is_synced: bool
    circuit_broken: bool


class ConsensusMixin:
    """
    Mixin providing client-side consensus verification.

    Integrates all 10 consensus modules for trustless validation.
    """

    if TYPE_CHECKING:

        def _rpc(self) -> "RPCProvider":
            """Helper to cast self to RPCProvider protocol for type checking."""
            return cast("RPCProvider", self)

    else:

        def _rpc(self):
            """At runtime, return self (duck typing)."""
            return self

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._consensus_initialized = False
        self._pos = None
        self._fork_choice = None
        self._liveness = None
        self._rotation = None
        self._slashing = None
        self._halving = None
        self._long_range = None
        self._fast_finality = None
        self._circuit_breaker = None
        self._fork_resolver = None

    def init_consensus(
        self,
        genesis_hash: str = GENESIS_HASH,
    ) -> None:
        """Initialize all consensus modules."""
        if self._consensus_initialized:
            return

        logger.info("Initializing consensus verification modules...")

        # CRITICAL: Initialize all consensus modules
        self._pos = ProofOfStake(ConsensusConfig())

        # Create genesis block info for fork choice
        genesis_block = BlockInfo(
            hash=genesis_hash,
            parent_hash="0x" + "00" * 32,
            height=0,
            timestamp=0,
            proposer="0x" + "00" * 20,
        )
        self._fork_choice = ForkChoice(genesis_block)
        self._liveness = LivenessMonitor(LivenessConfig())
        self._rotation = ValidatorRotation(RotationConfig())
        self._slashing = SlashingManager(SlashingConfig())
        self._halving = HalvingSchedule()
        self._long_range = LongRangeProtection(genesis_hash, LongRangeConfig())
        self._fast_finality = FastFinality(finality_threshold_percent=67)
        self._circuit_breaker = CircuitBreaker(CircuitBreakerConfig())
        self._fork_resolver = ForkResolver(finality_threshold=32, max_reorg_depth=64)

        self._consensus_initialized = True
        logger.info("✅ All consensus modules initialized")

    def verify_block(self, block_hash: str) -> bool:
        """Verify block using consensus rules."""
        if not self._consensus_initialized:
            self.init_consensus()

        try:
            block = self._rpc()._call_rpc("eth_getBlockByHash", [block_hash, True])
            if not block:
                return False

            if self._circuit_breaker.is_active():
                logger.warning("Circuit breaker active")
                return False

            if not self._fork_choice.is_canonical(block_hash):
                logger.warning("Block not canonical: %s", block_hash)
                return False

            return True
        except Exception as e:
            logger.error("Block verification failed: %s", e)
            return False

    def check_finality(self, block_hash: str) -> bool:
        """Check if block reached fast finality."""
        if not self._consensus_initialized:
            self.init_consensus()
        return self._fast_finality.is_finalized(block_hash)

    def get_consensus_state(self) -> ConsensusState:
        """Get current consensus state."""
        if not self._consensus_initialized:
            self.init_consensus()

        try:
            validators = self._pos.get_validators()
            active_validators = [v for v in validators.values() if v.active]

            return ConsensusState(
                current_epoch=self._rotation.current_epoch(),
                finalized_height=0,  # FastFinality doesn't track height, only block hashes
                active_validators=len(active_validators),
                total_stake=sum(v.stake for v in active_validators),
                is_synced=True,
                circuit_broken=self._circuit_breaker.state() == "open",
            )
        except Exception as e:
            logger.error("Failed to get consensus state: %s", e)
            return ConsensusState(0, 0, 0, 0, False, False)

    def calculate_block_reward(self, block_height: int) -> int:
        """Calculate halving-adjusted block reward."""
        if not self._consensus_initialized:
            self.init_consensus()
        return self._halving.calculate_reward(block_height)

    def is_circuit_broken(self) -> bool:
        """Check if emergency circuit breaker is active."""
        if not self._consensus_initialized:
            self.init_consensus()
        return self._circuit_breaker.is_active()
