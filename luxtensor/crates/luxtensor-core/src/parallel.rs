//! Parallel Processing Utilities for LuxTensor
//!
//! Phase 7b optimization: Provides parallel processing utilities
//! using rayon for CPU-bound operations.
//!
//! ## Key Features
//! - Thread pool configuration
//! - Parallel batch processing for transactions
//! - Parallel HNSW operations
//! - Backpressure-aware parallelism

use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Configuration for parallel processing
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Maximum number of threads (0 = auto-detect)
    pub max_threads: usize,
    /// Minimum batch size before parallelizing
    pub min_batch_size: usize,
    /// Enable parallel processing
    pub enabled: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_threads: 0, // auto-detect
            min_batch_size: 4,
            enabled: true,
        }
    }
}

impl ParallelConfig {
    /// Create config with specific thread count
    pub fn with_threads(threads: usize) -> Self {
        Self {
            max_threads: threads,
            min_batch_size: 4,
            enabled: true,
        }
    }

    /// Disable parallel processing (useful for debugging)
    pub fn disabled() -> Self {
        Self {
            max_threads: 1,
            min_batch_size: usize::MAX,
            enabled: false,
        }
    }
}

/// Statistics for parallel operations
#[derive(Debug, Default)]
pub struct ParallelStats {
    pub total_operations: AtomicUsize,
    pub parallel_operations: AtomicUsize,
    pub sequential_operations: AtomicUsize,
}

impl ParallelStats {
    /// Record a parallel operation
    pub fn record_parallel(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.parallel_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a sequential operation
    pub fn record_sequential(&self) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.sequential_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Get parallel operation ratio
    pub fn parallel_ratio(&self) -> f64 {
        let total = self.total_operations.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            self.parallel_operations.load(Ordering::Relaxed) as f64 / total as f64
        }
    }
}

/// Process items in parallel with configurable batch size
///
/// Uses rayon for parallel processing when batch size exceeds threshold.
/// Falls back to sequential processing for small batches.
pub fn parallel_process<T, R, F>(items: Vec<T>, config: &ParallelConfig, f: F) -> Vec<R>
where
    T: Send + Sync,
    R: Send,
    F: Fn(T) -> R + Send + Sync,
{
    if !config.enabled || items.len() < config.min_batch_size {
        // Sequential fallback for small batches
        items.into_iter().map(f).collect()
    } else {
        // Parallel processing
        items.into_par_iter().map(f).collect()
    }
}

/// Process items in parallel with error handling
pub fn parallel_try_process<T, R, E, F>(
    items: Vec<T>,
    config: &ParallelConfig,
    f: F
) -> Result<Vec<R>, E>
where
    T: Send + Sync,
    R: Send,
    E: Send,
    F: Fn(T) -> Result<R, E> + Send + Sync,
{
    if !config.enabled || items.len() < config.min_batch_size {
        items.into_iter().map(f).collect()
    } else {
        items.into_par_iter().map(f).collect()
    }
}

/// Parallel batch insert for HNSW operations
///
/// Optimized for inserting multiple vectors into HNSW index.
/// Uses parallel distance calculations.
pub fn parallel_batch_distances<T, F>(items: &[T], query: &T, config: &ParallelConfig, distance_fn: F) -> Vec<f32>
where
    T: Send + Sync,
    F: Fn(&T, &T) -> f32 + Send + Sync,
{
    if !config.enabled || items.len() < config.min_batch_size {
        items.iter().map(|item| distance_fn(item, query)).collect()
    } else {
        items.par_iter().map(|item| distance_fn(item, query)).collect()
    }
}

/// Parallel signature verification
///
/// Verifies multiple signatures in parallel for transaction batches.
pub fn parallel_verify_batch<T, F>(
    items: &[T],
    config: &ParallelConfig,
    verify_fn: F
) -> Vec<bool>
where
    T: Send + Sync,
    F: Fn(&T) -> bool + Send + Sync,
{
    if !config.enabled || items.len() < config.min_batch_size {
        items.iter().map(verify_fn).collect()
    } else {
        items.par_iter().map(verify_fn).collect()
    }
}

/// Result aggregator for parallel operations
pub struct ParallelResults<T> {
    pub successes: Vec<T>,
    pub failures: Vec<usize>, // indices of failed items
}

impl<T> Default for ParallelResults<T> {
    fn default() -> Self {
        Self {
            successes: Vec::new(),
            failures: Vec::new(),
        }
    }
}

/// Process items in parallel, collecting both successes and failures
pub fn parallel_process_with_failures<T, R, F>(
    items: Vec<T>,
    config: &ParallelConfig,
    f: F,
) -> ParallelResults<R>
where
    T: Send + Sync,
    R: Send,
    F: Fn(T) -> Option<R> + Send + Sync,
{
    let results: Vec<(usize, Option<R>)> = if !config.enabled || items.len() < config.min_batch_size {
        items.into_iter().enumerate().map(|(i, item)| (i, f(item))).collect()
    } else {
        items.into_par_iter().enumerate().map(|(i, item)| (i, f(item))).collect()
    };

    let mut output = ParallelResults::default();
    for (idx, result) in results {
        match result {
            Some(r) => output.successes.push(r),
            None => output.failures.push(idx),
        }
    }
    output
}

/// Get optimal thread count based on CPU cores and workload
pub fn optimal_thread_count(workload_size: usize) -> usize {
    let cpus = rayon::current_num_threads();
    if workload_size < cpus {
        workload_size.max(1)
    } else {
        cpus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.max_threads, 0);
        assert_eq!(config.min_batch_size, 4);
        assert!(config.enabled);
    }

    #[test]
    fn test_parallel_process_small_batch() {
        let config = ParallelConfig::default();
        let items = vec![1, 2, 3]; // below min_batch_size
        let results: Vec<i32> = parallel_process(items, &config, |x| x * 2);
        assert_eq!(results, vec![2, 4, 6]);
    }

    #[test]
    fn test_parallel_process_large_batch() {
        let config = ParallelConfig::default();
        let items: Vec<i32> = (0..100).collect();
        let results: Vec<i32> = parallel_process(items, &config, |x| x * 2);
        assert_eq!(results.len(), 100);
        assert_eq!(results[0], 0);
        assert_eq!(results[50], 100);
    }

    #[test]
    fn test_parallel_batch_distances() {
        let config = ParallelConfig::default();
        let items = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        let query = 3.0f32;
        let distances = parallel_batch_distances(&items, &query, &config, |a, b| (a - b).abs());
        assert_eq!(distances, vec![2.0, 1.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn test_parallel_stats() {
        let stats = ParallelStats::default();
        stats.record_parallel();
        stats.record_parallel();
        stats.record_sequential();

        assert_eq!(stats.total_operations.load(Ordering::Relaxed), 3);
        assert!((stats.parallel_ratio() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_optimal_thread_count() {
        let small_workload = optimal_thread_count(2);
        assert!(small_workload >= 1);

        let large_workload = optimal_thread_count(1000);
        assert!(large_workload >= 1);
    }
}
