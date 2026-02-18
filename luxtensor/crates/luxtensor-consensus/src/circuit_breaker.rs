//! Circuit Breaker Pattern for AI/ML Layer Operations
//!
//! Prevents cascade failures when AI/ML operations timeout or fail.
//! Implements the Circuit Breaker pattern with three states:
//! - Closed: Normal operation, requests pass through
//! - Open: Failures exceeded threshold, requests fail fast
//! - HalfOpen: Testing if service recovered
//!
//! Reference: Architecture patterns for distributed systems resilience

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tracing::{info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Too many failures - requests fail fast
    Open,
    /// Testing if service recovered
    HalfOpen,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self::Closed
    }
}

/// Configuration for circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Duration to keep circuit open before testing
    pub open_duration: Duration,
    /// Number of successful requests to close circuit
    pub success_threshold: u32,
    /// Timeout for individual operations
    pub operation_timeout: Duration,
    /// Name for logging
    pub name: String,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            open_duration: Duration::from_secs(30),
            success_threshold: 3,
            operation_timeout: Duration::from_secs(10),
            name: "default".to_string(),
        }
    }
}

impl CircuitBreakerConfig {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    pub fn with_open_duration(mut self, duration: Duration) -> Self {
        self.open_duration = duration;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }

    pub fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }
}

/// Statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub rejected_requests: u64,
    pub state_changes: u32,
    pub current_state: CircuitState,
}

