"""
Circuit Breaker Pattern for AI/ML Layer Operations.

Matches luxtensor-consensus/src/circuit_breaker.rs.

Prevents cascade failures when AI/ML operations timeout or fail.
Implements the Circuit Breaker pattern with three states:
- Closed: Normal operation, requests pass through
- Open: Too many failures, requests fail fast
- HalfOpen: Testing if service recovered
"""

from dataclasses import dataclass
from enum import Enum
from typing import Callable, Optional, TypeVar
import time
import threading

T = TypeVar('T')
E = TypeVar('E', bound=Exception)


class CircuitState(str, Enum):
    """Circuit breaker states."""
    CLOSED = "closed"       # Normal operation - requests pass through
    OPEN = "open"           # Too many failures - requests fail fast
    HALF_OPEN = "half_open"  # Testing if service recovered


@dataclass
class CircuitBreakerConfig:
    """
    Configuration for circuit breaker.

    Matches luxtensor-consensus CircuitBreakerConfig.
    """
    name: str = "default"
    failure_threshold: int = 5          # Failures before opening
    success_threshold: int = 3          # Successes in half-open to close
    open_duration_secs: float = 30.0    # Seconds to stay open
    timeout_secs: float = 10.0          # Operation timeout

    @classmethod
    def default(cls) -> 'CircuitBreakerConfig':
        return cls()

    def with_failure_threshold(self, threshold: int) -> 'CircuitBreakerConfig':
        self.failure_threshold = threshold
        return self

    def with_open_duration(self, secs: float) -> 'CircuitBreakerConfig':
        self.open_duration_secs = secs
        return self

    def with_timeout(self, secs: float) -> 'CircuitBreakerConfig':
        self.timeout_secs = secs
        return self

    def with_success_threshold(self, threshold: int) -> 'CircuitBreakerConfig':
        self.success_threshold = threshold
        return self


@dataclass
class CircuitBreakerStats:
    """
    Statistics for monitoring.

    Matches luxtensor-consensus CircuitBreakerStats.
    """
    total_requests: int = 0
    successful_requests: int = 0
    failed_requests: int = 0
    rejected_requests: int = 0
    current_failures: int = 0
    current_successes: int = 0
    state_changes: int = 0
    last_failure_time: Optional[float] = None
    last_state_change_time: Optional[float] = None

    def to_dict(self) -> dict:
        return {
            "total_requests": self.total_requests,
            "successful_requests": self.successful_requests,
            "failed_requests": self.failed_requests,
            "rejected_requests": self.rejected_requests,
            "success_rate": (
                self.successful_requests / self.total_requests * 100
                if self.total_requests > 0 else 0
            ),
            "current_failures": self.current_failures,
            "current_successes": self.current_successes,
            "state_changes": self.state_changes,
            "last_failure_time": self.last_failure_time,
        }


class CircuitBreakerError(Exception):
    """Errors from circuit breaker."""
    pass


class CircuitOpenError(CircuitBreakerError):
    """Circuit is open, request rejected."""
    pass


class OperationTimeoutError(CircuitBreakerError):
    """Operation timed out."""
    pass


