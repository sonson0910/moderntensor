// Event Log Storage and Querying for eth_getLogs
// Phase 2: DeFi Compatibility

use luxtensor_core::types::{Address, Hash};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// An event log entry from contract execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Address of the contract that emitted the log
    pub address: Address,
    /// Indexed topics (max 4: event signature + up to 3 indexed params)
    pub topics: Vec<Hash>,
    /// Non-indexed data
    pub data: Vec<u8>,
    /// Block number where log was emitted
    pub block_number: u64,
    /// Block hash
    pub block_hash: Hash,
    /// Transaction hash
    pub transaction_hash: Hash,
    /// Transaction index in block
    pub transaction_index: u32,
    /// Log index in block
    pub log_index: u32,
    /// Whether the log was removed due to reorg
    pub removed: bool,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(
        address: Address,
        topics: Vec<Hash>,
        data: Vec<u8>,
        block_number: u64,
        block_hash: Hash,
        transaction_hash: Hash,
        transaction_index: u32,
        log_index: u32,
    ) -> Self {
        Self {
            address,
            topics,
            data,
            block_number,
            block_hash,
            transaction_hash,
            transaction_index,
            log_index,
            removed: false,
        }
    }

    /// Convert to JSON-RPC format
    pub fn to_rpc_log(&self) -> serde_json::Value {
        serde_json::json!({
            "address": format!("0x{}", hex::encode(self.address.as_bytes())),
            "topics": self.topics.iter()
                .map(|t| format!("0x{}", hex::encode(t)))
                .collect::<Vec<_>>(),
            "data": format!("0x{}", hex::encode(&self.data)),
            "blockNumber": format!("0x{:x}", self.block_number),
            "blockHash": format!("0x{}", hex::encode(self.block_hash)),
            "transactionHash": format!("0x{}", hex::encode(self.transaction_hash)),
            "transactionIndex": format!("0x{:x}", self.transaction_index),
            "logIndex": format!("0x{:x}", self.log_index),
            "removed": self.removed,
        })
    }
}

/// Filter for querying logs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFilter {
    /// Start block (None = earliest)
    pub from_block: Option<u64>,
    /// End block (None = latest)
    pub to_block: Option<u64>,
    /// Contract addresses to filter (None = all addresses)
    pub address: Option<Vec<Address>>,
    /// Topics to filter. Each position can have multiple options (OR).
    /// Vec<Option<Vec<Hash>>> means:
    /// - Outer Vec: position 0-3 (event sig, indexed params)
    /// - Option: None means any topic at this position
    /// - Inner Vec: multiple options (OR) at this position
    pub topics: Option<Vec<Option<Vec<Hash>>>>,
    /// Block hash (if set, from_block and to_block are ignored)
    pub block_hash: Option<Hash>,
}

impl LogFilter {
    /// Check if a log matches this filter
    pub fn matches(&self, log: &LogEntry) -> bool {
        // Check block range
        if let Some(from) = self.from_block {
            if log.block_number < from {
                return false;
            }
        }
        if let Some(to) = self.to_block {
            if log.block_number > to {
                return false;
            }
        }

        // Check block hash
        if let Some(ref block_hash) = self.block_hash {
            if &log.block_hash != block_hash {
                return false;
            }
        }

        // Check address
        if let Some(ref addresses) = self.address {
            if !addresses.iter().any(|a| a == &log.address) {
                return false;
            }
        }

        // Check topics
        if let Some(ref topic_filters) = self.topics {
            for (i, topic_filter) in topic_filters.iter().enumerate() {
                if let Some(ref options) = topic_filter {
                    // This position has a filter
                    if i >= log.topics.len() {
                        // Log doesn't have this topic position
                        return false;
                    }
                    if !options.iter().any(|t| t == &log.topics[i]) {
                        return false;
                    }
                }
                // None means any topic at this position (matches)
            }
        }

        true
    }
}

/// Registered filter with ID
#[derive(Debug, Clone)]
pub struct RegisteredFilter {
    pub id: String,
    pub filter: LogFilter,
    pub last_block: u64,
    pub created_at: std::time::Instant,
}

/// Log storage and indexing
pub struct LogStore {
    /// All logs indexed by block number
    logs_by_block: Arc<RwLock<HashMap<u64, Vec<LogEntry>>>>,
    /// Index: address -> block numbers containing logs
    address_index: Arc<RwLock<HashMap<Address, Vec<u64>>>>,
    /// Index: topic[0] (event signature) -> block numbers
    topic0_index: Arc<RwLock<HashMap<Hash, Vec<u64>>>>,
    /// Registered filters
    filters: Arc<RwLock<HashMap<String, RegisteredFilter>>>,
    /// Filter ID counter
    filter_counter: Arc<RwLock<u64>>,
    /// Maximum blocks to keep logs for
    max_blocks: u64,
}

