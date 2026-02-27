use libp2p::PeerId;
use luxtensor_core::types::Hash;
use std::time::{Duration, SystemTime};

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Peer ID
    pub peer_id: PeerId,

    /// Best block hash known by this peer
    pub best_hash: Hash,

    /// Best block height known by this peer
    pub best_height: u64,

    /// Genesis hash (for compatibility check)
    pub genesis_hash: Hash,

    /// Connection timestamp
    pub connected_at: SystemTime,

    /// Last seen timestamp
    pub last_seen: SystemTime,

    /// Reputation score (0-100)
    pub reputation: u8,

    /// Number of failed requests
    pub failed_requests: u32,

    /// Number of successful requests
    pub successful_requests: u32,

    /// Number of times this peer has disconnected and reconnected.
    ///
    /// Rapid reconnection is a classic Sybil/eclipse attack vector:
    /// an attacker repeatedly connects and disconnects with many identities
    /// to flood the peer table.  A high `disconnection_count` over a short
    /// period triggers automatic banning via `should_ban()`.
    pub disconnection_count: u32,

    /// Timestamp of the most recent disconnection, used together with
    /// `disconnection_count` to compute reconnection frequency.
    pub last_disconnected_at: Option<SystemTime>,
}

impl PeerInfo {
    /// Create a new peer info
    pub fn new(peer_id: PeerId, genesis_hash: Hash) -> Self {
        let now = SystemTime::now();
        Self {
            peer_id,
            best_hash: [0u8; 32],
            best_height: 0,
            genesis_hash,
            connected_at: now,
            last_seen: now,
            reputation: 100,
            failed_requests: 0,
            successful_requests: 0,
            disconnection_count: 0,
            last_disconnected_at: None,
        }
    }

    /// Update peer status
    pub fn update_status(&mut self, best_hash: Hash, best_height: u64) {
        self.best_hash = best_hash;
        self.best_height = best_height;
        self.last_seen = SystemTime::now();
    }

    /// Record a successful request
    pub fn record_success(&mut self) {
        self.successful_requests += 1;
        self.last_seen = SystemTime::now();

        // Increase reputation (max 100)
        if self.reputation < 100 {
            self.reputation = (self.reputation + 1).min(100);
        }
    }

    /// Record a failed request
    pub fn record_failure(&mut self) {
        self.failed_requests += 1;

        // Decrease reputation (min 0)
        self.reputation = self.reputation.saturating_sub(5);
    }

    /// Check if peer is active (seen recently)
    pub fn is_active(&self, timeout: Duration) -> bool {
        if let Ok(elapsed) = self.last_seen.elapsed() {
            elapsed < timeout
        } else {
            false
        }
    }

