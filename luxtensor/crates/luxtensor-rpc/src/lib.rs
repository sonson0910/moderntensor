// LuxTensor RPC module
// Phase 5: JSON-RPC API and WebSocket implementation

pub mod broadcaster;
pub mod error;
pub mod server;
pub mod types;
pub mod websocket;
pub mod validation;
pub mod rewards_rpc;
pub mod allocation_rpc;
pub mod node_rpc;
pub mod eth_rpc;

pub use broadcaster::{
    TransactionBroadcaster, BroadcastError,
    NoOpBroadcaster, ChannelBroadcaster, CompositeBroadcaster,
    BroadcasterBuilder,
};
pub use error::*;
pub use server::RpcServer;
pub use types::*;
pub use websocket::{WebSocketServer, BroadcastEvent, SubscriptionType};
pub use validation::{RpcLimits, ValidationError, validate_address, validate_hash, validate_hex_data};
pub use rewards_rpc::register_reward_methods;
pub use allocation_rpc::register_allocation_methods;
pub use node_rpc::register_node_methods;
pub use eth_rpc::{register_eth_methods, EvmState};

