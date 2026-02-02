//! Database models for indexed data

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Indexed block
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Block {
    pub number: i64,
    pub hash: String,
    pub parent_hash: Option<String>,
    pub timestamp: i64,
    pub tx_count: i32,
    #[sqlx(default)]
    pub indexed_at: Option<DateTime<Utc>>,
}

/// Indexed transaction
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub hash: String,
    pub block_number: i64,
    pub chain_id: i64,  // Chain ID for replay protection
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: String, // Stored as string for large numbers
    pub gas_used: i64,
    pub status: i16,
    pub tx_type: String,
    #[sqlx(default)]
    pub indexed_at: Option<DateTime<Utc>>,
}

/// Token transfer event
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TokenTransfer {
    pub id: i64,
    pub tx_hash: String,
    pub block_number: i64,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub timestamp: i64,
}

/// Stake event
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct StakeEvent {
    pub id: i64,
    pub block_number: i64,
    pub coldkey: String,
    pub hotkey: String,
    pub amount: String,
    pub action: String, // "stake" or "unstake"
    pub timestamp: i64,
}

/// Neuron snapshot
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NeuronSnapshot {
    pub id: i64,
    pub block_number: i64,
    pub subnet_id: i64,
    pub uid: i64,
    pub hotkey: String,
    pub coldkey: String,
    pub stake: String,
    pub trust: f64,
    pub consensus: f64,
    pub incentive: f64,
    pub dividends: f64,
    pub emission: String,
    pub timestamp: i64,
}

/// Weight commit
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WeightCommit {
    pub id: i64,
    pub block_number: i64,
    pub subnet_id: i64,
    pub validator_uid: i64,
    pub weights_hash: String,
    pub timestamp: i64,
}

/// Aggregated metrics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyMetrics {
    pub date: chrono::NaiveDate,
    pub total_transactions: i64,
    pub total_transfers: i64,
    pub total_stake_amount: String,
    pub active_validators: i32,
    pub active_miners: i32,
}

/// Indexer sync status
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncStatus {
    pub id: i32,
    pub last_indexed_block: i64,
    #[sqlx(default)]
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub is_syncing: bool,
}
