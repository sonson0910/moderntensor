// LuxTensor network module
// Phase 3: Network Layer Implementation

pub mod error;
pub mod messages;
pub mod peer;
pub mod p2p;
pub mod sync;
pub mod sync_protocol;
pub mod rate_limiter;
pub mod swarm;

pub use error::*;
pub use messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_TRANSACTIONS, TOPIC_SYNC};
pub use peer::{PeerInfo, PeerManager};
pub use p2p::{P2PConfig, P2PEvent, P2PNode, GossipTopics, GossipStats};
pub use libp2p::Multiaddr;
pub use sync::{SyncManager, SyncStatus};
pub use sync_protocol::{SyncProtocol, SyncStats};
pub use rate_limiter::{RateLimiter, RateLimiterConfig, RateLimiterStats};
pub use swarm::{SwarmP2PNode, SwarmP2PEvent, SwarmCommand};


