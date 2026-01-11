// LuxTensor RPC module
// Phase 5: JSON-RPC API and WebSocket implementation

pub mod broadcaster;
pub mod error;
pub mod server;
pub mod types;
pub mod websocket;
pub mod validation;

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