    /// Check if peer should be banned.
    ///
    /// A peer is banned if:
    /// - Reputation drops below 20, OR
    /// - More than 10 failed requests, OR
    /// - Rapid reconnection (> 5 disconnects within the last 10 minutes)
    pub fn should_ban(&self) -> bool {
        if self.reputation < 20 || self.failed_requests > 10 {
            return true;
        }

        // Sybil protection: rapid reconnection detection.
        // If a peer has disconnected > 5 times and the last disconnect was
        // within the last 10 minutes, ban it.
        if self.disconnection_count > 5 {
            if let Some(last_dc) = self.last_disconnected_at {
                if let Ok(elapsed) = last_dc.elapsed() {
                    if elapsed < Duration::from_secs(600) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Get success rate
    pub fn success_rate(&self) -> f64 {
        let total = self.successful_requests + self.failed_requests;
        if total == 0 {
            1.0
        } else {
            self.successful_requests as f64 / total as f64
        }
    }

    /// Record a disconnection event.
    ///
    /// Called when the peer drops the connection.  Incrementing the counter
    /// enables rapid-reconnect detection in `should_ban()`.
    pub fn record_disconnection(&mut self) {
        self.disconnection_count += 1;
        self.last_disconnected_at = Some(SystemTime::now());
    }

    /// Connection age factor for scoring.
    ///
    /// Returns a multiplier that favours long-lived connections:
    /// - Connected < 5 min → 0.5 (untrusted new peer)
    /// - Connected 5–30 min → 0.8 (warming up)
    /// - Connected ≥ 30 min → 1.0 (established)
    ///
    /// This makes Sybil attacks harder because freshly-connected fake
    /// peers cannot immediately gain high scores.
    pub fn age_factor(&self) -> f64 {
        let age = self.connected_at.elapsed().unwrap_or(Duration::ZERO);
        if age < Duration::from_secs(5 * 60) {
            0.5
        } else if age < Duration::from_secs(30 * 60) {
            0.8
        } else {
            1.0
        }
    }

    /// Composite connection quality score (0.0 – 100.0).
    ///
    /// Combines reputation, success rate, and connection age:
    ///
    /// ```text
    /// score = reputation × success_rate × age_factor
    /// ```
    ///
    /// Higher is better.  Used by `PeerManager::select_trusted_peers()` to
    /// prioritise which peers to rely on for critical operations (block
    /// relay, state sync pivot selection).
    pub fn connection_score(&self) -> f64 {
        self.reputation as f64 * self.success_rate() * self.age_factor()
    }
}

/// Peer manager for tracking connected peers
pub struct PeerManager {
    peers: std::collections::HashMap<PeerId, PeerInfo>,
    max_peers: usize,
}

impl PeerManager {
    /// Create a new peer manager
    pub fn new(max_peers: usize) -> Self {
        Self {
            peers: std::collections::HashMap::new(),
            max_peers,
        }
    }

    /// Add or update a peer
    pub fn add_peer(&mut self, peer_info: PeerInfo) -> bool {
        if self.peers.len() >= self.max_peers && !self.peers.contains_key(&peer_info.peer_id) {
            return false;
        }

        self.peers.insert(peer_info.peer_id, peer_info);
        true
    }

    /// Remove a peer
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }

    /// Get peer info
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }

    /// Get mutable peer info
    pub fn get_peer_mut(&mut self, peer_id: &PeerId) -> Option<&mut PeerInfo> {
        self.peers.get_mut(peer_id)
    }

    /// Get all peers
    pub fn get_all_peers(&self) -> Vec<&PeerInfo> {
        self.peers.values().collect()
    }

    /// Get active peers
    pub fn get_active_peers(&self, timeout: Duration) -> Vec<&PeerInfo> {
        self.peers
            .values()
            .filter(|peer| peer.is_active(timeout))
            .collect()
    }

    /// Get best peer (highest block height)
    pub fn get_best_peer(&self) -> Option<&PeerInfo> {
        self.peers
            .values()
            .filter(|peer| !peer.should_ban())
            .max_by_key(|peer| peer.best_height)
    }

    /// Clean up inactive or banned peers
    pub fn cleanup(&mut self, timeout: Duration) {
        self.peers.retain(|_, peer| {
            peer.is_active(timeout) && !peer.should_ban()
        });
    }

    /// Get peer count
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Check if we can accept more peers
    pub fn can_accept_peer(&self) -> bool {
        self.peers.len() < self.max_peers
    }

    /// Select the top `count` most-trusted peers, sorted by descending
    /// `connection_score()`.
    ///
    /// Excludes peers that should be banned.  Useful for choosing which
    /// peers to rely on for block relay, state sync, and other critical
    /// operations.
    pub fn select_trusted_peers(&self, count: usize) -> Vec<&PeerInfo> {
        let mut candidates: Vec<&PeerInfo> = self.peers
            .values()
            .filter(|p| !p.should_ban())
            .collect();
        // Sort descending by connection_score.
        candidates.sort_by(|a, b| {
            b.connection_score()
                .partial_cmp(&a.connection_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates.truncate(count);
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer_id() -> PeerId {
        PeerId::random()
    }

    #[test]
    fn test_peer_info_creation() {
        let peer_id = create_test_peer_id();
        let genesis = [0u8; 32];
        let peer = PeerInfo::new(peer_id, genesis);

        assert_eq!(peer.reputation, 100);
        assert_eq!(peer.best_height, 0);
    }

    #[test]
    fn test_peer_update_status() {
        let peer_id = create_test_peer_id();
        let genesis = [0u8; 32];
        let mut peer = PeerInfo::new(peer_id, genesis);

        let new_hash = [1u8; 32];
        peer.update_status(new_hash, 100);

        assert_eq!(peer.best_height, 100);
        assert_eq!(peer.best_hash, new_hash);
    }

    #[test]
    fn test_peer_reputation() {
        let peer_id = create_test_peer_id();
        let genesis = [0u8; 32];
        let mut peer = PeerInfo::new(peer_id, genesis);

        // Record failures
        for _ in 0..5 {
            peer.record_failure();
        }

        assert!(peer.reputation < 100);

        // Record successes
        for _ in 0..10 {
            peer.record_success();
        }

        assert!(peer.reputation > 75);
    }

    #[test]
    fn test_peer_should_ban() {
        let peer_id = create_test_peer_id();
        let genesis = [0u8; 32];
        let mut peer = PeerInfo::new(peer_id, genesis);

        // Should not ban initially
        assert!(!peer.should_ban());

        // Ban after many failures
        for _ in 0..20 {
            peer.record_failure();
        }

        assert!(peer.should_ban());
    }

    #[test]
    fn test_peer_manager() {
        let mut manager = PeerManager::new(10);

        let peer_id = create_test_peer_id();
        let genesis = [0u8; 32];
        let peer = PeerInfo::new(peer_id, genesis);

        assert!(manager.add_peer(peer));
        assert_eq!(manager.peer_count(), 1);

        manager.remove_peer(&peer_id);
        assert_eq!(manager.peer_count(), 0);
    }

    #[test]
    fn test_peer_manager_max_peers() {
        let mut manager = PeerManager::new(2);
        let genesis = [0u8; 32];

        // Add 2 peers
        let peer1 = PeerInfo::new(create_test_peer_id(), genesis);
        let peer2 = PeerInfo::new(create_test_peer_id(), genesis);

        assert!(manager.add_peer(peer1));
        assert!(manager.add_peer(peer2));

        // Third peer should fail
        let peer3 = PeerInfo::new(create_test_peer_id(), genesis);
        assert!(!manager.add_peer(peer3));
    }

    #[test]
    fn test_get_best_peer() {
        let mut manager = PeerManager::new(10);
        let genesis = [0u8; 32];

        let mut peer1 = PeerInfo::new(create_test_peer_id(), genesis);
        peer1.update_status([1u8; 32], 100);

        let mut peer2 = PeerInfo::new(create_test_peer_id(), genesis);
        peer2.update_status([2u8; 32], 200);

        let _peer1_id = peer1.peer_id;
        let peer2_id = peer2.peer_id;

        manager.add_peer(peer1);
        manager.add_peer(peer2);

        let best = manager.get_best_peer().unwrap();
        assert_eq!(best.peer_id, peer2_id);
        assert_eq!(best.best_height, 200);
    }

    #[test]
    fn test_connection_score_new_peer() {
        let peer = PeerInfo::new(create_test_peer_id(), [0u8; 32]);
        // New peer: reputation 100, success_rate 1.0, age_factor 0.5
        let score = peer.connection_score();
        assert!(
            (score - 50.0).abs() < 0.01,
            "New peer score should be ~50.0 (100 * 1.0 * 0.5), got {}",
            score
        );
    }

    #[test]
    fn test_connection_score_after_failures() {
        let mut peer = PeerInfo::new(create_test_peer_id(), [0u8; 32]);
        // Record 4 successes and 1 failure → success_rate = 0.8
        for _ in 0..4 {
            peer.record_success();
        }
        peer.record_failure();
        let rate = peer.success_rate();
        assert!((rate - 0.8).abs() < 0.01);
        // Score = reputation * 0.8 * 0.5 (still new)
        let score = peer.connection_score();
        assert!(score < 50.0, "Score after failure should be < 50, got {}", score);
    }

    #[test]
    fn test_sybil_rapid_reconnect_ban() {
        let mut peer = PeerInfo::new(create_test_peer_id(), [0u8; 32]);
        assert!(!peer.should_ban());

        // Simulate 6 rapid disconnections (within current second)
        for _ in 0..6 {
            peer.record_disconnection();
        }

        // Should be banned due to rapid reconnection
        assert!(
            peer.should_ban(),
            "Peer with 6 disconnections within 10 minutes should be banned"
        );
    }

    #[test]
    fn test_select_trusted_peers() {
        let mut manager = PeerManager::new(10);
        let genesis = [0u8; 32];

        // Peer A: good reputation, some successes
        let mut peer_a = PeerInfo::new(create_test_peer_id(), genesis);
        for _ in 0..10 {
            peer_a.record_success();
        }
        let peer_a_id = peer_a.peer_id;

        // Peer B: poor peer — many failures
        let mut peer_b = PeerInfo::new(create_test_peer_id(), genesis);
        for _ in 0..8 {
            peer_b.record_failure();
        }

        // Peer C: mediocre — mixed
        let mut peer_c = PeerInfo::new(create_test_peer_id(), genesis);
        for _ in 0..3 { peer_c.record_success(); }
        for _ in 0..3 { peer_c.record_failure(); }

        manager.add_peer(peer_a);
        manager.add_peer(peer_b);
        manager.add_peer(peer_c);

        let trusted = manager.select_trusted_peers(2);
        assert_eq!(trusted.len(), 2);
        // Best peer should be peer_a (highest score)
        assert_eq!(trusted[0].peer_id, peer_a_id);
    }

    #[test]
    fn test_disconnection_count_tracking() {
        let mut peer = PeerInfo::new(create_test_peer_id(), [0u8; 32]);
        assert_eq!(peer.disconnection_count, 0);
        assert!(peer.last_disconnected_at.is_none());

        peer.record_disconnection();
        assert_eq!(peer.disconnection_count, 1);
        assert!(peer.last_disconnected_at.is_some());

        peer.record_disconnection();
        assert_eq!(peer.disconnection_count, 2);
    }
}
