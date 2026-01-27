//! Network Security Tests
//!
//! Tests for peer banning, gossipsub validation, and bootstrap failover.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// Peer ban manager for tracking misbehaving peers
pub struct PeerBanManager {
    /// Banned peers with ban expiration time
    banned_peers: HashMap<String, Instant>,
    /// Strike count per peer (3 strikes = ban)
    strikes: HashMap<String, u32>,
    /// Ban duration
    ban_duration: Duration,
    /// Maximum strikes before ban
    max_strikes: u32,
}

impl Default for PeerBanManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerBanManager {
    pub fn new() -> Self {
        Self {
            banned_peers: HashMap::new(),
            strikes: HashMap::new(),
            ban_duration: Duration::from_secs(3600), // 1 hour ban
            max_strikes: 3,
        }
    }

    /// Record a strike against a peer
    pub fn add_strike(&mut self, peer_id: &str, reason: &str) {
        let count = self.strikes.entry(peer_id.to_string()).or_insert(0);
        *count += 1;

        tracing::warn!("⚠️ Strike {} for peer {}: {}", count, peer_id, reason);

        if *count >= self.max_strikes {
            self.ban_peer(peer_id);
        }
    }

    /// Ban a peer immediately
    pub fn ban_peer(&mut self, peer_id: &str) {
        let expires = Instant::now() + self.ban_duration;
        self.banned_peers.insert(peer_id.to_string(), expires);
        self.strikes.remove(peer_id);
        tracing::warn!("🚫 Banned peer {} for {:?}", peer_id, self.ban_duration);
    }

    /// Check if peer is banned
    pub fn is_banned(&mut self, peer_id: &str) -> bool {
        if let Some(expires) = self.banned_peers.get(peer_id) {
            if Instant::now() >= *expires {
                self.banned_peers.remove(peer_id);
                return false;
            }
            return true;
        }
        false
    }

    /// Unban a peer manually
    pub fn unban_peer(&mut self, peer_id: &str) {
        self.banned_peers.remove(peer_id);
    }

    /// Get list of currently banned peers
    pub fn get_banned_peers(&self) -> Vec<String> {
        self.banned_peers.keys().cloned().collect()
    }
}

/// Bootstrap node failover manager
pub struct BootstrapFailover {
    /// List of bootstrap nodes in priority order
    bootstrap_nodes: Vec<String>,
    /// Currently connected bootstrap
    current_bootstrap: Option<usize>,
    /// Failed connection attempts per node
    failures: HashMap<String, u32>,
    /// Maximum failures before trying next node
    max_failures: u32,
}

impl BootstrapFailover {
    pub fn new(bootstrap_nodes: Vec<String>) -> Self {
        Self {
            bootstrap_nodes,
            current_bootstrap: None,
            failures: HashMap::new(),
            max_failures: 3,
        }
    }

    /// Get next bootstrap node to try
    pub fn get_next_bootstrap(&mut self) -> Option<String> {
        // Start from current or beginning
        let start = self.current_bootstrap.map(|i| i + 1).unwrap_or(0);

        for i in 0..self.bootstrap_nodes.len() {
            let idx = (start + i) % self.bootstrap_nodes.len();
            let node = &self.bootstrap_nodes[idx];

            let failures = self.failures.get(node).copied().unwrap_or(0);
            if failures < self.max_failures {
                self.current_bootstrap = Some(idx);
                return Some(node.clone());
            }
        }

        // All nodes have failed too many times, reset and try again
        self.failures.clear();
        self.bootstrap_nodes.first().cloned()
    }

    /// Record connection failure
    pub fn record_failure(&mut self, node: &str) {
        let count = self.failures.entry(node.to_string()).or_insert(0);
        *count += 1;
    }

    /// Record successful connection
    pub fn record_success(&mut self, node: &str) {
        self.failures.remove(node);
    }
}