/// Circuit Breaker implementation
///
/// NOTE: Uses `Instant::now()` intentionally — circuit breaker timings are local
/// to each node and do not affect consensus determinism. The open-to-half-open
/// transition is based on wall-clock elapsed time, which is acceptable because
/// it only governs local request gating, not any on-chain state transition.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    /// Current state
    state: RwLock<CircuitState>,
    /// Consecutive failure count
    failure_count: AtomicU32,
    /// Consecutive success count (in half-open state)
    success_count: AtomicU32,
    /// Time when circuit was opened
    opened_at: RwLock<Option<Instant>>,
    /// Statistics
    total_requests: AtomicU64,
    successful_requests: AtomicU64,
    failed_requests: AtomicU64,
    rejected_requests: AtomicU64,
    state_changes: AtomicU32,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        info!("CircuitBreaker '{}' initialized", config.name);
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            opened_at: RwLock::new(None),
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            rejected_requests: AtomicU64::new(0),
            state_changes: AtomicU32::new(0),
        }
    }

    /// Create with default config
    pub fn with_name(name: &str) -> Self {
        Self::new(CircuitBreakerConfig::new(name))
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        self.check_and_update_state();
        *self.state.read()
    }

    /// Check if request should be allowed
    pub fn allow_request(&self) -> bool {
        self.check_and_update_state();

        let state = *self.state.read();
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => true, // Allow test requests
        }
    }

    /// Record a successful operation
    ///
    /// SECURITY: Holds state write lock across the entire operation to prevent
    /// TOCTOU races between state read and counter updates.
    pub fn record_success(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.successful_requests.fetch_add(1, Ordering::Relaxed);

        // Hold write lock to ensure atomic read-modify-write of state + counters
        let state = self.state.write();
        self.failure_count.store(0, Ordering::SeqCst);

        if *state == CircuitState::HalfOpen {
            let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
            if successes >= self.config.success_threshold {
                drop(state); // drop before transition_to which also takes write lock
                self.transition_to(CircuitState::Closed);
                return;
            }
        }
        drop(state);
    }

    /// Record a failed operation
    ///
    /// SECURITY: Holds state write lock across the entire operation to prevent
    /// TOCTOU races between state read and counter updates.
    pub fn record_failure(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.failed_requests.fetch_add(1, Ordering::Relaxed);

        // Hold write lock to ensure atomic read-modify-write of state + counters
        let state = self.state.write();
        self.success_count.store(0, Ordering::SeqCst);

        match *state {
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if failures >= self.config.failure_threshold {
                    drop(state);
                    self.transition_to(CircuitState::Open);
                    return;
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open reopens circuit
                drop(state);
                self.transition_to(CircuitState::Open);
                return;
            }
            CircuitState::Open => {
                // Already open, just count
            }
        }
        drop(state);
    }

    /// Record a rejected request (circuit open)
    pub fn record_rejected(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        self.rejected_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Execute operation with circuit breaker protection
    pub fn execute<F, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if !self.allow_request() {
            self.record_rejected();
            return Err(CircuitBreakerError::CircuitOpen);
        }

        match operation() {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(e) => {
                self.record_failure();
                Err(CircuitBreakerError::OperationFailed(e))
            }
        }
    }

    /// Execute with fallback value when circuit is open
    pub fn execute_with_fallback<F, T, E>(&self, operation: F, fallback: T) -> T
    where
        F: FnOnce() -> Result<T, E>,
        T: Clone,
    {
        match self.execute(operation) {
            Ok(result) => result,
            Err(_) => fallback,
        }
    }

    /// Get statistics
    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            total_requests: self.total_requests.load(Ordering::SeqCst),
            successful_requests: self.successful_requests.load(Ordering::SeqCst),
            failed_requests: self.failed_requests.load(Ordering::SeqCst),
            rejected_requests: self.rejected_requests.load(Ordering::SeqCst),
            state_changes: self.state_changes.load(Ordering::SeqCst),
            current_state: self.state(),
        }
    }

    /// Reset circuit breaker
    pub fn reset(&self) {
        *self.state.write() = CircuitState::Closed;
        *self.opened_at.write() = None;
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        info!("CircuitBreaker '{}' reset", self.config.name);
    }

    /// Check if circuit should transition from Open to HalfOpen
    fn check_and_update_state(&self) {
        let state = *self.state.read();
        if state == CircuitState::Open {
            if let Some(opened_at) = *self.opened_at.read() {
                if opened_at.elapsed() >= self.config.open_duration {
                    self.transition_to(CircuitState::HalfOpen);
                }
            }
        }
    }

    /// Transition to a new state
    fn transition_to(&self, new_state: CircuitState) {
        let old_state = *self.state.read();
        if old_state == new_state {
            return;
        }

        *self.state.write() = new_state;
        self.state_changes.fetch_add(1, Ordering::SeqCst);

        match new_state {
            CircuitState::Open => {
                *self.opened_at.write() = Some(Instant::now());
                self.success_count.store(0, Ordering::SeqCst);
                warn!(
                    "CircuitBreaker '{}': {} -> Open (failures: {})",
                    self.config.name,
                    format!("{:?}", old_state),
                    self.failure_count.load(Ordering::SeqCst)
                );
            }
            CircuitState::HalfOpen => {
                self.success_count.store(0, Ordering::SeqCst);
                info!(
                    "CircuitBreaker '{}': Open -> HalfOpen (testing recovery)",
                    self.config.name
                );
            }
            CircuitState::Closed => {
                *self.opened_at.write() = None;
                self.failure_count.store(0, Ordering::SeqCst);
                info!(
                    "CircuitBreaker '{}': {} -> Closed (recovered)",
                    self.config.name,
                    format!("{:?}", old_state)
                );
            }
        }
    }
}

/// Errors from circuit breaker
#[derive(Debug)]
pub enum CircuitBreakerError<E> {
    /// Circuit is open, request rejected
    CircuitOpen,
    /// Operation failed with inner error
    OperationFailed(E),
    /// Operation timed out
    Timeout,
}

impl<E: std::fmt::Display> std::fmt::Display for CircuitBreakerError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CircuitOpen => write!(f, "Circuit breaker is open"),
            Self::OperationFailed(e) => write!(f, "Operation failed: {}", e),
            Self::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for CircuitBreakerError<E> {}

