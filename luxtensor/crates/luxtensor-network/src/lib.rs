// LuxTensor network module
// Phase 3: Network Layer Implementation

pub mod eclipse_protection;
pub mod error;
pub mod identity;
pub mod light_client;
pub mod messages;
pub mod peer;
pub mod peer_discovery;
pub mod rate_limiter;
pub mod seeds;
pub mod state_sync;
pub mod swarm;

// Legacy modules — only compiled with `legacy` feature flag
#[cfg(feature = "legacy")]
pub mod p2p;

pub use eclipse_protection::{
    EclipseConfig, EclipseProtection, EclipseStats, PeerInfo as EclipsePeerInfo,
};
pub use error::*;
pub use identity::{print_connection_info, NodeIdentity};
pub use libp2p::Multiaddr;
pub use light_client::{
    LightClientConfig, LightClientError, LightClientState, MerkleProof, SyncStatus, TrustedHeader,
};
pub use messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_SYNC, TOPIC_TRANSACTIONS};
#[cfg(feature = "legacy")]
pub use p2p::{GossipStats, GossipTopics, P2PConfig, P2PEvent, P2PNode};
pub use peer::{PeerInfo, PeerManager};
pub use peer_discovery::{
    DiscoveredPeer, DiscoveryConfig, DiscoveryStats, PeerCapabilities, PeerDiscovery,
};
pub use rate_limiter::{RateLimiter, RateLimiterConfig, RateLimiterStats};
pub use seeds::{get_seeds_for_chain, has_hardcoded_seeds};
pub use state_sync::{
    GetStateRange, GetStorageRange, StateRange, StateSnapshot, StateSyncConfig, StateSyncManager,
    StorageRange, SyncError, SyncPhase, SyncProgress,
};
pub use swarm::{SwarmCommand, SwarmP2PEvent, SwarmP2PNode};
