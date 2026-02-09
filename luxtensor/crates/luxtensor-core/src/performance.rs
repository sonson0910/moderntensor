// Performance optimization utilities for LuxTensor
// Provides caching, parallel processing, and optimized data structures

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// LRU cache for frequently accessed data
pub struct LruCache<K, V> {
    capacity: usize,
    cache: HashMap<K, CacheEntry<V>>,
    access_order: Vec<K>,
}

struct CacheEntry<V> {
    value: V,
    last_access: Instant,
    access_count: u64,
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> LruCache<K, V> {
    /// Create a new LRU cache with given capacity
    pub fn new(capacity: usize) -> Self {
        Self { capacity, cache: HashMap::new(), access_order: Vec::new() }
    }

    /// Get a value from cache
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.cache.get_mut(key) {
            entry.last_access = Instant::now();
            entry.access_count += 1;

            // Update access order
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
                self.access_order.push(key.clone());
            }

            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Insert a value into cache
    pub fn put(&mut self, key: K, value: V) {
        // Check if we need to evict
        if self.cache.len() >= self.capacity && !self.cache.contains_key(&key) {
            if let Some(evict_key) = self.access_order.first().cloned() {
                self.cache.remove(&evict_key);
                self.access_order.remove(0);
            }
        }

        let entry = CacheEntry { value, last_access: Instant::now(), access_count: 1 };

        self.cache.insert(key.clone(), entry);

        // Update access order
        if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
            self.access_order.remove(pos);
        }
        self.access_order.push(key);
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let total_accesses: u64 = self.cache.values().map(|e| e.access_count).sum();
        let avg_accesses =
            if !self.cache.is_empty() { total_accesses / self.cache.len() as u64 } else { 0 };

        CacheStats {
            size: self.cache.len(),
            capacity: self.capacity,
            total_accesses,
            average_accesses: avg_accesses,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
    pub total_accesses: u64,
    pub average_accesses: u64,
}

/// Thread-safe cache for concurrent access
pub struct ConcurrentCache<K, V> {
    cache: Arc<RwLock<LruCache<K, V>>>,
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> ConcurrentCache<K, V> {
    /// Create a new concurrent cache
    pub fn new(capacity: usize) -> Self {
        Self { cache: Arc::new(RwLock::new(LruCache::new(capacity))) }
    }

    /// Get a value from cache
    pub fn get(&self, key: &K) -> Option<V> {
        self.cache.write().get(key)
    }

    /// Insert a value into cache
    pub fn put(&self, key: K, value: V) {
        self.cache.write().put(key, value);
    }

    /// Clear the cache
    pub fn clear(&self) {
        self.cache.write().clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.cache.read().stats()
    }
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> Clone for ConcurrentCache<K, V> {
    fn clone(&self) -> Self {
        Self { cache: Arc::clone(&self.cache) }
    }
}

/// Batch processor for parallel transaction processing
pub struct BatchProcessor {
    batch_size: usize,
    max_parallel: usize,
}

impl BatchProcessor {
    /// Create a new batch processor
    pub fn new(batch_size: usize, max_parallel: usize) -> Self {
        Self { batch_size, max_parallel }
    }

    /// Get optimal batch size based on workload
    pub fn optimal_batch_size(&self, total_items: usize) -> usize {
        if total_items < self.batch_size {
            total_items
        } else {
            self.batch_size
        }
    }

    /// Calculate number of batches needed
    pub fn batch_count(&self, total_items: usize) -> usize {
        if self.batch_size == 0 {
            return if total_items > 0 { 1 } else { 0 };
        }
        (total_items + self.batch_size - 1) / self.batch_size
    }

    /// Get parallelism level
    pub fn parallelism(&self, batch_count: usize) -> usize {
        batch_count.min(self.max_parallel)
    }
}

impl Default for BatchProcessor {
    fn default() -> Self {
        Self::new(
            100, // Process 100 items per batch
            8,   // Use up to 8 parallel workers
        )
    }
}

/// Performance metrics collector
pub struct PerformanceMetrics {
    operation_times: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
}

impl PerformanceMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self { operation_times: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Record an operation duration
    pub fn record(&self, operation: &str, duration: Duration) {
        let mut times = self.operation_times.write();
        times.entry(operation.to_string()).or_insert_with(Vec::new).push(duration);
    }

    /// Get average duration for an operation
    pub fn average(&self, operation: &str) -> Option<Duration> {
        let times = self.operation_times.read();
        if let Some(durations) = times.get(operation) {
            if durations.is_empty() {
                return None;
            }
            let total: Duration = durations.iter().sum();
            Some(total / durations.len() as u32)
        } else {
            None
        }
    }

    /// Get all metrics
    pub fn get_all_metrics(&self) -> HashMap<String, MetricSummary> {
        let times = self.operation_times.read();
        let mut metrics = HashMap::new();

        for (operation, durations) in times.iter() {
            let durations: &Vec<Duration> = durations;
            if durations.is_empty() {
                continue;
            }

            let total: Duration = durations.iter().sum();
            let avg = total / durations.len() as u32;
            let min = durations.iter().min().copied().unwrap_or(Duration::ZERO);
            let max = durations.iter().max().copied().unwrap_or(Duration::ZERO);

            metrics.insert(
                operation.clone(),
                MetricSummary { count: durations.len(), total, average: avg, min, max },
            );
        }

        metrics
    }

    /// Clear all metrics
    pub fn clear(&self) {
        self.operation_times.write().clear();
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PerformanceMetrics {
    fn clone(&self) -> Self {
        Self { operation_times: Arc::clone(&self.operation_times) }
    }
}

/// Summary of metrics for an operation
#[derive(Debug, Clone)]
pub struct MetricSummary {
    pub count: usize,
    pub total: Duration,
    pub average: Duration,
    pub min: Duration,
    pub max: Duration,
}

/// Bloom filter for fast existence checks
pub struct BloomFilter {
    bits: Vec<bool>,
    hash_count: usize,
}

impl BloomFilter {
    /// Create a new bloom filter
    ///
    /// # Panics
    /// Panics if `expected_elements` is 0 or `false_positive_rate` is not in (0.0, 1.0).
    pub fn new(expected_elements: usize, false_positive_rate: f64) -> Self {
        assert!(expected_elements > 0, "BloomFilter: expected_elements must be > 0");
        assert!(
            false_positive_rate > 0.0 && false_positive_rate < 1.0,
            "BloomFilter: false_positive_rate must be in (0.0, 1.0), got {}",
            false_positive_rate,
        );

        let bits_count = Self::optimal_bits(expected_elements, false_positive_rate).max(1);
        let hash_count = Self::optimal_hashes(expected_elements, bits_count).max(1);

        Self { bits: vec![false; bits_count], hash_count }
    }

    fn optimal_bits(n: usize, p: f64) -> usize {
        if n == 0 {
            return 1; // Safety floor
        }
        let bits = -(n as f64 * p.ln()) / (2.0_f64.ln().powi(2));
        (bits.ceil() as usize).max(1)
    }

    fn optimal_hashes(n: usize, m: usize) -> usize {
        if n == 0 {
            return 1; // Safety floor — prevents division by zero
        }
        let hashes = (m as f64 / n as f64) * 2.0_f64.ln();
        (hashes.ceil() as usize).max(1)
    }

    fn hash(&self, item: &[u8], seed: usize) -> usize {
        let mut hash: usize = seed;
        for &byte in item {
            hash = hash.wrapping_mul(31).wrapping_add(byte as usize);
        }
        hash % self.bits.len()
    }

    /// Add an item to the filter
    pub fn add(&mut self, item: &[u8]) {
        for i in 0..self.hash_count {
            let index = self.hash(item, i);
            self.bits[index] = true;
        }
    }

    /// Check if an item might be in the set
    pub fn contains(&self, item: &[u8]) -> bool {
        for i in 0..self.hash_count {
            let index = self.hash(item, i);
            if !self.bits[index] {
                return false;
            }
        }
        true
    }

    /// Clear the filter
    pub fn clear(&mut self) {
        for bit in &mut self.bits {
            *bit = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_basic() {
        let mut cache = LruCache::new(3);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"b"), Some(2));
        assert_eq!(cache.get(&"c"), Some(3));
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = LruCache::new(2);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3); // Should evict "a"

        assert_eq!(cache.get(&"a"), None);
        assert_eq!(cache.get(&"b"), Some(2));
        assert_eq!(cache.get(&"c"), Some(3));
    }

    #[test]
    fn test_lru_cache_stats() {
        let mut cache = LruCache::new(10);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.get(&"a");
        cache.get(&"a");

        let stats = cache.stats();
        assert_eq!(stats.size, 2);
        assert_eq!(stats.capacity, 10);
    }

    #[test]
    fn test_concurrent_cache() {
        let cache = ConcurrentCache::new(10);

        cache.put("key1", "value1");
        cache.put("key2", "value2");

        assert_eq!(cache.get(&"key1"), Some("value1"));
        assert_eq!(cache.get(&"key2"), Some("value2"));
    }

    #[test]
    fn test_batch_processor() {
        let processor = BatchProcessor::new(10, 4);

        assert_eq!(processor.optimal_batch_size(5), 5);
        assert_eq!(processor.optimal_batch_size(15), 10);
        assert_eq!(processor.batch_count(25), 3);
        assert_eq!(processor.parallelism(2), 2);
        assert_eq!(processor.parallelism(10), 4);
    }

    #[test]
    fn test_performance_metrics() {
        let metrics = PerformanceMetrics::new();

        metrics.record("operation1", Duration::from_millis(100));
        metrics.record("operation1", Duration::from_millis(200));
        metrics.record("operation2", Duration::from_millis(50));

        let avg = metrics.average("operation1").unwrap();
        assert_eq!(avg, Duration::from_millis(150));

        let all = metrics.get_all_metrics();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_bloom_filter() {
        let mut filter = BloomFilter::new(100, 0.01);

        filter.add(b"test1");
        filter.add(b"test2");

        assert!(filter.contains(b"test1"));
        assert!(filter.contains(b"test2"));
        assert!(!filter.contains(b"test3"));
    }

    #[test]
    fn test_bloom_filter_clear() {
        let mut filter = BloomFilter::new(10, 0.01);

        filter.add(b"item");
        assert!(filter.contains(b"item"));

        filter.clear();
        assert!(!filter.contains(b"item"));
    }
}
