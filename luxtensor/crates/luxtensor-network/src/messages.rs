use luxtensor_core::block::{Block, BlockHeader};
use luxtensor_core::transaction::Transaction;
use luxtensor_core::types::Hash;
use serde::{Deserialize, Serialize};

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// New transaction announcement
    NewTransaction(Transaction),

    /// New block announcement
    NewBlock(Block),

    /// Request block by hash
    GetBlock(Hash),

    /// Response with requested block
    Block(Block),

    /// Request block headers starting from hash
    GetBlockHeaders {
        start_hash: Hash,
        max_count: u32,
    },

    /// Response with block headers
    BlockHeaders(Vec<BlockHeader>),

    /// Request blocks by hashes
    GetBlocks(Vec<Hash>),

    /// Response with blocks
    Blocks(Vec<Block>),

    /// Status message with chain info
    Status {
        best_hash: Hash,
        best_height: u64,
        genesis_hash: Hash,
    },

    /// Sync request - asking for blocks from a range
    SyncRequest {
        from_height: u64,
        to_height: u64,
        requester_id: String,
    },

    /// Ping message
    Ping,

    /// Pong response
    Pong,
}

impl NetworkMessage {
    /// Get the message type as a string
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::NewTransaction(_) => "NewTransaction",
            Self::NewBlock(_) => "NewBlock",
            Self::GetBlock(_) => "GetBlock",
            Self::Block(_) => "Block",
            Self::GetBlockHeaders { .. } => "GetBlockHeaders",
            Self::BlockHeaders(_) => "BlockHeaders",
            Self::GetBlocks(_) => "GetBlocks",
            Self::Blocks(_) => "Blocks",
            Self::Status { .. } => "Status",
            Self::SyncRequest { .. } => "SyncRequest",
            Self::Ping => "Ping",
            Self::Pong => "Pong",
        }
    }
}

/// Topics for gossipsub
pub const TOPIC_BLOCKS: &str = "luxtensor/blocks/1.0.0";
pub const TOPIC_TRANSACTIONS: &str = "luxtensor/transactions/1.0.0";
pub const TOPIC_SYNC: &str = "luxtensor/sync/1.0.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = NetworkMessage::Ping;
        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            NetworkMessage::Ping => {},
            _ => panic!("Expected Ping message"),
        }
    }

    #[test]
    fn test_status_message() {
        let msg = NetworkMessage::Status {
            best_hash: [1u8; 32],
            best_height: 100,
            genesis_hash: [0u8; 32],
        };

        assert_eq!(msg.message_type(), "Status");

        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            NetworkMessage::Status { best_height, .. } => {
                assert_eq!(best_height, 100);
            },
            _ => panic!("Expected Status message"),
        }
    }

    #[test]
    fn test_get_block_headers() {
        let msg = NetworkMessage::GetBlockHeaders {
            start_hash: [1u8; 32],
            max_count: 10,
        };

        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            NetworkMessage::GetBlockHeaders { max_count, .. } => {
                assert_eq!(max_count, 10);
            },
            _ => panic!("Expected GetBlockHeaders message"),
        }
    }
}
