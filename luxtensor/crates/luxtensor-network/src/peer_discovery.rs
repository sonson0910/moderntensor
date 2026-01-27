//! Peer Auto-Discovery with Latency-Based Selection
//!
//! Automatically discovers peers from the P2P network and selects
//! the nearest/fastest ones based on measured latency.
//!
//! No hardcoding required - peers are discovered dynamically from:
//! - Gossipsub peer exchange
//! - Kademlia DHT
//! - mDNS for local networks
//! - Bootstrap nodes

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;

/// Discovered peer information
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    /// Peer ID (libp2p PeerId string)
    pub peer_id: String,
    /// Known addresses (may have multiple)
    pub addresses: Vec<String>,
    /// RPC endpoint if available
    pub rpc_endpoint: Option<String>,
    /// Measured latency in milliseconds
    pub latency_ms: f64,
    /// Geographic region (auto-detected from IP if possible)
    pub region: Option<String>,
    /// Last seen timestamp
    pub last_seen: Instant,
    /// Is this peer currently connected?
    pub is_connected: bool,
    /// Is this peer an RPC provider?
    pub provides_rpc: bool,
    /// Peer capabilities
    pub capabilities: PeerCapabilities,
}

/// Peer capabilities (what services the peer provides)
#[derive(Debug, Clone, Default)]
pub struct PeerCapabilities {
    /// Provides RPC service
    pub rpc: bool,
    /// Is a validator
    pub validator: bool,
    /// Provides full sync
    pub full_sync: bool,
    /// Supports light client
    pub light_client: bool,
}

/// Configuration for auto-discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// How often to probe peer latencies
    pub probe_interval: Duration,
    /// Timeout for latency probes
    pub probe_timeout: Duration,
    /// Maximum peers to track
    pub max_tracked_peers: usize,
    /// Minimum peers to maintain connections to
    pub min_connected_peers: usize,
    /// Prefer peers in same region
    pub prefer_local_region: bool,
    /// Maximum acceptable latency (ms)
    pub max_acceptable_latency_ms: f64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            probe_interval: Duration::from_secs(60),
            probe_timeout: Duration::from_secs(5),
            max_tracked_peers: 100,
            min_connected_peers: 8,
            prefer_local_region: true,
            max_acceptable_latency_ms: 500.0,
        }
    }
}

/// Peer Discovery Manager
///
/// Automatically discovers and ranks peers based on latency.
/// No hardcoding required - peers are discovered from the live network.
pub struct PeerDiscovery {
    /// Configuration
    config: DiscoveryConfig,
    /// Discovered peers (peer_id -> DiscoveredPeer)
    peers: RwLock<HashMap<String, DiscoveredPeer>>,
    /// Our own region (auto-detected)
    our_region: RwLock<Option<String>>,
    /// Latency history for EWMA calculation
    latency_history: RwLock<HashMap<String, Vec<f64>>>,
}

