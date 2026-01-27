//! RPC Load Balancer with Latency-Based Routing
//!
//! Automatically routes queries to the nearest/fastest node based on:
//! - Measured latency to each endpoint
//! - Node health status
//! - Automatic failover when nodes are down

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Node endpoint information
#[derive(Debug, Clone)]
pub struct NodeEndpoint {
    /// Endpoint URL (e.g., "http://node1.luxtensor.network:8545")
    pub url: String,
    /// Optional region identifier
    pub region: Option<String>,
    /// Priority (lower = higher priority, used as tiebreaker)
    pub priority: u32,
}

/// Health status of a node
#[derive(Debug, Clone)]
pub struct NodeHealth {
    /// Is the node currently healthy?
    pub is_healthy: bool,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Last measured latency
    pub last_latency_ms: f64,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Last health check timestamp
    pub last_check: Instant,
    /// Total successful requests
    pub success_count: u64,
    /// Total failed requests
    pub failure_count: u64,
}

impl Default for NodeHealth {
    fn default() -> Self {
        Self {
            is_healthy: true,
            avg_latency_ms: 100.0, // Assume 100ms initially
            last_latency_ms: 0.0,
            consecutive_failures: 0,
            last_check: Instant::now(),
            success_count: 0,
            failure_count: 0,
        }
    }
}

/// Configuration for the load balancer
#[derive(Debug, Clone)]
pub struct LoadBalancerConfig {
    /// Health check interval
    pub health_check_interval: Duration,
    /// Request timeout
    pub request_timeout: Duration,
    /// Max consecutive failures before marking unhealthy
    pub max_consecutive_failures: u32,
    /// Latency weight for rolling average (0.0-1.0)
    pub latency_ewma_alpha: f64,
    /// Minimum healthy nodes before falling back to unhealthy
    pub min_healthy_nodes: usize,
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            health_check_interval: Duration::from_secs(30),
            request_timeout: Duration::from_secs(10),
            max_consecutive_failures: 3,
            latency_ewma_alpha: 0.3, // 30% weight to new measurement
            min_healthy_nodes: 1,
        }
    }
}

/// RPC Load Balancer
///
/// Routes queries to the nearest/fastest available node.
pub struct RpcLoadBalancer {
    /// Configuration
    config: LoadBalancerConfig,
    /// Node endpoints
    endpoints: Vec<NodeEndpoint>,
    /// Health status for each node (indexed by URL)
    health: RwLock<HashMap<String, NodeHealth>>,
    /// HTTP client for health checks
    #[cfg(feature = "reqwest")]
    client: reqwest::Client,
}

