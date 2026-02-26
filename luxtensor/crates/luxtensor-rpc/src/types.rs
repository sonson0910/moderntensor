use luxtensor_core::{Block, Transaction};
use serde::{Deserialize, Serialize};

/// Block number parameter - can be a number, "latest", "earliest", or "pending"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockNumber {
    Number(u64),
    Tag(String),
}

impl BlockNumber {
    pub fn latest() -> Self {
        BlockNumber::Tag("latest".to_string())
    }

    pub fn earliest() -> Self {
        BlockNumber::Tag("earliest".to_string())
    }
}

/// RPC Block representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcBlock {
    pub number: String,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    pub transactions: Vec<String>,
    pub state_root: String,
    pub gas_used: String,
    pub gas_limit: String,
}

impl From<Block> for RpcBlock {
    fn from(block: Block) -> Self {
        let hash = block.hash();
        RpcBlock {
            number: format!("0x{:x}", block.header.height),
            hash: format!("0x{}", hex::encode(hash)),
            parent_hash: format!("0x{}", hex::encode(block.header.previous_hash)),
            timestamp: format!("0x{:x}", block.header.timestamp),
            transactions: block
                .transactions
                .iter()
                .map(|tx| format!("0x{}", hex::encode(tx.hash())))
                .collect(),
            state_root: format!("0x{}", hex::encode(block.header.state_root)),
            gas_used: format!("0x{:x}", block.header.gas_used),
            gas_limit: format!("0x{:x}", block.header.gas_limit),
        }
    }
}

/// RPC Transaction representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcTransaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub nonce: String,
    pub gas_price: String,
    pub gas_limit: String,
    pub data: String,
}

impl From<Transaction> for RpcTransaction {
    fn from(tx: Transaction) -> Self {
        RpcTransaction {
            hash: format!("0x{}", hex::encode(tx.hash())),
            from: format!("0x{}", hex::encode(tx.from.as_bytes())),
            to: tx.to.map(|addr| format!("0x{}", hex::encode(addr.as_bytes()))),
            value: format!("0x{:x}", tx.value),
            nonce: format!("0x{:x}", tx.nonce),
            gas_price: format!("0x{:x}", tx.gas_price),
            gas_limit: format!("0x{:x}", tx.gas_limit),
            data: format!("0x{}", hex::encode(&tx.data)),
        }
    }
}

/// AI Task request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITaskRequest {
    pub model_hash: String,
    pub input_data: String,
    pub requester: String,
    pub reward: String,
}

/// AI Task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITaskResult {
    pub task_id: String,
    pub result_data: String,
    pub worker: String,
    pub status: String,
}

/// AI Task information (internal tracking)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITaskInfo {
    pub id: [u8; 32],
    pub model_hash: String,
    pub input_data: String,
    pub requester: String,
    pub reward: u128,
    pub status: AITaskStatus,
    pub result: Option<String>,
    pub worker: Option<String>,
    pub created_at: u64,
    pub completed_at: Option<u64>,
}

/// AI Task status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AITaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

/// Validator status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorStatus {
    pub address: String,
    pub stake: String,
    pub active: bool,
}

/// Subnet information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubnetInfo {
    pub id: u64,
    pub name: String,
    pub owner: String,
    pub emission_rate: u128,
    pub participant_count: usize,
    pub total_stake: u128,
    pub created_at: u64,
}

/// Neuron (miner/validator) information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NeuronInfo {
    pub uid: u64,
    pub subnet_id: u64,
    /// Hotkey address (hex-encoded)
    pub hotkey: String,
    /// Coldkey address (hex-encoded)
    pub coldkey: String,
    /// Legacy alias — same as `hotkey`, kept for backward compatibility
    #[serde(default)]
    pub address: String,
    pub stake: u128,
    pub trust: f64,
    pub consensus: f64,
    pub rank: u64,
    pub incentive: f64,
    pub dividends: f64,
    /// Emission (MDT units)
    pub emission: u128,
    /// Block height of last update
    pub last_update: u64,
    pub active: bool,
    pub endpoint: Option<String>,
}

/// Weight information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WeightInfo {
    pub neuron_uid: u64,
    pub weight: u32,
}
