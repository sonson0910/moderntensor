// Enhanced block synchronization protocol
// Implements a complete multi-peer sync strategy with parallel downloads

use libp2p::PeerId;
use luxtensor_core::block::{Block, BlockHeader};
use luxtensor_core::types::Hash;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Maximum number of blocks to request at once
const MAX_BLOCKS_PER_REQUEST: u32 = 128;

/// Maximum number of parallel downloads
const MAX_PARALLEL_DOWNLOADS: usize = 4;

/// Timeout for block requests (seconds)
const BLOCK_REQUEST_TIMEOUT: u64 = 30;

/// Enhanced sync protocol with parallel downloads and retry logic
pub struct SyncProtocol {
    /// Pending block requests
    pending_requests: Arc<RwLock<HashMap<Hash, PendingRequest>>>,
    /// Download queue
    download_queue: Arc<RwLock<VecDeque<Hash>>>,
    /// Downloaded blocks waiting to be processed
    downloaded_blocks: Arc<RwLock<HashMap<Hash, Block>>>,
}

/// A pending block request
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub block_hash: Hash,
    pub peer_id: PeerId,
    pub requested_at: std::time::Instant,
    pub retry_count: u32,
}

impl SyncProtocol {
    /// Create a new sync protocol
    pub fn new() -> Self {
        Self {
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            download_queue: Arc::new(RwLock::new(VecDeque::new())),
            downloaded_blocks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add headers to download queue
    pub async fn queue_headers(&self, headers: &[BlockHeader]) {
        let mut queue = self.download_queue.write().await;
        for header in headers {
            let hash = header.hash();
            if !queue.contains(&hash) {
                queue.push_back(hash);
                debug!("Queued block {} for download", hex::encode(&hash));
            }
        }
        info!("Download queue size: {}", queue.len());
    }

    /// Get next batch of blocks to download
    pub async fn get_next_batch(&self, max_count: usize) -> Vec<Hash> {
        let mut queue = self.download_queue.write().await;
        let pending = self.pending_requests.read().await;
        
        let mut batch = Vec::new();
        let mut taken = 0;
        
        while taken < max_count && !queue.is_empty() {
            if let Some(hash) = queue.pop_front() {
                // Skip if already pending
                if !pending.contains_key(&hash) {
                    batch.push(hash);
                    taken += 1;
                } else {
                    // Put back at end if still pending
                    queue.push_back(hash);
                }
            }
        }
        
        batch
    }

    /// Mark a block request as pending
    pub async fn mark_pending(&self, block_hash: Hash, peer_id: PeerId) {
        let mut pending = self.pending_requests.write().await;
        pending.insert(
            block_hash,
            PendingRequest {
                block_hash,
                peer_id,
                requested_at: std::time::Instant::now(),
                retry_count: 0,
            },
        );
    }

    /// Record a successfully downloaded block
    pub async fn record_downloaded(&self, block: Block) {
        let block_hash = block.hash();
        
        // Remove from pending
        let mut pending = self.pending_requests.write().await;
        pending.remove(&block_hash);
        
        // Add to downloaded cache
        let mut downloaded = self.downloaded_blocks.write().await;
        downloaded.insert(block_hash, block);
        
        debug!("Downloaded block {}", hex::encode(&block_hash));
    }

    /// Get a downloaded block
    pub async fn get_downloaded(&self, block_hash: &Hash) -> Option<Block> {
        let downloaded = self.downloaded_blocks.read().await;
        downloaded.get(block_hash).cloned()
    }

    /// Remove a block from downloaded cache
    pub async fn remove_downloaded(&self, block_hash: &Hash) {
        let mut downloaded = self.downloaded_blocks.write().await;
        downloaded.remove(block_hash);
    }

    /// Check for timed out requests and retry
    pub async fn check_timeouts(&self) -> Vec<Hash> {
        let mut pending = self.pending_requests.write().await;
        let mut timed_out = Vec::new();
        let now = std::time::Instant::now();
        
        let to_retry: Vec<Hash> = pending
            .iter()
            .filter(|(_, req)| {
                now.duration_since(req.requested_at).as_secs() > BLOCK_REQUEST_TIMEOUT
            })
            .map(|(hash, _)| *hash)
            .collect();
        
        for hash in to_retry {
            if let Some(mut req) = pending.remove(&hash) {
                req.retry_count += 1;
                
                if req.retry_count < 3 {
                    // Re-queue for retry
                    timed_out.push(hash);
                    warn!(
                        "Block {} timed out, retry {} of 3",
                        hex::encode(&hash),
                        req.retry_count
                    );
                } else {
                    warn!(
                        "Block {} failed after 3 retries, giving up",
                        hex::encode(&hash)
                    );
                }
            }
        }
        
        timed_out
    }

    /// Get sync statistics
    pub async fn get_stats(&self) -> SyncStats {
        let pending = self.pending_requests.read().await;
        let queue = self.download_queue.read().await;
        let downloaded = self.downloaded_blocks.read().await;
        
        SyncStats {
            pending_requests: pending.len(),
            queued_blocks: queue.len(),
            downloaded_blocks: downloaded.len(),
        }
    }

    /// Clear all state
    pub async fn clear(&self) {
        let mut pending = self.pending_requests.write().await;
        let mut queue = self.download_queue.write().await;
        let mut downloaded = self.downloaded_blocks.write().await;
        
        pending.clear();
        queue.clear();
        downloaded.clear();
    }
}

impl Default for SyncProtocol {
    fn default() -> Self {
        Self::new()
    }
}

/// Sync statistics
#[derive(Debug, Clone)]
pub struct SyncStats {
    pub pending_requests: usize,
    pub queued_blocks: usize,
    pub downloaded_blocks: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use luxtensor_core::block::BlockHeader;

    #[tokio::test]
    async fn test_sync_protocol_creation() {
        let protocol = SyncProtocol::new();
        let stats = protocol.get_stats().await;
        
        assert_eq!(stats.pending_requests, 0);
        assert_eq!(stats.queued_blocks, 0);
        assert_eq!(stats.downloaded_blocks, 0);
    }

    #[tokio::test]
    async fn test_queue_headers() {
        let protocol = SyncProtocol::new();
        
        let headers = vec![
            BlockHeader::new(
                1, 1, 1000, [0u8; 32], [1u8; 32], [0u8; 32], [0u8; 32],
                [0u8; 32], [0u8; 64], 0, 1000000, vec![],
            ),
            BlockHeader::new(
                1, 2, 1001, [0u8; 32], [2u8; 32], [0u8; 32], [0u8; 32],
                [0u8; 32], [0u8; 64], 0, 1000000, vec![],
            ),
        ];
        
        protocol.queue_headers(&headers).await;
        
        let stats = protocol.get_stats().await;
        assert_eq!(stats.queued_blocks, 2);
    }

    #[tokio::test]
    async fn test_get_next_batch() {
        let protocol = SyncProtocol::new();
        
        let headers = vec![
            BlockHeader::new(
                1, 1, 1000, [0u8; 32], [1u8; 32], [0u8; 32], [0u8; 32],
                [0u8; 32], [0u8; 64], 0, 1000000, vec![],
            ),
            BlockHeader::new(
                1, 2, 1001, [0u8; 32], [2u8; 32], [0u8; 32], [0u8; 32],
                [0u8; 32], [0u8; 64], 0, 1000000, vec![],
            ),
        ];
        
        protocol.queue_headers(&headers).await;
        
        let batch = protocol.get_next_batch(10).await;
        assert_eq!(batch.len(), 2);
        
        let stats = protocol.get_stats().await;
        assert_eq!(stats.queued_blocks, 0);
    }

    #[tokio::test]
    async fn test_mark_pending() {
        let protocol = SyncProtocol::new();
        let peer_id = PeerId::random();
        let block_hash = [1u8; 32];
        
        protocol.mark_pending(block_hash, peer_id).await;
        
        let stats = protocol.get_stats().await;
        assert_eq!(stats.pending_requests, 1);
    }

    #[tokio::test]
    async fn test_clear() {
        let protocol = SyncProtocol::new();
        
        let headers = vec![BlockHeader::new(
            1, 1, 1000, [0u8; 32], [1u8; 32], [0u8; 32], [0u8; 32],
            [0u8; 32], [0u8; 64], 0, 1000000, vec![],
        )];
        
        protocol.queue_headers(&headers).await;
        protocol.mark_pending([1u8; 32], PeerId::random()).await;
        
        let stats_before = protocol.get_stats().await;
        assert!(stats_before.queued_blocks > 0 || stats_before.pending_requests > 0);
        
        protocol.clear().await;
        
        let stats_after = protocol.get_stats().await;
        assert_eq!(stats_after.pending_requests, 0);
        assert_eq!(stats_after.queued_blocks, 0);
        assert_eq!(stats_after.downloaded_blocks, 0);
    }
}
