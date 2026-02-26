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
//! | `staking_` | [`handlers::staking`] | Validator staking (signature-protected) |
//! | `subnet_` | [`handlers::subnet`] | Subnet management (signature-protected) |
//! | `neuron_` | [`handlers::neuron`] | Neuron registration & queries |
//! | `weight_` | [`handlers::weight`] | Weight setting & queries |
//! | `ai_` | [`ai_rpc`] | AI model inference |
//! | `training_` | [`training_rpc`] | Distributed training |
//! | `zkml_` | [`zkml_rpc`] | Zero-knowledge ML proofs |
//! | `lux_` | [`metagraph_rpc`] | Metagraph state queries |
//! | `query_` | [`query_rpc`] | SDK query aliases |
//! | `tx_` | [`tx_rpc`] | Transaction handling |
//! | `agent_` | [`agent_rpc`] | Autonomous agent management |
//! | `bridge_` | [`bridge_rpc`] | Cross-chain bridge |
//! | `multisig_` | [`multisig_rpc`] | Multi-signature wallets |
//! | `dispute_` | [`dispute_rpc`] | Fraud proof disputes |
//! | `system_` | [`system_rpc`] | Node health & monitoring |
//! | `admin_` | [`admin_auth`] | Admin-only operations |
//! | `rpc_` | [`api_registry`] | API introspection |
//!
//! See [`api_registry::ALL_METHODS`] for the complete compile-time catalog.
//!

pub mod blockchain_rpc;
pub mod broadcaster;
pub mod error;
pub mod server;
pub mod system_rpc;
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
pub mod rpc_cache;
pub mod api_registry;


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
pub use eth_rpc::{register_eth_methods, register_log_methods, register_aa_methods, FaucetRpcConfig};
pub use logs::{LogStore, LogEntry, LogFilter, LogStoreStats};
pub use subnet_rpc::{register_subnet_methods, RootSubnet, new_root_subnet, RootSubnetState};
pub use query_rpc::{QueryRpcContext, register_query_methods};
pub use ai_rpc::{AiRpcContext, register_ai_methods};
pub use tx_rpc::{TxRpcContext, register_tx_methods};
pub use miner_dispatch_rpc::{MinerDispatchContext, register_miner_dispatch_methods};
pub use training_rpc::{TrainingRpcContext, register_training_methods};
pub use rpc_cache::RpcStateCache;
pub use api_registry::{RpcMethodInfo, register_list_methods};

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