impl LogStore {
    /// Create a new log store
    pub fn new(max_blocks: u64) -> Self {
        Self {
            logs_by_block: Arc::new(RwLock::new(HashMap::new())),
            address_index: Arc::new(RwLock::new(HashMap::new())),
            topic0_index: Arc::new(RwLock::new(HashMap::new())),
            filters: Arc::new(RwLock::new(HashMap::new())),
            filter_counter: Arc::new(RwLock::new(0)),
            max_blocks,
        }
    }

    /// Add logs from a block
    pub fn add_logs(&self, block_number: u64, logs: Vec<LogEntry>) {
        if logs.is_empty() {
            return;
        }

        // Update address index
        {
            let mut addr_idx = self.address_index.write();
            for log in &logs {
                addr_idx
                    .entry(log.address)
                    .or_insert_with(Vec::new)
                    .push(block_number);
            }
        }

        // Update topic0 index
        {
            let mut topic_idx = self.topic0_index.write();
            for log in &logs {
                if let Some(topic0) = log.topics.first() {
                    topic_idx
                        .entry(*topic0)
                        .or_insert_with(Vec::new)
                        .push(block_number);
                }
            }
        }

        // Store logs
        {
            let mut logs_by_block = self.logs_by_block.write();
            logs_by_block.insert(block_number, logs.clone());
        }

        debug!("Added {} logs for block {}", logs.len(), block_number);

        // Prune old blocks
        self.prune_old_blocks(block_number);
    }

    /// Remove logs older than max_blocks
    fn prune_old_blocks(&self, current_block: u64) {
        if current_block <= self.max_blocks {
            return;
        }

        let cutoff = current_block - self.max_blocks;

        let mut logs_by_block = self.logs_by_block.write();
        let blocks_to_remove: Vec<_> = logs_by_block
            .keys()
            .filter(|&&b| b < cutoff)
            .copied()
            .collect();

        for block in blocks_to_remove {
            logs_by_block.remove(&block);
        }
    }

    /// Query logs with a filter
    pub fn get_logs(&self, filter: &LogFilter, current_block: u64) -> Vec<LogEntry> {
        let from = filter.from_block.unwrap_or(0);
        let to = filter.to_block.unwrap_or(current_block);

        let logs_by_block = self.logs_by_block.read();
        let mut result = Vec::new();

        // Determine which blocks to scan
        let blocks_to_scan: Vec<u64> = if let Some(ref addresses) = filter.address {
            // Use address index for optimization
            let addr_idx = self.address_index.read();
            let mut blocks = std::collections::HashSet::new();
            for addr in addresses {
                if let Some(addr_blocks) = addr_idx.get(addr) {
                    for &b in addr_blocks {
                        if b >= from && b <= to {
                            blocks.insert(b);
                        }
                    }
                }
            }
            blocks.into_iter().collect()
        } else if let Some(ref topics) = filter.topics {
            // Use topic0 index if available
            if let Some(Some(ref topic0_options)) = topics.first() {
                let topic_idx = self.topic0_index.read();
                let mut blocks = std::collections::HashSet::new();
                for topic0 in topic0_options {
                    if let Some(topic_blocks) = topic_idx.get(topic0) {
                        for &b in topic_blocks {
                            if b >= from && b <= to {
                                blocks.insert(b);
                            }
                        }
                    }
                }
                blocks.into_iter().collect()
            } else {
                // Scan all blocks in range
                (from..=to).collect()
            }
        } else {
            // No filter optimization, scan all blocks
            (from..=to).collect()
        };

        // Collect matching logs
        for block_num in blocks_to_scan {
            if let Some(block_logs) = logs_by_block.get(&block_num) {
                for log in block_logs {
                    if filter.matches(log) {
                        result.push(log.clone());
                    }
                }
            }
        }

        // Sort by block number, then log index
        result.sort_by(|a, b| {
            a.block_number
                .cmp(&b.block_number)
                .then(a.log_index.cmp(&b.log_index))
        });

        result
    }

    /// Create a new filter and return its ID
    pub fn new_filter(&self, filter: LogFilter, current_block: u64) -> String {
        let mut counter = self.filter_counter.write();
        *counter += 1;
        let id = format!("0x{:x}", *counter);

        let registered = RegisteredFilter {
            id: id.clone(),
            filter,
            last_block: current_block,
            created_at: std::time::Instant::now(),
        };

        self.filters.write().insert(id.clone(), registered);
        id
    }

