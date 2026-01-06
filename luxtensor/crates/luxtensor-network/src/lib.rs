// LuxTensor network module
// Phase 3: Network Layer Implementation

pub mod error;
pub mod messages;
pub mod peer;
pub mod p2p;
pub mod sync;
pub mod sync_protocol;

pub use error::*;
pub use messages::{NetworkMessage, TOPIC_BLOCKS, TOPIC_TRANSACTIONS};
pub use peer::{PeerInfo, PeerManager};
pub use p2p::{P2PConfig, P2PEvent, P2PNode};
pub use sync::{SyncManager, SyncStatus};
pub use sync_protocol::{SyncProtocol, SyncStats};