class CircuitBreaker:
    """
    Circuit Breaker implementation.

    Matches luxtensor-consensus CircuitBreaker.

    Usage:
        cb = CircuitBreaker.with_name("ai_inference")

        try:
            result = cb.execute(lambda: ai_model.inference(data))
        except CircuitOpenError:
            # Use fallback
            result = fallback_value
    """

    def __init__(self, config: Optional[CircuitBreakerConfig] = None):
        self.config = config or CircuitBreakerConfig.default()
        self._state = CircuitState.CLOSED
        self._stats = CircuitBreakerStats()
        self._opened_at: Optional[float] = None
        self._lock = threading.RLock()

    @classmethod
    def with_name(cls, name: str) -> 'CircuitBreaker':
        """Create with named config."""
        config = CircuitBreakerConfig(name=name)
        return cls(config)

    @property
    def state(self) -> CircuitState:
        """Get current state."""
        with self._lock:
            self._check_and_update_state()
            return self._state

    def allow_request(self) -> bool:
        """Check if request should be allowed."""
        with self._lock:
            self._check_and_update_state()

            if self._state == CircuitState.CLOSED:
                return True
            elif self._state == CircuitState.HALF_OPEN:
                return True  # Allow test request
            else:  # OPEN
                return False

    def record_success(self):
        """Record a successful operation."""
        with self._lock:
            self._stats.total_requests += 1
            self._stats.successful_requests += 1
            self._stats.current_failures = 0
            self._stats.current_successes += 1

            if self._state == CircuitState.HALF_OPEN:
                if self._stats.current_successes >= self.config.success_threshold:
                    self._transition_to(CircuitState.CLOSED)

    def record_failure(self):
        """Record a failed operation."""
        with self._lock:
            self._stats.total_requests += 1
            self._stats.failed_requests += 1
            self._stats.current_failures += 1
            self._stats.current_successes = 0
            self._stats.last_failure_time = time.time()

            if self._state == CircuitState.HALF_OPEN:
                # Immediate open on failure in half-open
                self._transition_to(CircuitState.OPEN)
            elif self._state == CircuitState.CLOSED:
                if self._stats.current_failures >= self.config.failure_threshold:
                    self._transition_to(CircuitState.OPEN)

    def record_rejected(self):
        """Record a rejected request (circuit open)."""
        with self._lock:
            self._stats.total_requests += 1
            self._stats.rejected_requests += 1

    def execute(self, operation: Callable[[], T]) -> T:
        """
        Execute operation with circuit breaker protection.

        Args:
            operation: Function to execute

        Returns:
            Result of operation

        Raises:
            CircuitOpenError: If circuit is open
            Exception: If operation fails
        """
        if not self.allow_request():
            self.record_rejected()
            raise CircuitOpenError(f"Circuit '{self.config.name}' is open")

        try:
            result = operation()
            self.record_success()
            return result
        except Exception:
            self.record_failure()
            raise

    def execute_with_fallback(
        self,
        operation: Callable[[], T],
        fallback: T,
    ) -> T:
        """Execute with fallback value when circuit is open."""
        try:
            return self.execute(operation)
        except CircuitOpenError:
            return fallback
        except Exception:
            return fallback

    @property
    def stats(self) -> CircuitBreakerStats:
        """Get statistics."""
        with self._lock:
            return CircuitBreakerStats(
                total_requests=self._stats.total_requests,
                successful_requests=self._stats.successful_requests,
                failed_requests=self._stats.failed_requests,
                rejected_requests=self._stats.rejected_requests,
                current_failures=self._stats.current_failures,
                current_successes=self._stats.current_successes,
                state_changes=self._stats.state_changes,
                last_failure_time=self._stats.last_failure_time,
                last_state_change_time=self._stats.last_state_change_time,
            )

    def reset(self):
        """Reset circuit breaker."""
        with self._lock:
            self._state = CircuitState.CLOSED
            self._stats = CircuitBreakerStats()
            self._opened_at = None

    def _check_and_update_state(self):
        """Check if circuit should transition from Open to HalfOpen."""
        if self._state == CircuitState.OPEN and self._opened_at:
            elapsed = time.time() - self._opened_at
            if elapsed >= self.config.open_duration_secs:
                self._transition_to(CircuitState.HALF_OPEN)

    def _transition_to(self, new_state: CircuitState):
        """Transition to a new state."""
        if self._state == new_state:
            return

        _ = self._state  # Capture current state before transition
        self._state = new_state
        self._stats.state_changes += 1
        self._stats.last_state_change_time = time.time()

        if new_state == CircuitState.OPEN:
            self._opened_at = time.time()
        elif new_state == CircuitState.CLOSED:
            self._stats.current_failures = 0
            self._stats.current_successes = 0
        elif new_state == CircuitState.HALF_OPEN:
            self._stats.current_successes = 0


class CircuitBreakerRegistry:
    """
    Registry for managing multiple circuit breakers.
    """

    _instance: Optional['CircuitBreakerRegistry'] = None
    _lock = threading.Lock()

    def __new__(cls):
        with cls._lock:
            if cls._instance is None:
                cls._instance = super().__new__(cls)
                cls._instance._breakers = {}
                cls._instance._registry_lock = threading.RLock()
            return cls._instance

    def get(self, name: str) -> CircuitBreaker:
        """Get or create a circuit breaker by name."""
        with self._registry_lock:
            if name not in self._breakers:
                self._breakers[name] = CircuitBreaker.with_name(name)
            return self._breakers[name]

    def get_all_stats(self) -> dict:
        """Get stats for all circuit breakers."""
        with self._registry_lock:
            return {
                name: cb.stats.to_dict()
                for name, cb in self._breakers.items()
            }

    def reset_all(self):
        """Reset all circuit breakers."""
        with self._registry_lock:
            for cb in self._breakers.values():
                cb.reset()

    def remove(self, name: str) -> bool:
        """
        Remove a circuit breaker from the registry.

        Args:
            name: Name of the circuit breaker to remove

        Returns:
            True if removed, False if not found
        """
        with self._registry_lock:
            if name in self._breakers:
                del self._breakers[name]
                return True
            return False

    def clear(self) -> int:
        """
        Remove all circuit breakers from the registry.

        Returns:
            Number of circuit breakers removed
        """
        with self._registry_lock:
            count = len(self._breakers)
            self._breakers.clear()
            return count


# Global registry
def get_circuit_breaker(name: str) -> CircuitBreaker:
    """Get a circuit breaker by name from the global registry."""
    return CircuitBreakerRegistry().get(name)


# Module exports
__all__ = [
    "CircuitState",
    "CircuitBreakerConfig",
    "CircuitBreakerStats",
    "CircuitBreakerError",
    "CircuitOpenError",
    "OperationTimeoutError",
    "CircuitBreaker",
    "CircuitBreakerRegistry",
    "get_circuit_breaker",
]