/// Gossipsub message validator
pub struct MessageValidator {
    /// Seen message IDs to prevent duplicates
    seen_messages: HashSet<[u8; 32]>,
    /// Maximum message size in bytes
    max_message_size: usize,
    /// Valid topics
    valid_topics: HashSet<String>,
}

impl Default for MessageValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageValidator {
    pub fn new() -> Self {
        let mut valid_topics = HashSet::new();
        valid_topics.insert("blocks".to_string());
        valid_topics.insert("transactions".to_string());
        valid_topics.insert("sync".to_string());

        Self {
            seen_messages: HashSet::new(),
            max_message_size: 10 * 1024 * 1024, // 10 MB
            valid_topics,
        }
    }

    /// Validate incoming message
    pub fn validate(&mut self, message_id: [u8; 32], topic: &str, data: &[u8]) -> Result<(), &'static str> {
        // Check topic
        if !self.valid_topics.contains(topic) {
            return Err("Invalid topic");
        }

        // Check size
        if data.len() > self.max_message_size {
            return Err("Message too large");
        }

        // Check duplicate
        if self.seen_messages.contains(&message_id) {
            return Err("Duplicate message");
        }

        // Mark as seen
        self.seen_messages.insert(message_id);

        // Cleanup old messages if too many (simple LRU approximation)
        if self.seen_messages.len() > 10000 {
            self.seen_messages.clear();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_ban_strikes() {
        let mut manager = PeerBanManager::new();
        let peer = "12D3KooWtest";

        // Add strikes
        manager.add_strike(peer, "invalid block");
        assert!(!manager.is_banned(peer));

        manager.add_strike(peer, "invalid tx");
        assert!(!manager.is_banned(peer));

        manager.add_strike(peer, "spam");
        assert!(manager.is_banned(peer), "Should be banned after 3 strikes");
    }

    #[test]
    fn test_peer_manual_ban() {
        let mut manager = PeerBanManager::new();
        let peer = "12D3KooWmalicious";

        manager.ban_peer(peer);
        assert!(manager.is_banned(peer));

        manager.unban_peer(peer);
        assert!(!manager.is_banned(peer));
    }

    #[test]
    fn test_bootstrap_failover() {
        let nodes = vec![
            "node1.example.com".to_string(),
            "node2.example.com".to_string(),
            "node3.example.com".to_string(),
        ];
        let mut failover = BootstrapFailover::new(nodes);

        // First call should return first node
        assert_eq!(failover.get_next_bootstrap(), Some("node1.example.com".to_string()));

        // Record 3 failures for node1
        for _ in 0..3 {
            failover.record_failure("node1.example.com");
        }

        // Next call should skip node1 and return node2
        assert_eq!(failover.get_next_bootstrap(), Some("node2.example.com".to_string()));
    }

    #[test]
    fn test_message_validation() {
        let mut validator = MessageValidator::new();
        let msg_id = [1u8; 32];

        // Valid message
        assert!(validator.validate(msg_id, "blocks", b"block data").is_ok());

        // Duplicate should fail
        assert!(validator.validate(msg_id, "blocks", b"block data").is_err());

        // Invalid topic
        let msg_id2 = [2u8; 32];
        assert!(validator.validate(msg_id2, "invalid_topic", b"data").is_err());

        // Too large message
        let msg_id3 = [3u8; 32];
        let large_data = vec![0u8; 11 * 1024 * 1024];
        assert!(validator.validate(msg_id3, "blocks", &large_data).is_err());
    }

    #[test]
    fn test_bootstrap_all_failed_reset() {
        let nodes = vec!["node1".to_string(), "node2".to_string()];
        let mut failover = BootstrapFailover::new(nodes);

        // Fail all nodes
        for _ in 0..3 {
            failover.record_failure("node1");
            failover.record_failure("node2");
        }

        // Should reset and return first node
        let next = failover.get_next_bootstrap();
        assert!(next.is_some(), "Should reset after all nodes failed");
    }
}
