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

pub mod allocation_rpc;
pub mod blockchain_rpc;
pub mod broadcaster;
pub mod error;
pub mod eth_rpc;
pub mod handlers;
pub mod helpers;
pub mod logs;
pub mod node_rpc;
pub mod peer_count;
pub mod rewards_rpc;
pub mod server;
pub mod staking_rpc;
pub mod subnet_rpc;
pub mod system_rpc;
pub mod types;
pub mod validation;
pub mod websocket;
// contract_registry module DELETED - bytecode now stored in Account.code
pub mod admin_auth;
pub mod ai_rpc;
pub mod api_registry;
pub mod load_balancer;
pub mod miner_dispatch_rpc;
pub mod query_rpc;
pub mod rate_limiter;
pub mod rpc_cache;
pub mod training_rpc;
pub mod tx_rpc;

pub use admin_auth::{check_admin_auth, requires_admin_auth, AdminAuth, AdminAuthConfig};
pub use ai_rpc::{register_ai_methods, AiRpcContext};
pub use allocation_rpc::register_allocation_methods;
pub use api_registry::{register_list_methods, RpcMethodInfo};
pub use broadcaster::{
    BroadcastError, BroadcasterBuilder, ChannelBroadcaster, CompositeBroadcaster, NoOpBroadcaster,
    TransactionBroadcaster,
};
pub use error::*;
pub use eth_rpc::{
    register_aa_methods, register_eth_methods, register_log_methods, FaucetRpcConfig,
};
pub use load_balancer::{
    LoadBalancerConfig, LoadBalancerStats, NodeEndpoint, NodeHealth, RpcLoadBalancer,
    SmartRpcClient,
};
pub use logs::{LogEntry, LogFilter, LogStore, LogStoreStats};
pub use miner_dispatch_rpc::{register_miner_dispatch_methods, MinerDispatchContext};
pub use node_rpc::register_node_methods;
pub use query_rpc::{register_query_methods, QueryRpcContext};
pub use rate_limiter::{RateLimiter, RateLimiterConfig};
pub use rewards_rpc::register_reward_methods;
pub use rpc_cache::RpcStateCache;
pub use server::RpcServer;
pub use subnet_rpc::{new_root_subnet, register_subnet_methods, RootSubnet, RootSubnetState};
pub use training_rpc::{register_training_methods, TrainingRpcContext};
pub use tx_rpc::{register_tx_methods, TxRpcContext};
pub use types::*;
pub use validation::{
    validate_address, validate_hash, validate_hex_data, RpcLimits, ValidationError,
};
pub use websocket::{BroadcastEvent, SubscriptionType, WebSocketServer};

pub mod zkml_rpc;
pub use zkml_rpc::{register_zkml_methods, ZkmlRpcContext};

pub mod agent_rpc;
pub use agent_rpc::{register_agent_methods, AgentRpcContext};

pub mod dispute_rpc;
pub use dispute_rpc::{register_dispute_methods, DisputeRpcContext};

pub mod bridge_rpc;
pub use bridge_rpc::{register_bridge_methods, BridgeRpcContext};

pub mod multisig_rpc;
pub use multisig_rpc::{register_multisig_methods, MultisigRpcContext};