    /// Get changes for a filter since last poll
    pub fn get_filter_changes(&self, filter_id: &str, current_block: u64) -> Option<Vec<LogEntry>> {
        let mut filters = self.filters.write();
        let filter = filters.get_mut(filter_id)?;

        let from = filter.last_block + 1;
        let to = current_block;

        if from > to {
            return Some(vec![]);
        }

        let mut query_filter = filter.filter.clone();
        query_filter.from_block = Some(from);
        query_filter.to_block = Some(to);

        filter.last_block = current_block;

        drop(filters);
        Some(self.get_logs(&query_filter, current_block))
    }

    /// Remove a filter
    pub fn uninstall_filter(&self, filter_id: &str) -> bool {
        self.filters.write().remove(filter_id).is_some()
    }

    /// Get stats
    pub fn stats(&self) -> LogStoreStats {
        let logs_by_block = self.logs_by_block.read();
        let total_logs: usize = logs_by_block.values().map(|v| v.len()).sum();

        LogStoreStats {
            total_blocks: logs_by_block.len(),
            total_logs,
            active_filters: self.filters.read().len(),
        }
    }
}

impl Default for LogStore {
    fn default() -> Self {
        Self::new(10000) // Keep logs for 10000 blocks by default
    }
}

/// Log store statistics
#[derive(Debug, Clone)]
pub struct LogStoreStats {
    pub total_blocks: usize,
    pub total_logs: usize,
    pub active_filters: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_log(block: u64, log_idx: u32) -> LogEntry {
        LogEntry::new(
            Address::from([1u8; 20]),
            vec![[2u8; 32]], // topic0
            vec![3u8; 32],   // data
            block,
            [4u8; 32],       // block_hash
            [5u8; 32],       // tx_hash
            0,               // tx_index
            log_idx,
        )
    }

    #[test]
    fn test_log_store_creation() {
        let store = LogStore::new(100);
        let stats = store.stats();
        assert_eq!(stats.total_blocks, 0);
        assert_eq!(stats.total_logs, 0);
    }

    #[test]
    fn test_add_and_query_logs() {
        let store = LogStore::new(100);

        // Add logs to block 1
        store.add_logs(1, vec![
            create_test_log(1, 0),
            create_test_log(1, 1),
        ]);

        // Add logs to block 2
        store.add_logs(2, vec![
            create_test_log(2, 0),
        ]);

        // Query all logs
        let filter = LogFilter::default();
        let logs = store.get_logs(&filter, 10);
        assert_eq!(logs.len(), 3);
    }

    #[test]
    fn test_filter_by_block_range() {
        let store = LogStore::new(100);

        store.add_logs(1, vec![create_test_log(1, 0)]);
        store.add_logs(5, vec![create_test_log(5, 0)]);
        store.add_logs(10, vec![create_test_log(10, 0)]);

        let filter = LogFilter {
            from_block: Some(3),
            to_block: Some(7),
            ..Default::default()
        };

        let logs = store.get_logs(&filter, 10);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].block_number, 5);
    }

    #[test]
    fn test_filter_by_address() {
        let store = LogStore::new(100);

        let addr1 = Address::from([1u8; 20]);
        let addr2 = Address::from([2u8; 20]);

        let mut log1 = create_test_log(1, 0);
        log1.address = addr1;

        let mut log2 = create_test_log(1, 1);
        log2.address = addr2;

        store.add_logs(1, vec![log1, log2]);

        let filter = LogFilter {
            address: Some(vec![addr1]),
            ..Default::default()
        };

        let logs = store.get_logs(&filter, 10);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].address, addr1);
    }

    #[test]
    fn test_new_filter() {
        let store = LogStore::new(100);

        let filter = LogFilter::default();
        let id = store.new_filter(filter, 100);

        assert!(id.starts_with("0x"));
        assert_eq!(store.stats().active_filters, 1);
    }

    #[test]
    fn test_uninstall_filter() {
        let store = LogStore::new(100);

        let filter = LogFilter::default();
        let id = store.new_filter(filter, 100);

        assert!(store.uninstall_filter(&id));
        assert!(!store.uninstall_filter(&id)); // Already removed
        assert_eq!(store.stats().active_filters, 0);
    }

    #[test]
    fn test_to_rpc_log() {
        let log = create_test_log(1, 0);
        let rpc_log = log.to_rpc_log();

        assert!(rpc_log["address"].as_str().unwrap().starts_with("0x"));
        assert!(rpc_log["blockNumber"].as_str().unwrap().starts_with("0x"));
        assert_eq!(rpc_log["removed"], false);
    }
}
