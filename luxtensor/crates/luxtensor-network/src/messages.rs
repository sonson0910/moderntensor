use bincode::Options;
use luxtensor_core::block::{Block, BlockHeader};
use luxtensor_core::transaction::Transaction;
use luxtensor_core::types::Hash;
use serde::{Deserialize, Serialize};

/// Maximum allowed size for a single deserialized network message (4 MB)
/// 🔧 FIX: Aligned with gossipsub max_transmit_size (4 MB) to avoid
/// accepting messages at deserialization layer that gossipsub already rejects.
pub const MAX_MESSAGE_SIZE: u64 = 4 * 1024 * 1024;

/// 🔧 FIX F5: Protocol-level caps for request messages to prevent DoS.
/// A single `GetBlockHeaders { max_count: u32::MAX }` would trigger billions
/// of database lookups on the responder.
pub const MAX_HEADERS_REQUEST: u32 = 1024;
/// Maximum number of block hashes in a GetBlocks request.
pub const MAX_BLOCKS_REQUEST: usize = 256;
/// 🔧 FIX F18: Maximum ECDSA signature length (DER-encoded + buffer).
pub const MAX_SIGNATURE_LEN: usize = 73;

/// Serialize a NetworkMessage using fixint encoding.
///
/// IMPORTANT: Must use the same options as `deserialize_message()` to ensure
/// encoding compatibility. Using mismatched options (e.g., varint vs fixint)
/// causes "unexpected end of file" deserialization errors on the receiving side.
pub fn serialize_message(msg: &NetworkMessage) -> Result<Vec<u8>, bincode::Error> {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize(msg)
}

/// Deserialize a NetworkMessage with a size limit to prevent DoS attacks.
/// Returns an error if the data exceeds MAX_MESSAGE_SIZE.
///
/// SECURITY: Does NOT allow trailing bytes — valid messages must be
/// exactly represented. Trailing bytes could be used to bypass size
/// limits or cause message ID collisions.
pub fn deserialize_message(data: &[u8]) -> Result<NetworkMessage, bincode::Error> {
    let msg: NetworkMessage = bincode::DefaultOptions::new()
        .with_limit(MAX_MESSAGE_SIZE)
        .with_fixint_encoding()
        .deserialize(data)?;

    // 🔧 FIX F5 + F18: Validate request sizes after deserialization
    match &msg {
        NetworkMessage::GetBlockHeaders { max_count, .. } if *max_count > MAX_HEADERS_REQUEST => {
            return Err(bincode::ErrorKind::Custom(
                format!("GetBlockHeaders max_count {} exceeds limit {}", max_count, MAX_HEADERS_REQUEST),
            ).into());
        }
        NetworkMessage::GetBlocks(hashes) if hashes.len() > MAX_BLOCKS_REQUEST => {
            return Err(bincode::ErrorKind::Custom(
                format!("GetBlocks hash count {} exceeds limit {}", hashes.len(), MAX_BLOCKS_REQUEST),
            ).into());
        }
        NetworkMessage::AITaskDispatch { validator_signature, .. }
            if validator_signature.len() > MAX_SIGNATURE_LEN =>
        {
            return Err(bincode::ErrorKind::Custom(
                format!("AITaskDispatch signature {} bytes exceeds limit {}", validator_signature.len(), MAX_SIGNATURE_LEN),
            ).into());
        }
        _ => {}
    }

    Ok(msg)
}

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
    GetBlockHeaders { start_hash: Hash, max_count: u32 },

    /// Response with block headers
    BlockHeaders(Vec<BlockHeader>),

    /// Request blocks by hashes
    GetBlocks(Vec<Hash>),

    /// Response with blocks
    /// `nonce` breaks gossipsub dedup for repeated sync responses with identical blocks
    Blocks { blocks: Vec<Block>, nonce: u64 },

    /// Status message with chain info
    Status { best_hash: Hash, best_height: u64, genesis_hash: Hash },

    /// Sync request - asking for blocks from a range
    /// `nonce` breaks gossipsub message_id deduplication for repeated requests
    SyncRequest { from_height: u64, to_height: u64, requester_id: String, nonce: u64 },

    /// Ping message
    Ping,

    /// Pong response
    Pong,

    /// AI task dispatch to miners
    ///
    /// SECURITY: `validator_signature` authenticates the task dispatcher to prevent
    /// forged tasks with arbitrary rewards. The signature covers
    /// `(task_id || model_hash || input_hash || reward || deadline)`.
    /// Receivers MUST verify the signature before processing (H3 fix).
    AITaskDispatch {
        task_id: [u8; 32],
        model_hash: String,
        input_hash: [u8; 32],
        reward: u128,
        deadline: u64,
        /// ECDSA signature over the task fields, proving authority
        validator_signature: Vec<u8>,
        /// 20-byte address of the signing validator
        validator_address: [u8; 20],
    },
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
            Self::Blocks { .. } => "Blocks",
            Self::Status { .. } => "Status",
            Self::SyncRequest { .. } => "SyncRequest",
            Self::Ping => "Ping",
            Self::Pong => "Pong",
            Self::AITaskDispatch { .. } => "AITaskDispatch",
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
            NetworkMessage::Ping => {}
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
            }
            _ => panic!("Expected Status message"),
        }
    }

    #[test]
    fn test_get_block_headers() {
        let msg = NetworkMessage::GetBlockHeaders { start_hash: [1u8; 32], max_count: 10 };

        let serialized = bincode::serialize(&msg).unwrap();
        let deserialized: NetworkMessage = bincode::deserialize(&serialized).unwrap();

        match deserialized {
            NetworkMessage::GetBlockHeaders { max_count, .. } => {
                assert_eq!(max_count, 10);
            }
            _ => panic!("Expected GetBlockHeaders message"),
        }
    }
}
