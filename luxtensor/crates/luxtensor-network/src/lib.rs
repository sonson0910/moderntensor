// LuxTensor network module
// Phase 3: Network Layer Implementation

pub mod error;
pub mod identity;
pub mod messages;
pub mod peer;
pub mod p2p;
pub mod rate_limiter;
pub mod seeds;
pub mod swarm;
pub mod eclipse_protection;
pub mod peer_discovery;

pub use error::*;
pub use identity::{NodeIdentity, print_connection_info};
pub use messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_TRANSACTIONS, TOPIC_SYNC};
pub use peer::{PeerInfo, PeerManager};
pub use p2p::{P2PConfig, P2PEvent, P2PNode, GossipTopics, GossipStats};
pub use libp2p::Multiaddr;
pub use rate_limiter::{RateLimiter, RateLimiterConfig, RateLimiterStats};
pub use seeds::{get_seeds_for_chain, has_hardcoded_seeds};
pub use swarm::{SwarmP2PNode, SwarmP2PEvent, SwarmCommand};
pub use eclipse_protection::{EclipseProtection, EclipseConfig, EclipseStats, PeerInfo as EclipsePeerInfo};
pub use peer_discovery::{PeerDiscovery, DiscoveryConfig, DiscoveredPeer, DiscoveryStats, PeerCapabilities};