/// AI Layer Circuit Breaker - specialized for AI/ML operations
pub struct AILayerCircuitBreaker {
    /// Circuit breaker for weight consensus
    pub weight_consensus: CircuitBreaker,
    /// Circuit breaker for commit-reveal
    pub commit_reveal: CircuitBreaker,
    /// Circuit breaker for emission distribution
    pub emission: CircuitBreaker,
}

impl AILayerCircuitBreaker {
    pub fn new() -> Self {
        Self {
            weight_consensus: CircuitBreaker::new(
                CircuitBreakerConfig::new("weight_consensus")
                    .with_failure_threshold(3)
                    .with_open_duration(Duration::from_secs(60))
                    .with_timeout(Duration::from_secs(30))
            ),
            commit_reveal: CircuitBreaker::new(
                CircuitBreakerConfig::new("commit_reveal")
                    .with_failure_threshold(5)
                    .with_open_duration(Duration::from_secs(120))
                    .with_timeout(Duration::from_secs(60))
            ),
            emission: CircuitBreaker::new(
                CircuitBreakerConfig::new("emission")
                    .with_failure_threshold(3)
                    .with_open_duration(Duration::from_secs(30))
                    .with_timeout(Duration::from_secs(15))
            ),
        }
    }

    /// Get overall AI layer health
    pub fn is_healthy(&self) -> bool {
        self.weight_consensus.state() == CircuitState::Closed
            && self.commit_reveal.state() == CircuitState::Closed
            && self.emission.state() == CircuitState::Closed
    }

    /// Get summary of all circuit breakers
    pub fn summary(&self) -> AILayerStatus {
        AILayerStatus {
            weight_consensus_state: self.weight_consensus.state(),
            commit_reveal_state: self.commit_reveal.state(),
            emission_state: self.emission.state(),
            healthy: self.is_healthy(),
        }
    }
}

impl Default for AILayerCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Status of AI layer circuit breakers
#[derive(Debug, Clone)]
pub struct AILayerStatus {
    pub weight_consensus_state: CircuitState,
    pub commit_reveal_state: CircuitState,
    pub emission_state: CircuitState,
    pub healthy: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::with_name("test");
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig::new("test")
                .with_failure_threshold(3)
        );

        // Record failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure(); // 3rd failure
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_transitions_to_half_open() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig::new("test")
                .with_failure_threshold(1)
                .with_open_duration(Duration::from_millis(50))
        );

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for open duration
        std::thread::sleep(Duration::from_millis(60));

        // Should transition to half-open
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_closes_after_successes() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig::new("test")
                .with_failure_threshold(1)
                .with_success_threshold(2)
                .with_open_duration(Duration::from_millis(10))
        );

        // Open the circuit
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait and transition to half-open
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Record successes
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        cb.record_success(); // 2nd success
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_failure_reopens() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig::new("test")
                .with_failure_threshold(1)
                .with_open_duration(Duration::from_millis(10))
        );

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Failure in half-open reopens
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_execute_with_fallback() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig::new("test")
                .with_failure_threshold(1)
        );

        // Open the circuit
        cb.record_failure();

        // Execute should return fallback
        let result = cb.execute_with_fallback(|| Ok::<i32, ()>(42), 0);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_statistics() {
        let cb = CircuitBreaker::with_name("test");

        cb.record_success();
        cb.record_success();
        cb.record_failure();

        let stats = cb.stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);
    }

    #[test]
    fn test_ai_layer_circuit_breaker() {
        let ai_cb = AILayerCircuitBreaker::new();

        assert!(ai_cb.is_healthy());

        // Trip weight consensus
        for _ in 0..3 {
            ai_cb.weight_consensus.record_failure();
        }

        assert!(!ai_cb.is_healthy());

        let status = ai_cb.summary();
        assert_eq!(status.weight_consensus_state, CircuitState::Open);
        assert_eq!(status.commit_reveal_state, CircuitState::Closed);
    }

    #[test]
    fn test_reset() {
        let cb = CircuitBreaker::new(
            CircuitBreakerConfig::new("test")
                .with_failure_threshold(1)
        );

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }
}
