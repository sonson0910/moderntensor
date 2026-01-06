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

    /// Check if peer should be banned (low reputation)
    pub fn should_ban(&self) -> bool {
        self.reputation < 20 || self.failed_requests > 10
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
}
