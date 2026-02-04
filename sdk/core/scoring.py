"""
Scoring Types matching luxtensor-consensus/src/scoring.rs

Provides performance tracking types for miners and validators.
"""

from dataclasses import dataclass
from typing import Dict, Optional
from enum import Enum
import time


@dataclass
class MinerMetrics:
    """
    Miner performance metrics.

    Matches luxtensor-consensus MinerMetrics.
    """
    address: str
    tasks_completed: int = 0
    tasks_failed: int = 0
    total_execution_time: int = 0  # ms
    average_quality_score: float = 0.0
    performance_score: int = 0  # 0-100_000 (0-100%)
    last_active: int = 0  # timestamp

    @property
    def completion_rate(self) -> float:
        """Calculate task completion rate."""
        total = self.tasks_completed + self.tasks_failed
        if total == 0:
            return 0.0
        return self.tasks_completed / total

    @property
    def average_execution_time(self) -> float:
        """Calculate average execution time in ms."""
        if self.tasks_completed == 0:
            return 0.0
        return self.total_execution_time / self.tasks_completed

    def to_dict(self) -> Dict:
        return {
            "address": self.address,
            "tasks_completed": self.tasks_completed,
            "tasks_failed": self.tasks_failed,
            "completion_rate": self.completion_rate,
            "average_execution_time_ms": self.average_execution_time,
            "average_quality_score": self.average_quality_score,
            "performance_score": self.performance_score,
            "performance_score_pct": self.performance_score / 1000,  # Convert to %
            "last_active": self.last_active,
        }


@dataclass
class ValidatorMetrics:
    """
    Validator performance metrics.

    Matches luxtensor-consensus ValidatorMetrics.
    """
    address: str
    blocks_produced: int = 0
    blocks_missed: int = 0
    attestations_made: int = 0
    total_attestation_delay: int = 0  # ms
    slashing_events: int = 0
    performance_score: int = 0  # 0-100_000 (0-100%)
    last_active: int = 0  # timestamp

    @property
    def block_production_rate(self) -> float:
        """Calculate block production rate."""
        total = self.blocks_produced + self.blocks_missed
        if total == 0:
            return 0.0
        return self.blocks_produced / total

    @property
    def average_attestation_delay(self) -> float:
        """Calculate average attestation delay in ms."""
        if self.attestations_made == 0:
            return 0.0
        return self.total_attestation_delay / self.attestations_made

    def to_dict(self) -> Dict:
        return {
            "address": self.address,
            "blocks_produced": self.blocks_produced,
            "blocks_missed": self.blocks_missed,
            "block_production_rate": self.block_production_rate,
            "attestations_made": self.attestations_made,
            "average_attestation_delay_ms": self.average_attestation_delay,
            "slashing_events": self.slashing_events,
            "performance_score": self.performance_score,
            "performance_score_pct": self.performance_score / 1000,  # Convert to %
            "last_active": self.last_active,
        }


class ScoringEventType(str, Enum):
    """Scoring event types."""
    TASK_COMPLETED = "task_completed"
    TASK_FAILED = "task_failed"
    BLOCK_PRODUCED = "block_produced"
    BLOCK_MISSED = "block_missed"
    ATTESTATION_MADE = "attestation_made"


@dataclass
class ScoringEvent:
    """
    Scoring event for metrics updates.

    Matches luxtensor-consensus ScoringEvent enum.
    """
    event_type: ScoringEventType
    address: str
    execution_time: int = 0  # ms
    quality_score: int = 0   # 0-100_000
    delay: int = 0           # ms

    @classmethod
    def task_completed(cls, miner: str, execution_time: int, quality_score: int) -> 'ScoringEvent':
        return cls(
            event_type=ScoringEventType.TASK_COMPLETED,
            address=miner,
            execution_time=execution_time,
            quality_score=quality_score,
        )

    @classmethod
    def task_failed(cls, miner: str) -> 'ScoringEvent':
        return cls(
            event_type=ScoringEventType.TASK_FAILED,
            address=miner,
        )

    @classmethod
    def block_produced(cls, validator: str) -> 'ScoringEvent':
        return cls(
            event_type=ScoringEventType.BLOCK_PRODUCED,
            address=validator,
        )

    @classmethod
    def block_missed(cls, validator: str) -> 'ScoringEvent':
        return cls(
            event_type=ScoringEventType.BLOCK_MISSED,
            address=validator,
        )

    @classmethod
    def attestation_made(cls, validator: str, delay: int) -> 'ScoringEvent':
        return cls(
            event_type=ScoringEventType.ATTESTATION_MADE,
            address=validator,
            delay=delay,
        )


