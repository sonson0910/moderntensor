// Shared peer count for RPC
// This is a simple global counter updated by P2P layer

use std::sync::atomic::{AtomicUsize, Ordering};

lazy_static::lazy_static! {
    /// Global peer count, updated by P2P swarm events
    pub static ref PEER_COUNT: AtomicUsize = AtomicUsize::new(0);
}

/// Increment peer count (called on PeerConnected)
pub fn increment_peer_count() {
    PEER_COUNT.fetch_add(1, Ordering::SeqCst);
}

/// Decrement peer count (called on PeerDisconnected)
pub fn decrement_peer_count() {
    let current = PEER_COUNT.load(Ordering::SeqCst);
    if current > 0 {
        PEER_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Get current peer count
pub fn get_peer_count() -> usize {
    PEER_COUNT.load(Ordering::SeqCst)
}
