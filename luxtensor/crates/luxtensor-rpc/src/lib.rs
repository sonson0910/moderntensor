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
pub mod staking_rpc;
pub mod peer_count;
pub mod contract_registry;
pub mod rate_limiter;
pub mod admin_auth;
pub mod load_balancer;
pub mod query_rpc;
pub mod ai_rpc;
pub mod tx_rpc;
pub mod miner_dispatch_rpc;


pub use rate_limiter::{RateLimiter, RateLimiterConfig};
pub use admin_auth::{AdminAuth, AdminAuthConfig, requires_admin_auth};
pub use load_balancer::{
    RpcLoadBalancer, LoadBalancerConfig, LoadBalancerStats,
    NodeEndpoint, NodeHealth, SmartRpcClient,
};
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
pub use query_rpc::{QueryRpcContext, register_query_methods};
pub use ai_rpc::{AiRpcContext, register_ai_methods};
pub use tx_rpc::{TxRpcContext, register_tx_methods};
pub use miner_dispatch_rpc::{MinerDispatchContext, register_miner_dispatch_methods};

pub mod zkml_rpc;
pub use zkml_rpc::{ZkmlRpcContext, register_zkml_methods};


