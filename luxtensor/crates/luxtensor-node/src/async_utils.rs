//! Async Utilities for LuxTensor Node
//!
//! Phase 7c optimization: Provides utilities for offloading blocking I/O
//! operations to the tokio blocking thread pool.
//!
//! ## Key Features
//! - `spawn_blocking` wrappers for RocksDB operations
//! - Async-safe storage access patterns
//! - Timeout and backpressure handling

use std::future::Future;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Timeout for blocking operations (default 5 seconds)
const DEFAULT_BLOCKING_TIMEOUT: Duration = Duration::from_secs(5);

/// Offload a blocking closure to the tokio blocking thread pool
///
/// Use this for I/O-bound operations that would block the async runtime:
/// - RocksDB reads/writes
/// - File system operations
/// - Cryptographic operations
///
/// # Example
/// ```ignore
/// let block = offload_blocking(|| db.get_block(&hash)).await?;
/// ```
pub async fn offload_blocking<F, T>(f: F) -> Result<T, AsyncError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AsyncError::JoinError(e.to_string()))
}

/// Offload a blocking closure with timeout
///
/// Returns error if operation takes longer than specified duration.
pub async fn offload_blocking_timeout<F, T>(
    f: F,
    duration: Duration
) -> Result<T, AsyncError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    timeout(duration, tokio::task::spawn_blocking(f))
        .await
        .map_err(|_| AsyncError::Timeout)?
        .map_err(|e| AsyncError::JoinError(e.to_string()))
}

/// Offload with default timeout (5 seconds)
pub async fn offload_blocking_default<F, T>(f: F) -> Result<T, AsyncError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    offload_blocking_timeout(f, DEFAULT_BLOCKING_TIMEOUT).await
}

/// Offload a fallible blocking closure
pub async fn offload_blocking_try<F, T, E>(f: F) -> Result<T, AsyncError>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: Send + 'static,
    E: std::error::Error + Send + 'static,
{
    let result = tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AsyncError::JoinError(e.to_string()))?;

    result.map_err(|e| AsyncError::Inner(e.to_string()))
}

/// Batch offload multiple blocking operations
///
/// Executes operations concurrently using spawn_blocking.
pub async fn offload_batch<F, T>(operations: Vec<F>) -> Vec<Result<T, AsyncError>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let handles: Vec<_> = operations
        .into_iter()
        .map(|f| tokio::task::spawn_blocking(f))
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let result = handle
            .await
            .map_err(|e| AsyncError::JoinError(e.to_string()));
        results.push(result);
    }
    results
}

/// Debounced executor for I/O operations
///
/// Useful for coalescing multiple writes into a single operation.
pub struct DebouncedExecutor {
    /// Minimum delay between executions
    debounce_delay: Duration,
    /// Last execution timestamp
    last_execution: std::sync::atomic::AtomicU64,
}

impl DebouncedExecutor {
    pub fn new(debounce_delay: Duration) -> Self {
        Self {
            debounce_delay,
            last_execution: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Execute only if debounce period has passed
    pub async fn execute<F, T>(&self, f: F) -> Option<Result<T, AsyncError>>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let last = self.last_execution.load(std::sync::atomic::Ordering::Relaxed);
        let delay_ms = self.debounce_delay.as_millis() as u64;

        if now - last < delay_ms {
            debug!("Debounced execution skipped");
            return None;
        }

        self.last_execution.store(now, std::sync::atomic::Ordering::Relaxed);
        Some(offload_blocking(f).await)
    }
}

/// Errors from async operations
#[derive(Debug, thiserror::Error)]
pub enum AsyncError {
    #[error("Join error: {0}")]
    JoinError(String),

    #[error("Operation timed out")]
    Timeout,

    #[error("Inner error: {0}")]
    Inner(String),

    #[error("Backpressure: too many pending operations")]
    Backpressure,
}

/// Backpressure controller for limiting concurrent blocking operations
pub struct BackpressureController {
    /// Maximum concurrent operations
    max_concurrent: usize,
    /// Current count of pending operations
    pending: std::sync::atomic::AtomicUsize,
}

impl BackpressureController {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            max_concurrent,
            pending: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Try to acquire a slot for a new operation
    pub fn try_acquire(&self) -> Option<BackpressureGuard> {
        let current = self.pending.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        if current >= self.max_concurrent {
            self.pending.fetch_sub(1, std::sync::atomic::Ordering::Release);
            warn!("Backpressure: {} pending operations (max: {})", current, self.max_concurrent);
            None
        } else {
            Some(BackpressureGuard { controller: self })
        }
    }

    /// Execute with backpressure control
    pub async fn execute<F, T>(&self, f: F) -> Result<T, AsyncError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let _guard = self.try_acquire().ok_or(AsyncError::Backpressure)?;
        offload_blocking(f).await
    }

    /// Current pending operation count
    pub fn pending_count(&self) -> usize {
        self.pending.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Guard that releases backpressure slot on drop
pub struct BackpressureGuard<'a> {
    controller: &'a BackpressureController,
}

impl<'a> Drop for BackpressureGuard<'a> {
    fn drop(&mut self) {
        self.controller.pending.fetch_sub(1, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_offload_blocking_simple() {
        let result = offload_blocking(|| 42).await.unwrap();
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_offload_blocking_timeout_success() {
        let result = offload_blocking_timeout(
            || std::thread::sleep(Duration::from_millis(10)),
            Duration::from_secs(1)
        ).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_offload_batch() {
        let ops: Vec<Box<dyn FnOnce() -> i32 + Send>> = vec![
            Box::new(|| 1),
            Box::new(|| 2),
            Box::new(|| 3),
        ];

        let results: Vec<_> = offload_batch(ops).await;
        assert_eq!(results.len(), 3);

        let values: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_backpressure_controller() {
        let controller = BackpressureController::new(2);

        let guard1 = controller.try_acquire();
        assert!(guard1.is_some());
        assert_eq!(controller.pending_count(), 1);

        let guard2 = controller.try_acquire();
        assert!(guard2.is_some());
        assert_eq!(controller.pending_count(), 2);

        // Should fail - at capacity
        let guard3 = controller.try_acquire();
        assert!(guard3.is_none());

        // Drop one, should be able to acquire again
        drop(guard1);
        let guard4 = controller.try_acquire();
        assert!(guard4.is_some());
    }
}
