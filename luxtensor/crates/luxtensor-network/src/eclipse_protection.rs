//! Eclipse Attack Protection
//!
//! Protects against eclipse attacks by:
//! - Maintaining peer diversity across IP ranges
//! - Limiting connections per IP subnet
//! - Requiring minimum outbound connections
//! - Tracking peer behavior scores

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use parking_lot::RwLock;

/// Eclipse protection configuration
#[derive(Debug, Clone)]
pub struct EclipseConfig {
    /// Maximum peers from same /16 subnet
    pub max_peers_per_subnet16: usize,
    /// Maximum peers from same /24 subnet
    pub max_peers_per_subnet24: usize,
    /// Minimum outbound connections required
    pub min_outbound_connections: usize,
    /// Maximum inbound connections
    pub max_inbound_connections: usize,
    /// Peer rotation interval in seconds
    pub peer_rotation_interval: u64,
    /// Minimum peer diversity score (0-100)
    pub min_diversity_score: u32,
}

impl Default for EclipseConfig {
    fn default() -> Self {
        Self {
            max_peers_per_subnet16: 4,
            max_peers_per_subnet24: 2,
            min_outbound_connections: 8,
            max_inbound_connections: 100,
            peer_rotation_interval: 3600, // 1 hour
            min_diversity_score: 50,
        }
    }
}

/// Peer connection info
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    pub ip_addr: IpAddr,
    pub is_outbound: bool,
    pub connected_at: u64,
    pub behavior_score: i32, // -100 to 100
}

/// Eclipse protection manager
pub struct EclipseProtection {
    config: EclipseConfig,
    /// Connected peers
    peers: RwLock<HashMap<String, PeerInfo>>,
    /// IP addresses per /16 subnet
    subnet16_counts: RwLock<HashMap<String, usize>>,
    /// IP addresses per /24 subnet
    subnet24_counts: RwLock<HashMap<String, usize>>,
    /// Banned subnets
    banned_subnets: RwLock<HashSet<String>>,
}

impl EclipseProtection {
    pub fn new(config: EclipseConfig) -> Self {
        Self {
            config,
            peers: RwLock::new(HashMap::new()),
            subnet16_counts: RwLock::new(HashMap::new()),
            subnet24_counts: RwLock::new(HashMap::new()),
            banned_subnets: RwLock::new(HashSet::new()),
        }
    }