@dataclass
class ScoringConfig:
    """
    Scoring configuration.

    Matches luxtensor-consensus ScoringConfig.
    """
    # Score weights (must sum to 1.0)
    completion_weight: float = 0.4      # Weight for task completion rate
    latency_weight: float = 0.3         # Weight for execution time
    quality_weight: float = 0.3         # Weight for quality score

    # Validator score weights
    block_production_weight: float = 0.5  # Weight for block production
    attestation_weight: float = 0.3       # Weight for attestations
    uptime_weight: float = 0.2            # Weight for uptime

    # Decay parameters
    score_decay_rate: float = 0.99      # Score decay per epoch
    min_tasks_for_score: int = 10       # Minimum tasks before scoring

    # Thresholds
    latency_target_ms: int = 1000       # Target latency for max score
    latency_penalty_ms: int = 5000      # Latency at which score is 0

    @classmethod
    def default(cls) -> 'ScoringConfig':
        return cls()


class ScoringManager:
    """
    Scoring Manager - tracks and calculates performance scores.

    Matches luxtensor-consensus ScoringManager.
    """

    def __init__(self, config: Optional[ScoringConfig] = None):
        self.config = config or ScoringConfig.default()
        self._miner_metrics: Dict[str, MinerMetrics] = {}
        self._validator_metrics: Dict[str, ValidatorMetrics] = {}

    def process_event(self, event: ScoringEvent):
        """Process a scoring event."""
        if event.event_type == ScoringEventType.TASK_COMPLETED:
            self.record_task_completed(
                event.address,
                event.execution_time,
                event.quality_score,
            )
        elif event.event_type == ScoringEventType.TASK_FAILED:
            self.record_task_failed(event.address)
        elif event.event_type == ScoringEventType.BLOCK_PRODUCED:
            self.record_block_produced(event.address)
        elif event.event_type == ScoringEventType.BLOCK_MISSED:
            self.record_block_missed(event.address)
        elif event.event_type == ScoringEventType.ATTESTATION_MADE:
            self.record_attestation(event.address, event.delay)

    def record_task_completed(self, miner: str, execution_time: int, quality_score: int):
        """Record successful task completion."""
        miner = miner.lower()
        if miner not in self._miner_metrics:
            self._miner_metrics[miner] = MinerMetrics(address=miner)

        m = self._miner_metrics[miner]
        m.tasks_completed += 1
        m.total_execution_time += execution_time

        # Update running average quality score
        total_tasks = m.tasks_completed
        m.average_quality_score = (
            (m.average_quality_score * (total_tasks - 1) + quality_score / 1000)
            / total_tasks
        )
        m.last_active = int(time.time())

        self._recalculate_miner_score(miner)

    def record_task_failed(self, miner: str):
        """Record failed task."""
        miner = miner.lower()
        if miner not in self._miner_metrics:
            self._miner_metrics[miner] = MinerMetrics(address=miner)

        self._miner_metrics[miner].tasks_failed += 1
        self._miner_metrics[miner].last_active = int(time.time())
        self._recalculate_miner_score(miner)

    def record_block_produced(self, validator: str):
        """Record block production."""
        validator = validator.lower()
        if validator not in self._validator_metrics:
            self._validator_metrics[validator] = ValidatorMetrics(address=validator)

        self._validator_metrics[validator].blocks_produced += 1
        self._validator_metrics[validator].last_active = int(time.time())
        self._recalculate_validator_score(validator)

    def record_block_missed(self, validator: str):
        """Record missed block."""
        validator = validator.lower()
        if validator not in self._validator_metrics:
            self._validator_metrics[validator] = ValidatorMetrics(address=validator)

        self._validator_metrics[validator].blocks_missed += 1
        self._recalculate_validator_score(validator)

    def record_attestation(self, validator: str, delay: int):
        """Record attestation."""
        validator = validator.lower()
        if validator not in self._validator_metrics:
            self._validator_metrics[validator] = ValidatorMetrics(address=validator)

        self._validator_metrics[validator].attestations_made += 1
        self._validator_metrics[validator].total_attestation_delay += delay
        self._validator_metrics[validator].last_active = int(time.time())
        self._recalculate_validator_score(validator)

    def _recalculate_miner_score(self, miner: str):
        """Recalculate miner score."""
        m = self._miner_metrics.get(miner)
        if not m:
            return

        total_tasks = m.tasks_completed + m.tasks_failed
        if total_tasks < self.config.min_tasks_for_score:
            m.performance_score = 0
            return

        # Completion score (0-1)
        completion_score = m.completion_rate

        # Latency score (0-1)
        avg_latency = m.average_execution_time
        if avg_latency <= self.config.latency_target_ms:
            latency_score = 1.0
        elif avg_latency >= self.config.latency_penalty_ms:
            latency_score = 0.0
        else:
            latency_score = 1.0 - (avg_latency - self.config.latency_target_ms) / (
                self.config.latency_penalty_ms - self.config.latency_target_ms
            )

        # Quality score (already 0-1)
        quality_score = m.average_quality_score / 100

        # Weighted average
        final_score = (
            completion_score * self.config.completion_weight +
            latency_score * self.config.latency_weight +
            quality_score * self.config.quality_weight
        )

        m.performance_score = int(final_score * 100_000)

    def _recalculate_validator_score(self, validator: str):
        """Recalculate validator score."""
        v = self._validator_metrics.get(validator)
        if not v:
            return

        # Block production score
        block_score = v.block_production_rate

        # Attestation score (based on delay, lower is better)
        if v.attestations_made > 0:
            avg_delay = v.average_attestation_delay
            attestation_score = max(0.0, 1.0 - (avg_delay / 10000))  # 10s = 0 score
        else:
            attestation_score = 0.0

        # Uptime score (no slashing = 1.0)
        uptime_score = max(0.0, 1.0 - (v.slashing_events * 0.1))

        # Weighted average
        final_score = (
            block_score * self.config.block_production_weight +
            attestation_score * self.config.attestation_weight +
            uptime_score * self.config.uptime_weight
        )

        v.performance_score = int(final_score * 100_000)

    def get_miner_score(self, miner: str) -> int:
        """Get miner score (0-100_000)."""
        m = self._miner_metrics.get(miner.lower())
        return m.performance_score if m else 0

    def get_validator_score(self, validator: str) -> int:
        """Get validator score (0-100_000)."""
        v = self._validator_metrics.get(validator.lower())
        return v.performance_score if v else 0

    def get_all_miner_scores(self) -> Dict[str, int]:
        """Get all miner scores."""
        return {addr: m.performance_score for addr, m in self._miner_metrics.items()}

    def get_all_validator_scores(self) -> Dict[str, int]:
        """Get all validator scores."""
        return {addr: v.performance_score for addr, v in self._validator_metrics.items()}

    def get_miner_metrics(self, miner: str) -> Optional[MinerMetrics]:
        """Get miner metrics."""
        return self._miner_metrics.get(miner.lower())

    def get_validator_metrics(self, validator: str) -> Optional[ValidatorMetrics]:
        """Get validator metrics."""
        return self._validator_metrics.get(validator.lower())

    def apply_decay(self):
        """Apply decay to all scores."""
        for m in self._miner_metrics.values():
            m.performance_score = int(m.performance_score * self.config.score_decay_rate)
        for v in self._validator_metrics.values():
            v.performance_score = int(v.performance_score * self.config.score_decay_rate)

    @property
    def miner_count(self) -> int:
        """Total miner count."""
        return len(self._miner_metrics)

    @property
    def validator_count(self) -> int:
        """Total validator count."""
        return len(self._validator_metrics)


# Module exports
__all__ = [
    "MinerMetrics",
    "ValidatorMetrics",
    "ScoringEvent",
    "ScoringEventType",
    "ScoringConfig",
    "ScoringManager",
]
