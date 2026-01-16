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
pub mod logs;
pub mod subnet_rpc;
pub mod helpers;
pub mod handlers;

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
pub use eth_rpc::{register_eth_methods, register_log_methods, register_aa_methods, EvmState, ReadyTransaction};
pub use logs::{LogStore, LogEntry, LogFilter, LogStoreStats};
pub use subnet_rpc::{register_subnet_methods, RootSubnet, new_root_subnet, RootSubnetState};