impl RpcLoadBalancer {
    /// Create a new load balancer with the given endpoints
    pub fn new(endpoints: Vec<NodeEndpoint>, config: LoadBalancerConfig) -> Self {
        let mut health_map = HashMap::new();
        for endpoint in &endpoints {
            health_map.insert(endpoint.url.clone(), NodeHealth::default());
        }

        Self {
            config,
            endpoints,
            health: RwLock::new(health_map),
            #[cfg(feature = "reqwest")]
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Get the best (nearest/fastest) healthy node
    pub fn get_best_node(&self) -> Option<&NodeEndpoint> {
        let health = self.health.read();

        // First, try to find healthy nodes
        let mut candidates: Vec<_> = self.endpoints.iter()
            .filter(|e| {
                health.get(&e.url)
                    .map(|h| h.is_healthy)
                    .unwrap_or(false)
            })
            .collect();

        // If no healthy nodes, fall back to all nodes
        if candidates.is_empty() && self.endpoints.len() > 0 {
            eprintln!("[WARN] No healthy nodes available, falling back to all endpoints");
            candidates = self.endpoints.iter().collect();
        }

        if candidates.is_empty() {
            return None;
        }

        // Sort by latency (ascending) then priority (ascending)
        candidates.sort_by(|a, b| {
            let latency_a = health.get(&a.url)
                .map(|h| h.avg_latency_ms)
                .unwrap_or(f64::MAX);
            let latency_b = health.get(&b.url)
                .map(|h| h.avg_latency_ms)
                .unwrap_or(f64::MAX);

            latency_a.partial_cmp(&latency_b)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.priority.cmp(&b.priority))
        });

        candidates.first().copied()
    }

    /// Get multiple healthy nodes for redundant queries
    pub fn get_top_nodes(&self, count: usize) -> Vec<&NodeEndpoint> {
        let health = self.health.read();

        let mut candidates: Vec<_> = self.endpoints.iter()
            .filter(|e| {
                health.get(&e.url)
                    .map(|h| h.is_healthy)
                    .unwrap_or(false)
            })
            .collect();

        candidates.sort_by(|a, b| {
            let latency_a = health.get(&a.url)
                .map(|h| h.avg_latency_ms)
                .unwrap_or(f64::MAX);
            let latency_b = health.get(&b.url)
                .map(|h| h.avg_latency_ms)
                .unwrap_or(f64::MAX);

            latency_a.partial_cmp(&latency_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.into_iter().take(count).collect()
    }

    /// Record a successful request to a node
    pub fn record_success(&self, url: &str, latency_ms: f64) {
        let mut health = self.health.write();

        if let Some(node_health) = health.get_mut(url) {
            node_health.last_latency_ms = latency_ms;
            node_health.avg_latency_ms =
                (1.0 - self.config.latency_ewma_alpha) * node_health.avg_latency_ms
                + self.config.latency_ewma_alpha * latency_ms;
            node_health.consecutive_failures = 0;
            node_health.is_healthy = true;
            node_health.last_check = Instant::now();
            node_health.success_count += 1;

            // Debug logging removed for cleaner output
        }
    }

    /// Record a failed request to a node
    pub fn record_failure(&self, url: &str) {
        let mut health = self.health.write();

        if let Some(node_health) = health.get_mut(url) {
            node_health.consecutive_failures += 1;
            node_health.failure_count += 1;
            node_health.last_check = Instant::now();

            if node_health.consecutive_failures >= self.config.max_consecutive_failures {
                node_health.is_healthy = false;
                eprintln!(
                    "[WARN] Node {} marked unhealthy after {} consecutive failures",
                    url, node_health.consecutive_failures
                );
            }
        }
    }

    /// Get health statistics for all nodes
    pub fn get_all_health(&self) -> HashMap<String, NodeHealth> {
        self.health.read().clone()
    }

    /// Get health for a specific node
    pub fn get_node_health(&self, url: &str) -> Option<NodeHealth> {
        self.health.read().get(url).cloned()
    }

    /// Manually mark a node as healthy/unhealthy
    pub fn set_node_health(&self, url: &str, is_healthy: bool) {
        let mut health = self.health.write();
        if let Some(node_health) = health.get_mut(url) {
            node_health.is_healthy = is_healthy;
            if is_healthy {
                node_health.consecutive_failures = 0;
            }
        }
    }

    /// Get statistics summary
    pub fn get_stats(&self) -> LoadBalancerStats {
        let health = self.health.read();

        let healthy_count = health.values().filter(|h| h.is_healthy).count();
        let total_success: u64 = health.values().map(|h| h.success_count).sum();
        let total_failure: u64 = health.values().map(|h| h.failure_count).sum();

        let avg_latency = if healthy_count > 0 {
            health.values()
                .filter(|h| h.is_healthy)
                .map(|h| h.avg_latency_ms)
                .sum::<f64>() / healthy_count as f64
        } else {
            0.0
        };

        LoadBalancerStats {
            total_nodes: self.endpoints.len(),
            healthy_nodes: healthy_count,
            unhealthy_nodes: self.endpoints.len() - healthy_count,
            total_success_requests: total_success,
            total_failed_requests: total_failure,
            avg_latency_ms: avg_latency,
        }
    }

    /// Add a new endpoint dynamically
    pub fn add_endpoint(&mut self, endpoint: NodeEndpoint) {
        let url = endpoint.url.clone();
        self.endpoints.push(endpoint);
        self.health.write().insert(url, NodeHealth::default());
    }

    /// Remove an endpoint
    pub fn remove_endpoint(&mut self, url: &str) {
        self.endpoints.retain(|e| e.url != url);
        self.health.write().remove(url);
    }
}

/// Load balancer statistics
#[derive(Debug, Clone)]
pub struct LoadBalancerStats {
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub unhealthy_nodes: usize,
    pub total_success_requests: u64,
    pub total_failed_requests: u64,
    pub avg_latency_ms: f64,
}

/// Smart RPC caller that uses load balancing
pub struct SmartRpcClient {
    load_balancer: Arc<RpcLoadBalancer>,
    /// Retry count for failed requests
    max_retries: u32,
}

impl SmartRpcClient {
    pub fn new(load_balancer: Arc<RpcLoadBalancer>, max_retries: u32) -> Self {
        Self {
            load_balancer,
            max_retries,
        }
    }

    /// Make an RPC call with automatic failover
    pub async fn call<T, F, Fut>(&self, request_fn: F) -> Result<T, String>
    where
        F: Fn(String) -> Fut,
        Fut: std::future::Future<Output = Result<(T, f64), String>>,
    {
        let mut last_error = String::new();
        let mut tried_urls = Vec::new();

        for attempt in 0..=self.max_retries {
            // Get best node that we haven't tried yet
            let health = self.load_balancer.health.read();
            let endpoint = self.load_balancer.endpoints.iter()
                .filter(|e| !tried_urls.contains(&e.url))
                .filter(|e| {
                    health.get(&e.url)
                        .map(|h| h.is_healthy || attempt == self.max_retries)
                        .unwrap_or(true)
                })
                .min_by(|a, b| {
                    let latency_a = health.get(&a.url)
                        .map(|h| h.avg_latency_ms)
                        .unwrap_or(f64::MAX);
                    let latency_b = health.get(&b.url)
                        .map(|h| h.avg_latency_ms)
                        .unwrap_or(f64::MAX);
                    latency_a.partial_cmp(&latency_b).unwrap_or(std::cmp::Ordering::Equal)
                });
            drop(health);

            let url = match endpoint {
                Some(e) => e.url.clone(),
                None => {
                    // No more endpoints to try
                    break;
                }
            };

            tried_urls.push(url.clone());

            match request_fn(url.clone()).await {
                Ok((result, latency_ms)) => {
                    self.load_balancer.record_success(&url, latency_ms);
                    return Ok(result);
                }
                Err(e) => {
                    self.load_balancer.record_failure(&url);
                    last_error = format!("{}: {}", url, e);
                    // Debug logging
                }
            }
        }

        Err(format!("All nodes failed. Last error: {}", last_error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_endpoints() -> Vec<NodeEndpoint> {
        vec![
            NodeEndpoint {
                url: "http://node1.example.com:8545".to_string(),
                region: Some("us-east".to_string()),
                priority: 1,
            },
            NodeEndpoint {
                url: "http://node2.example.com:8545".to_string(),
                region: Some("eu-west".to_string()),
                priority: 2,
            },
            NodeEndpoint {
                url: "http://node3.example.com:8545".to_string(),
                region: Some("ap-south".to_string()),
                priority: 3,
            },
        ]
    }

    #[test]
    fn test_load_balancer_creation() {
        let lb = RpcLoadBalancer::new(create_test_endpoints(), LoadBalancerConfig::default());
        assert_eq!(lb.endpoints.len(), 3);
    }

    #[test]
    fn test_get_best_node() {
        let lb = RpcLoadBalancer::new(create_test_endpoints(), LoadBalancerConfig::default());

        // Initially all have same latency, should pick by priority
        let best = lb.get_best_node();
        assert!(best.is_some());
        assert_eq!(best.unwrap().priority, 1);
    }

    #[test]
    fn test_latency_based_selection() {
        let lb = RpcLoadBalancer::new(create_test_endpoints(), LoadBalancerConfig::default());

        // Record lower latency for node3
        lb.record_success("http://node3.example.com:8545", 10.0);
        lb.record_success("http://node3.example.com:8545", 10.0);
        lb.record_success("http://node3.example.com:8545", 10.0);

        // Node3 should now be selected despite lower priority
        let best = lb.get_best_node();
        assert!(best.is_some());
        assert_eq!(best.unwrap().url, "http://node3.example.com:8545");
    }

    #[test]
    fn test_failure_handling() {
        let config = LoadBalancerConfig {
            max_consecutive_failures: 3,
            ..Default::default()
        };
        let lb = RpcLoadBalancer::new(create_test_endpoints(), config);

        // Record failures for node1
        lb.record_failure("http://node1.example.com:8545");
        lb.record_failure("http://node1.example.com:8545");

        // Should still be healthy
        let health = lb.get_node_health("http://node1.example.com:8545");
        assert!(health.is_some());
        assert!(health.unwrap().is_healthy);

        // Third failure marks it unhealthy
        lb.record_failure("http://node1.example.com:8545");
        let health = lb.get_node_health("http://node1.example.com:8545");
        assert!(!health.unwrap().is_healthy);

        // Best node should now be node2
        let best = lb.get_best_node();
        assert_eq!(best.unwrap().url, "http://node2.example.com:8545");
    }

    #[test]
    fn test_stats() {
        let lb = RpcLoadBalancer::new(create_test_endpoints(), LoadBalancerConfig::default());

        lb.record_success("http://node1.example.com:8545", 50.0);
        lb.record_success("http://node2.example.com:8545", 100.0);
        lb.record_failure("http://node3.example.com:8545");

        let stats = lb.get_stats();
        assert_eq!(stats.total_nodes, 3);
        assert_eq!(stats.total_success_requests, 2);
        assert_eq!(stats.total_failed_requests, 1);
    }
}
