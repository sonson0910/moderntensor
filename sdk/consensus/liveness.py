"""
Liveness Monitor for Network Health.

Matches luxtensor-consensus/src/liveness.rs.

Monitors network liveness and block production to detect and recover from stalls.
Helps prevent network hangs by:
- Tracking last block production time
- Detecting stalled validators
- Triggering recovery actions when network is stuck
"""

from dataclasses import dataclass
from enum import Enum
from typing import Optional
import time
import threading


class LivenessAction(str, Enum):
    """Actions that can be taken when liveness issues are detected."""
    HEALTHY = "healthy"               # Network is healthy, no action needed
    WAIT_MORE = "wait_more"           # Waiting for block, but not yet critical
    WARN_SLOW = "warn_slow"           # Blocks coming slower than expected
    DISCOVER_PEERS = "discover_peers"  # Low peer count, find more peers
    REQUEST_SYNC = "request_sync"      # Request sync from other nodes
    NETWORK_STALLED = "network_stalled"  # Critical: Network appears to be stalled


@dataclass
class LivenessConfig:
    """
    Configuration for liveness monitoring.

    Matches luxtensor-consensus LivenessConfig.
    """
    # Timing thresholds (in seconds)
    block_timeout_secs: float = 30.0        # Expected max time between blocks
    warning_timeout_secs: float = 60.0      # Time before warning
    stall_timeout_secs: float = 120.0       # Time before declaring stall

    # Peer thresholds
    min_peers: int = 3                       # Minimum healthy peer count
    low_peer_threshold: int = 5              # Low peer count threshold

    # Missed block tracking
    max_missed_before_action: int = 5        # Missed blocks before action

    @classmethod
    def default(cls) -> 'LivenessConfig':
        return cls()


@dataclass
class LivenessStats:
    """
    Statistics about liveness monitoring.

    Matches luxtensor-consensus LivenessStats.
    """
    blocks_received: int = 0
    missed_blocks: int = 0
    current_height: int = 0
    peer_count: int = 0
    last_block_time: Optional[float] = None
    time_since_last_block_secs: float = 0.0
    consecutive_missed: int = 0
    recoveries_triggered: int = 0

    def to_dict(self) -> dict:
        return {
            "blocks_received": self.blocks_received,
            "missed_blocks": self.missed_blocks,
            "current_height": self.current_height,
            "peer_count": self.peer_count,
            "last_block_time": self.last_block_time,
            "time_since_last_block_secs": self.time_since_last_block_secs,
            "consecutive_missed": self.consecutive_missed,
            "recoveries_triggered": self.recoveries_triggered,
            "is_healthy": self.consecutive_missed == 0,
        }


class LivenessMonitor:
    """
    Liveness monitor for detecting and recovering from network stalls.

    Matches luxtensor-consensus LivenessMonitor.

    Usage:
        monitor = LivenessMonitor()

        # In block processing loop
        monitor.record_block(block.height)

        # Periodically check liveness
        action = monitor.check_liveness()
        if action == LivenessAction.NETWORK_STALLED:
            trigger_recovery()
    """

    def __init__(self, config: Optional[LivenessConfig] = None):
        self.config = config or LivenessConfig.default()
        self._stats = LivenessStats()
        self._stats.last_block_time = time.time()
        self._lock = threading.RLock()

    @classmethod
    def default(cls) -> 'LivenessMonitor':
        """Create with default configuration."""
        return cls()

    def record_block(self, height: int):
        """Record that a new block was produced."""
        with self._lock:
            now = time.time()

            # Only record if this is a new block
            if height > self._stats.current_height:
                self._stats.blocks_received += 1
                self._stats.current_height = height
                self._stats.last_block_time = now
                self._stats.consecutive_missed = 0
                self._stats.time_since_last_block_secs = 0.0

    def record_missed_block(self):
        """Record that we expected a block but didn't get one."""
        with self._lock:
            self._stats.missed_blocks += 1
            self._stats.consecutive_missed += 1

    def update_peer_count(self, count: int):
        """Update the current peer count."""
        with self._lock:
            self._stats.peer_count = count

    def check_liveness(self) -> LivenessAction:
        """Check network liveness and return recommended action."""
        with self._lock:
            now = time.time()

            # Update time since last block
            if self._stats.last_block_time:
                self._stats.time_since_last_block_secs = now - self._stats.last_block_time

            time_since_block = self._stats.time_since_last_block_secs

            # Check peer count first
            if self._stats.peer_count < self.config.min_peers:
                return LivenessAction.DISCOVER_PEERS

            if self._stats.peer_count < self.config.low_peer_threshold:
                # Low peers but not critical
                pass

            # Check block timing
            if time_since_block >= self.config.stall_timeout_secs:
                self._stats.recoveries_triggered += 1
                return LivenessAction.NETWORK_STALLED

            if time_since_block >= self.config.warning_timeout_secs:
                return LivenessAction.REQUEST_SYNC

            if time_since_block >= self.config.block_timeout_secs:
                return LivenessAction.WARN_SLOW

            # Check consecutive missed blocks
            if self._stats.consecutive_missed >= self.config.max_missed_before_action:
                return LivenessAction.REQUEST_SYNC

            if self._stats.consecutive_missed > 0:
                return LivenessAction.WAIT_MORE

            return LivenessAction.HEALTHY

    @property
    def stats(self) -> LivenessStats:
        """Get current liveness statistics."""
        with self._lock:
            # Update time since last block
            if self._stats.last_block_time:
                self._stats.time_since_last_block_secs = time.time() - self._stats.last_block_time
            return LivenessStats(
                blocks_received=self._stats.blocks_received,
                missed_blocks=self._stats.missed_blocks,
                current_height=self._stats.current_height,
                peer_count=self._stats.peer_count,
                last_block_time=self._stats.last_block_time,
                time_since_last_block_secs=self._stats.time_since_last_block_secs,
                consecutive_missed=self._stats.consecutive_missed,
                recoveries_triggered=self._stats.recoveries_triggered,
            )

    def time_since_last_block(self) -> float:
        """Get time since last block (seconds)."""
        with self._lock:
            if self._stats.last_block_time:
                return time.time() - self._stats.last_block_time
            return 0.0

    def current_height(self) -> int:
        """Get current block height."""
        with self._lock:
            return self._stats.current_height

    def is_healthy(self) -> bool:
        """Check if network is healthy."""
        with self._lock:
            action = self.check_liveness()
            return action in [LivenessAction.HEALTHY, LivenessAction.WAIT_MORE]

    def reset(self):
        """Reset the liveness monitor (e.g., after recovery)."""
        with self._lock:
            self._stats = LivenessStats()
            self._stats.last_block_time = time.time()


# Module exports
__all__ = [
    "LivenessAction",
    "LivenessConfig",
    "LivenessStats",
    "LivenessMonitor",
]