impl PeerDiscovery {
    /// Create a new peer discovery manager
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            config,
            peers: RwLock::new(HashMap::new()),
            our_region: RwLock::new(None),
            latency_history: RwLock::new(HashMap::new()),
        }
    }

    /// Register a newly discovered peer (called from P2P layer)
    pub fn on_peer_discovered(&self, peer_id: String, addresses: Vec<String>) {
        let mut peers = self.peers.write();

        if peers.len() >= self.config.max_tracked_peers {
            // Remove oldest peer if at capacity
            self.remove_oldest_peer(&mut peers);
        }

        let rpc_endpoint = self.extract_rpc_endpoint(&addresses);
        let region = self.detect_region_from_addresses(&addresses);

        peers.insert(peer_id.clone(), DiscoveredPeer {
            peer_id,
            addresses,
            rpc_endpoint,
            latency_ms: f64::MAX, // Unknown until probed
            region,
            last_seen: Instant::now(),
            is_connected: false,
            provides_rpc: false,
            capabilities: PeerCapabilities::default(),
        });
    }

    /// Update peer when connected
    pub fn on_peer_connected(&self, peer_id: &str) {
        if let Some(peer) = self.peers.write().get_mut(peer_id) {
            peer.is_connected = true;
            peer.last_seen = Instant::now();
        }
    }

    /// Update peer when disconnected
    pub fn on_peer_disconnected(&self, peer_id: &str) {
        if let Some(peer) = self.peers.write().get_mut(peer_id) {
            peer.is_connected = false;
        }
    }

    /// Update latency measurement for a peer
    pub fn update_latency(&self, peer_id: &str, latency_ms: f64) {
        // Update latency history for EWMA
        {
            let mut history = self.latency_history.write();
            let peer_history = history.entry(peer_id.to_string()).or_insert_with(Vec::new);
            peer_history.push(latency_ms);
            // Keep last 10 measurements
            if peer_history.len() > 10 {
                peer_history.remove(0);
            }
        }

        // Calculate EWMA latency
        let ewma_latency = {
            let history = self.latency_history.read();
            if let Some(peer_history) = history.get(peer_id) {
                self.calculate_ewma(peer_history)
            } else {
                latency_ms
            }
        };

        // Update peer
        if let Some(peer) = self.peers.write().get_mut(peer_id) {
            peer.latency_ms = ewma_latency;
            peer.last_seen = Instant::now();
        }
    }

    /// Calculate exponential weighted moving average
    fn calculate_ewma(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let alpha = 0.3; // Weight for new values
        let mut ewma = values[0];

        for &value in &values[1..] {
            ewma = alpha * value + (1.0 - alpha) * ewma;
        }

        ewma
    }

    /// Get the best (lowest latency) connected peer
    pub fn get_best_peer(&self) -> Option<DiscoveredPeer> {
        let peers = self.peers.read();

        peers.values()
            .filter(|p| p.is_connected)
            .filter(|p| p.latency_ms < self.config.max_acceptable_latency_ms)
            .min_by(|a, b| {
                a.latency_ms.partial_cmp(&b.latency_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    /// Get the best RPC endpoint (lowest latency peer with RPC)
    pub fn get_best_rpc_endpoint(&self) -> Option<String> {
        let peers = self.peers.read();

        peers.values()
            .filter(|p| p.provides_rpc && p.rpc_endpoint.is_some())
            .filter(|p| p.latency_ms < self.config.max_acceptable_latency_ms)
            .min_by(|a, b| {
                a.latency_ms.partial_cmp(&b.latency_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|p| p.rpc_endpoint.clone())
    }

    /// Get top N peers by latency
    pub fn get_top_peers(&self, count: usize) -> Vec<DiscoveredPeer> {
        let peers = self.peers.read();

        let mut sorted: Vec<_> = peers.values()
            .filter(|p| p.is_connected)
            .cloned()
            .collect();

        sorted.sort_by(|a, b| {
            a.latency_ms.partial_cmp(&b.latency_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted.into_iter().take(count).collect()
    }

    /// Get peers in the same region (for regional preference)
    pub fn get_regional_peers(&self) -> Vec<DiscoveredPeer> {
        let our_region = self.our_region.read().clone();
        let peers = self.peers.read();

        if let Some(region) = our_region {
            peers.values()
                .filter(|p| p.region.as_ref() == Some(&region))
                .filter(|p| p.is_connected)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all known RPC endpoints sorted by latency
    pub fn get_all_rpc_endpoints(&self) -> Vec<(String, f64)> {
        let peers = self.peers.read();

        let mut endpoints: Vec<_> = peers.values()
            .filter(|p| p.provides_rpc && p.rpc_endpoint.is_some())
            .map(|p| (p.rpc_endpoint.clone().unwrap(), p.latency_ms))
            .collect();

        endpoints.sort_by(|a, b| {
            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        endpoints
    }

    /// Update peer capabilities (called when peer announces capabilities)
    pub fn update_capabilities(&self, peer_id: &str, capabilities: PeerCapabilities) {
        if let Some(peer) = self.peers.write().get_mut(peer_id) {
            peer.capabilities = capabilities.clone();
            peer.provides_rpc = capabilities.rpc;
        }
    }

    /// Update RPC endpoint for a peer
    pub fn update_rpc_endpoint(&self, peer_id: &str, endpoint: String) {
        if let Some(peer) = self.peers.write().get_mut(peer_id) {
            peer.rpc_endpoint = Some(endpoint);
            peer.provides_rpc = true;
        }
    }

    /// Get discovery statistics
    pub fn get_stats(&self) -> DiscoveryStats {
        let peers = self.peers.read();

        let total_peers = peers.len();
        let connected_peers = peers.values().filter(|p| p.is_connected).count();
        let rpc_providers = peers.values().filter(|p| p.provides_rpc).count();

        let avg_latency = if connected_peers > 0 {
            peers.values()
                .filter(|p| p.is_connected && p.latency_ms < f64::MAX)
                .map(|p| p.latency_ms)
                .sum::<f64>() / connected_peers as f64
        } else {
            0.0
        };

        let best_latency = peers.values()
            .filter(|p| p.is_connected && p.latency_ms < f64::MAX)
            .map(|p| p.latency_ms)
            .fold(f64::MAX, f64::min);

        DiscoveryStats {
            total_discovered: total_peers,
            connected: connected_peers,
            rpc_providers,
            avg_latency_ms: avg_latency,
            best_latency_ms: if best_latency == f64::MAX { 0.0 } else { best_latency },
            our_region: self.our_region.read().clone(),
        }
    }

    /// Extract RPC endpoint from peer addresses
    fn extract_rpc_endpoint(&self, addresses: &[String]) -> Option<String> {
        // Look for HTTP addresses or guess from IP
        for addr in addresses {
            if addr.contains("http://") || addr.contains("https://") {
                return Some(addr.clone());
            }

            // Try to extract IP and assume port 8545
            if let Some(ip) = self.extract_ip_from_multiaddr(addr) {
                return Some(format!("http://{}:8545", ip));
            }
        }
        None
    }

    /// Extract IP from multiaddr
    fn extract_ip_from_multiaddr(&self, addr: &str) -> Option<String> {
        // Parse /ip4/x.x.x.x/tcp/... format
        if addr.starts_with("/ip4/") {
            let parts: Vec<&str> = addr.split('/').collect();
            if parts.len() >= 3 {
                return Some(parts[2].to_string());
            }
        }
        if addr.starts_with("/ip6/") {
            let parts: Vec<&str> = addr.split('/').collect();
            if parts.len() >= 3 {
                return Some(format!("[{}]", parts[2]));
            }
        }
        None
    }

    /// Detect region from IP addresses (simplified GeoIP)
    fn detect_region_from_addresses(&self, addresses: &[String]) -> Option<String> {
        for addr in addresses {
            if let Some(ip_str) = self.extract_ip_from_multiaddr(addr) {
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    return self.geoip_lookup(&ip);
                }
            }
        }
        None
    }

    /// Simple GeoIP lookup (would use maxmind in production)
    fn geoip_lookup(&self, ip: &IpAddr) -> Option<String> {
        // In production, use MaxMind GeoIP database
        // Here we do a simple heuristic based on IP ranges

        match ip {
            IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();

                // Very simplified region detection
                // In production, use a real GeoIP database
                match octets[0] {
                    1..=49 => Some("us-east".to_string()),
                    50..=99 => Some("us-west".to_string()),
                    100..=149 => Some("eu-west".to_string()),
                    150..=199 => Some("ap-south".to_string()),
                    200..=255 => Some("sa-east".to_string()),
                    _ => None,
                }
            }
            IpAddr::V6(_) => None, // Simplified
        }
    }

    /// Remove the oldest (least recently seen) peer
    fn remove_oldest_peer(&self, peers: &mut HashMap<String, DiscoveredPeer>) {
        if let Some((oldest_id, _)) = peers.iter()
            .filter(|(_, p)| !p.is_connected) // Don't remove connected peers
            .min_by_key(|(_, p)| p.last_seen)
        {
            let id = oldest_id.clone();
            peers.remove(&id);
        }
    }

    /// Set our own region (auto-detected or configured)
    pub fn set_our_region(&self, region: String) {
        *self.our_region.write() = Some(region);
    }
}

/// Discovery statistics
#[derive(Debug, Clone)]
pub struct DiscoveryStats {
    pub total_discovered: usize,
    pub connected: usize,
    pub rpc_providers: usize,
    pub avg_latency_ms: f64,
    pub best_latency_ms: f64,
    pub our_region: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_discovery() {
        let discovery = PeerDiscovery::new(DiscoveryConfig::default());

        // Discover some peers
        discovery.on_peer_discovered(
            "peer1".to_string(),
            vec!["/ip4/1.2.3.4/tcp/30303".to_string()],
        );
        discovery.on_peer_discovered(
            "peer2".to_string(),
            vec!["/ip4/5.6.7.8/tcp/30303".to_string()],
        );

        // Connect and update latencies
        discovery.on_peer_connected("peer1");
        discovery.on_peer_connected("peer2");
        discovery.update_latency("peer1", 50.0);
        discovery.update_latency("peer2", 100.0);

        // Best peer should be peer1 (lower latency)
        let best = discovery.get_best_peer();
        assert!(best.is_some());
        assert_eq!(best.unwrap().peer_id, "peer1");
    }

    #[test]
    fn test_rpc_endpoint_discovery() {
        let discovery = PeerDiscovery::new(DiscoveryConfig::default());

        discovery.on_peer_discovered(
            "rpc-peer".to_string(),
            vec!["/ip4/10.0.0.1/tcp/30303".to_string()],
        );
        discovery.on_peer_connected("rpc-peer");
        discovery.update_rpc_endpoint("rpc-peer", "http://10.0.0.1:8545".to_string());
        discovery.update_latency("rpc-peer", 25.0);

        let endpoint = discovery.get_best_rpc_endpoint();
        assert!(endpoint.is_some());
        assert_eq!(endpoint.unwrap(), "http://10.0.0.1:8545");
    }

    #[test]
    fn test_latency_ewma() {
        let discovery = PeerDiscovery::new(DiscoveryConfig::default());

        discovery.on_peer_discovered("peer".to_string(), vec![]);
        discovery.on_peer_connected("peer");

        // Update multiple times
        discovery.update_latency("peer", 100.0);
        discovery.update_latency("peer", 50.0);
        discovery.update_latency("peer", 50.0);

        let peer = discovery.get_best_peer().unwrap();
        // Should be somewhere between 50 and 100 due to EWMA
        assert!(peer.latency_ms > 50.0 && peer.latency_ms < 100.0);
    }
}