    /// Get /16 subnet key from IP
    fn get_subnet16(ip: &IpAddr) -> String {
        match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                format!("{}.{}", octets[0], octets[1])
            }
            IpAddr::V6(v6) => {
                let segments = v6.segments();
                format!("{:x}:{:x}", segments[0], segments[1])
            }
        }
    }

    /// Get /24 subnet key from IP
    fn get_subnet24(ip: &IpAddr) -> String {
        match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                format!("{}.{}.{}", octets[0], octets[1], octets[2])
            }
            IpAddr::V6(v6) => {
                let segments = v6.segments();
                format!("{:x}:{:x}:{:x}", segments[0], segments[1], segments[2])
            }
        }
    }

    /// Check if a new connection should be allowed
    pub fn should_allow_connection(&self, ip: &IpAddr, is_outbound: bool) -> bool {
        let subnet16 = Self::get_subnet16(ip);
        let subnet24 = Self::get_subnet24(ip);

        // Check banned subnets
        if self.banned_subnets.read().contains(&subnet16) ||
           self.banned_subnets.read().contains(&subnet24) {
            return false;
        }

        // Check /16 limit
        let subnet16_count = self.subnet16_counts.read().get(&subnet16).copied().unwrap_or(0);
        if subnet16_count >= self.config.max_peers_per_subnet16 {
            tracing::warn!("🛡️ Rejecting connection from {} - /16 subnet limit reached", ip);
            return false;
        }

        // Check /24 limit
        let subnet24_count = self.subnet24_counts.read().get(&subnet24).copied().unwrap_or(0);
        if subnet24_count >= self.config.max_peers_per_subnet24 {
            tracing::warn!("🛡️ Rejecting connection from {} - /24 subnet limit reached", ip);
            return false;
        }

        // Check inbound limit
        if !is_outbound {
            let inbound_count = self.peers.read().values()
                .filter(|p| !p.is_outbound)
                .count();
            if inbound_count >= self.config.max_inbound_connections {
                tracing::warn!("🛡️ Rejecting inbound connection - limit reached");
                return false;
            }
        }

        true
    }

    /// Register a new peer connection
    pub fn add_peer(&self, peer_id: String, ip: IpAddr, is_outbound: bool) {
        let subnet16 = Self::get_subnet16(&ip);
        let subnet24 = Self::get_subnet24(&ip);

        let peer_info = PeerInfo {
            peer_id: peer_id.clone(),
            ip_addr: ip,
            is_outbound,
            connected_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            behavior_score: 50, // Start neutral
        };

        self.peers.write().insert(peer_id, peer_info);
        *self.subnet16_counts.write().entry(subnet16).or_insert(0) += 1;
        *self.subnet24_counts.write().entry(subnet24).or_insert(0) += 1;
    }

    /// Remove a peer
    pub fn remove_peer(&self, peer_id: &str) {
        if let Some(peer) = self.peers.write().remove(peer_id) {
            let subnet16 = Self::get_subnet16(&peer.ip_addr);
            let subnet24 = Self::get_subnet24(&peer.ip_addr);

            let mut subnet16_counts = self.subnet16_counts.write();
            if let Some(count) = subnet16_counts.get_mut(&subnet16) {
                *count = count.saturating_sub(1);
            }

            let mut subnet24_counts = self.subnet24_counts.write();
            if let Some(count) = subnet24_counts.get_mut(&subnet24) {
                *count = count.saturating_sub(1);
            }
        }
    }

    /// Update peer behavior score
    pub fn update_peer_score(&self, peer_id: &str, delta: i32) {
        if let Some(peer) = self.peers.write().get_mut(peer_id) {
            peer.behavior_score = (peer.behavior_score + delta).clamp(-100, 100);

            // Ban subnet if score too low
            if peer.behavior_score < -50 {
                let subnet16 = Self::get_subnet16(&peer.ip_addr);
                tracing::warn!("🚫 Banning subnet {} due to low behavior score", subnet16);
                self.banned_subnets.write().insert(subnet16);
            }
        }
    }

    /// Calculate peer diversity score (0-100)
    pub fn calculate_diversity_score(&self) -> u32 {
        let peers = self.peers.read();
        let total_peers = peers.len();

        if total_peers == 0 {
            return 0;
        }

        let subnet16_counts = self.subnet16_counts.read();
        let unique_subnets = subnet16_counts.len();

        // Score based on subnet diversity
        // Perfect score: every peer from different /16
        let diversity_ratio = unique_subnets as f64 / total_peers as f64;
        (diversity_ratio * 100.0) as u32
    }

    /// Check if peer diversity is sufficient
    pub fn is_diversity_sufficient(&self) -> bool {
        self.calculate_diversity_score() >= self.config.min_diversity_score
    }

    /// Get count of outbound connections
    pub fn outbound_connection_count(&self) -> usize {
        self.peers.read().values()
            .filter(|p| p.is_outbound)
            .count()
    }

    /// Check if we need more outbound connections
    pub fn needs_more_outbound(&self) -> bool {
        self.outbound_connection_count() < self.config.min_outbound_connections
    }

    /// Get peers that should be rotated (old or low score)
    pub fn get_peers_to_rotate(&self, max_count: usize) -> Vec<String> {
        let peers = self.peers.read();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut candidates: Vec<_> = peers.iter()
            .filter(|(_, p)| {
                // Rotate if old or low score
                let age = now.saturating_sub(p.connected_at);
                age > self.config.peer_rotation_interval || p.behavior_score < 0
            })
            .map(|(id, p)| (id.clone(), p.behavior_score))
            .collect();

        // Sort by score (lowest first)
        candidates.sort_by_key(|(_, score)| *score);
        candidates.truncate(max_count);

        candidates.into_iter().map(|(id, _)| id).collect()
    }

    /// Get statistics
    pub fn get_stats(&self) -> EclipseStats {
        let peers = self.peers.read();
        EclipseStats {
            total_peers: peers.len(),
            outbound_peers: peers.values().filter(|p| p.is_outbound).count(),
            inbound_peers: peers.values().filter(|p| !p.is_outbound).count(),
            unique_subnet16s: self.subnet16_counts.read().len(),
            unique_subnet24s: self.subnet24_counts.read().len(),
            diversity_score: self.calculate_diversity_score(),
            banned_subnets: self.banned_subnets.read().len(),
        }
    }
}

/// Eclipse protection statistics
#[derive(Debug, Clone)]
pub struct EclipseStats {
    pub total_peers: usize,
    pub outbound_peers: usize,
    pub inbound_peers: usize,
    pub unique_subnet16s: usize,
    pub unique_subnet24s: usize,
    pub diversity_score: u32,
    pub banned_subnets: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_eclipse_protection_creation() {
        let protection = EclipseProtection::new(EclipseConfig::default());
        assert_eq!(protection.outbound_connection_count(), 0);
    }

    #[test]
    fn test_subnet_limits() {
        let config = EclipseConfig {
            max_peers_per_subnet24: 2,
            ..Default::default()
        };
        let protection = EclipseProtection::new(config);

        let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));
        let ip3 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 3));

        // First two should be allowed
        assert!(protection.should_allow_connection(&ip1, false));
        protection.add_peer("peer1".to_string(), ip1, false);

        assert!(protection.should_allow_connection(&ip2, false));
        protection.add_peer("peer2".to_string(), ip2, false);

        // Third from same /24 should be rejected
        assert!(!protection.should_allow_connection(&ip3, false));
    }

    #[test]
    fn test_peer_diversity() {
        let protection = EclipseProtection::new(EclipseConfig::default());

        // Add peers from different subnets
        protection.add_peer("p1".to_string(), IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), true);
        protection.add_peer("p2".to_string(), IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)), true);
        protection.add_peer("p3".to_string(), IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3)), true);

        // Perfect diversity - 3 peers from 3 different /16 subnets
        assert_eq!(protection.calculate_diversity_score(), 100);
    }

    #[test]
    fn test_subnet_ban() {
        let protection = EclipseProtection::new(EclipseConfig::default());
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        protection.add_peer("bad_peer".to_string(), ip, false);

        // Lower score significantly
        for _ in 0..6 {
            protection.update_peer_score("bad_peer", -10);
        }

        // New connection from same subnet should be blocked
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 2, 2));
        assert!(!protection.should_allow_connection(&ip2, false));
    }
}
