//! # LuxTensor RPC Crate
//!
//! JSON-RPC 2.0 API server for the LuxTensor blockchain.
//!
//! ## Features
//!
//! - **Ethereum-compatible RPC** (`eth_*` methods)
//! - **Staking operations** (`staking_*` methods)
//! - **AI/ML inference** (`ai_*`, `training_*` methods)
//! - **Zero-knowledge proofs** (`zkml_*` methods)
//! - **WebSocket subscriptions** for real-time events
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use luxtensor_rpc::RpcServer;
//!
//! let server = RpcServer::new_with_shared_pending_txs(db, mempool, broadcaster, pending_txs, chain_id);
//! server.start("127.0.0.1:8545").await?;
//! ```
//!
//! ## RPC Method Categories
//!
//! | Prefix | Module | Description |
//! |--------|--------|-------------|
//! | `eth_` | [`eth_rpc`] | Ethereum JSON-RPC compatible |
//! | `staking_` | [`staking_rpc`] | Validator staking & delegation |
//! | `ai_` | [`ai_rpc`] | AI model inference |
//! | `training_` | [`training_rpc`] | Distributed training |
//! | `zkml_` | [`zkml_rpc`] | Zero-knowledge ML proofs |
//! | `subnet_` | [`subnet_rpc`] | Subnet management |
//! | `tx_` | [`tx_rpc`] | Transaction handling |
//!

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
// contract_registry module DELETED - bytecode now stored in Account.code
pub mod rate_limiter;
pub mod admin_auth;
pub mod load_balancer;
pub mod query_rpc;
pub mod ai_rpc;
pub mod tx_rpc;
pub mod miner_dispatch_rpc;
pub mod training_rpc;


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
pub use eth_rpc::{register_eth_methods, register_log_methods, register_aa_methods, Mempool, ReadyTransaction};
pub use logs::{LogStore, LogEntry, LogFilter, LogStoreStats};
pub use subnet_rpc::{register_subnet_methods, RootSubnet, new_root_subnet, RootSubnetState};
pub use query_rpc::{QueryRpcContext, register_query_methods};
pub use ai_rpc::{AiRpcContext, register_ai_methods};
pub use tx_rpc::{TxRpcContext, register_tx_methods};
pub use miner_dispatch_rpc::{MinerDispatchContext, register_miner_dispatch_methods};
pub use training_rpc::{TrainingRpcContext, register_training_methods};

pub mod zkml_rpc;
pub use zkml_rpc::{ZkmlRpcContext, register_zkml_methods};

pub mod agent_rpc;
pub use agent_rpc::{AgentRpcContext, register_agent_methods};

pub mod dispute_rpc;
pub use dispute_rpc::{DisputeRpcContext, register_dispute_methods};

pub mod bridge_rpc;
pub use bridge_rpc::{BridgeRpcContext, register_bridge_methods};

pub mod multisig_rpc;
pub use multisig_rpc::{MultisigRpcContext, register_multisig_methods};
