use crate::error::NetworkError;
use crate::peer::PeerManager;
use libp2p::PeerId;
use luxtensor_core::block::{Block, BlockHeader};
use luxtensor_core::types::Hash;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

/// Maximum number of headers to request per batch
const MAX_HEADERS_PER_REQUEST: u32 = 256;

/// Maximum number of consecutive sync failures before aborting
const MAX_SYNC_FAILURES: u32 = 5;

/// Sync manager for blockchain synchronization
pub struct SyncManager {
    peer_manager: Arc<RwLock<PeerManager>>,
    syncing: Arc<RwLock<bool>>,
    /// Callback for sending block request messages via P2P
    /// In production: wired to the libp2p request-response protocol.
    /// Must be set via `set_block_requester` before `start_sync` is called.
    block_requester: Arc<RwLock<Option<Box<dyn BlockRequester>>>>,
}

/// Trait for requesting blocks from peers over the P2P network.
///
/// Implementations must send a request-response message via libp2p
/// and return the result. The `SyncManager` calls these methods during
/// chain synchronization.
#[async_trait::async_trait]
pub trait BlockRequester: Send + Sync {
    /// Request a single block by hash from a specific peer.
    async fn request_block(
        &self,
        peer_id: &PeerId,
        block_hash: Hash,
    ) -> Result<Block, NetworkError>;

    /// Request a batch of block headers starting from `start_height`.
    /// Returns up to `max_count` sequential headers.
    async fn request_headers(
        &self,
        peer_id: &PeerId,
        start_height: u64,
        max_count: u32,
    ) -> Result<Vec<BlockHeader>, NetworkError>;

    /// Request a full block by height from a specific peer.
    async fn request_block_by_height(
        &self,
        peer_id: &PeerId,
        height: u64,
    ) -> Result<Block, NetworkError>;
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(peer_manager: Arc<RwLock<PeerManager>>) -> Self {
        Self {
            peer_manager,
            syncing: Arc::new(RwLock::new(false)),
            block_requester: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the block requester (must be called before `start_sync`).
    /// This wires the sync manager to the actual P2P request-response protocol.
    pub async fn set_block_requester(&self, requester: Box<dyn BlockRequester>) {
        *self.block_requester.write().await = Some(requester);
    }

    /// Check if currently syncing
    pub async fn is_syncing(&self) -> bool {
        *self.syncing.read().await
    }

    /// Start syncing from best peer.
    ///
    /// Algorithm:
    /// 1. Find the best peer (highest advertised height).
    /// 2. Request block headers in batches of `MAX_HEADERS_PER_REQUEST`.
    /// 3. Validate each batch forms a valid chain continuation.
    /// 4. For each validated header, request the full block.
    /// 5. Pass each block to `process_block` for execution/storage.
    /// 6. Repeat until caught up or max failures exceeded.
    pub async fn start_sync<F, Fut>(
        &self,
        current_height: u64,
        mut process_block: F,
    ) -> Result<u64, NetworkError>
    where
        F: FnMut(Block) -> Fut,
        Fut: std::future::Future<Output = Result<(), NetworkError>>,
    {
        // Check if already syncing
        {
            let mut syncing = self.syncing.write().await;
            if *syncing {
                return Err(NetworkError::AlreadySyncing);
            }
            *syncing = true;
        }

        // Ensure block requester is configured
        let requester_guard = self.block_requester.read().await;
        let requester = match requester_guard.as_ref() {
            Some(r) => r,
            None => {
                *self.syncing.write().await = false;
                error!("Block sync failed: no BlockRequester configured. \
                        Call set_block_requester() before start_sync().");
                return Err(NetworkError::InvalidChain(
                    "BlockRequester not configured — cannot sync".to_string(),
                ));
            }
        };

        // Find best peer
        let best_peer = {
            let peer_manager = self.peer_manager.read().await;
            match peer_manager.get_best_peer() {
                Some(p) => p.clone(),
                None => {
                    *self.syncing.write().await = false;
                    return Err(NetworkError::NoPeersAvailable);
                }
            }
        };

        info!(
            "Starting sync from peer {} at height {}",
            best_peer.peer_id, best_peer.best_height
        );

        // Check if we need to sync
        if best_peer.best_height <= current_height {
            info!("Already synced to latest height {}", current_height);
            *self.syncing.write().await = false;
            return Ok(current_height);
        }

        let blocks_to_sync = best_peer.best_height - current_height;
        info!("Need to sync {} blocks ({} → {})",
              blocks_to_sync, current_height, best_peer.best_height);

        let mut synced_height = current_height;
        let mut consecutive_failures: u32 = 0;

        // Sync loop: request headers in batches, validate, download blocks
        while synced_height < best_peer.best_height {
            let batch_start = synced_height + 1;
            let remaining = best_peer.best_height - synced_height;
            let batch_size = std::cmp::min(remaining as u32, MAX_HEADERS_PER_REQUEST);

            // Step 1: Request headers batch
            let headers = match requester
                .request_headers(&best_peer.peer_id, batch_start, batch_size)
                .await
            {
                Ok(h) if h.is_empty() => {
                    warn!("Peer {} returned empty headers at height {}", best_peer.peer_id, batch_start);
                    consecutive_failures += 1;
                    if consecutive_failures >= MAX_SYNC_FAILURES {
                        error!("Max sync failures ({}) reached, aborting sync", MAX_SYNC_FAILURES);
                        break;
                    }
                    continue;
                }
                Ok(h) => {
                    consecutive_failures = 0;
                    h
                }
                Err(e) => {
                    warn!("Failed to request headers from {}: {}", best_peer.peer_id, e);
                    consecutive_failures += 1;
                    if consecutive_failures >= MAX_SYNC_FAILURES {
                        error!("Max sync failures ({}) reached, aborting sync", MAX_SYNC_FAILURES);
                        break;
                    }
                    continue;
                }
            };

            // Step 2: Validate headers form a valid chain
            if let Err(e) = self.validate_headers(&headers) {
                error!("Invalid header chain from peer {}: {}", best_peer.peer_id, e);
                *self.syncing.write().await = false;
                return Err(e);
            }

            // Verify first header connects to our chain tip
            if let Some(first) = headers.first() {
                if first.height != batch_start {
                    error!(
                        "Header height mismatch: expected {}, got {}",
                        batch_start, first.height
                    );
                    *self.syncing.write().await = false;
                    return Err(NetworkError::InvalidChain(
                        format!("Expected header at height {}, got {}", batch_start, first.height),
                    ));
                }
            }

            // Step 3: Download and process each block
            for header in &headers {
                let block = match requester
                    .request_block_by_height(&best_peer.peer_id, header.height)
                    .await
                {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("Failed to download block {} from {}: {}",
                              header.height, best_peer.peer_id, e);
                        consecutive_failures += 1;
                        if consecutive_failures >= MAX_SYNC_FAILURES {
                            break;
                        }
                        continue;
                    }
                };

                // Verify the downloaded block's header matches what we expected
                if block.header.hash() != header.hash() {
                    error!(
                        "Block hash mismatch at height {}: expected {:?}, got {:?}",
                        header.height,
                        header.hash(),
                        block.header.hash()
                    );
                    *self.syncing.write().await = false;
                    return Err(NetworkError::InvalidChain(
                        format!("Block hash mismatch at height {}", header.height),
                    ));
                }

                // Step 4: Process the block (execute transactions, update state)
                if let Err(e) = process_block(block).await {
                    error!("Failed to process block at height {}: {}", header.height, e);
                    *self.syncing.write().await = false;
                    return Err(e);
                }

                synced_height = header.height;
                consecutive_failures = 0;

                if synced_height % 100 == 0 {
                    info!("Sync progress: block {}/{}", synced_height, best_peer.best_height);
                }
            }

            if consecutive_failures >= MAX_SYNC_FAILURES {
                break;
            }
        }

        *self.syncing.write().await = false;

        if synced_height >= best_peer.best_height {
            info!("Sync complete: reached height {}", synced_height);
        } else {
            warn!(
                "Sync incomplete: reached {} of {} ({} failures)",
                synced_height, best_peer.best_height, consecutive_failures
            );
        }

        Ok(synced_height)
    }

    /// Request a single block from a peer by hash.
    /// Returns `NetworkError` if no `BlockRequester` is configured.
    pub async fn request_block(
        &self,
        peer_id: &PeerId,
        block_hash: Hash,
    ) -> Result<Block, NetworkError> {
        debug!("Requesting block {:?} from peer {}", block_hash, peer_id);

        let requester_guard = self.block_requester.read().await;
        match requester_guard.as_ref() {
            Some(r) => r.request_block(peer_id, block_hash).await,
            None => {
                error!("Cannot request block: no BlockRequester configured");
                Err(NetworkError::BlockNotFound(block_hash))
            }
        }
    }

    /// Request block headers from a peer starting at a given hash.
    /// Returns `NetworkError` if no `BlockRequester` is configured.
    pub async fn request_block_headers(
        &self,
        peer_id: &PeerId,
        start_height: u64,
        max_count: u32,
    ) -> Result<Vec<BlockHeader>, NetworkError> {
        debug!(
            "Requesting {} block headers from {} starting at height {}",
            max_count, peer_id, start_height
        );

        let requester_guard = self.block_requester.read().await;
        match requester_guard.as_ref() {
            Some(r) => r.request_headers(peer_id, start_height, max_count).await,
            None => {
                error!("Cannot request headers: no BlockRequester configured");
                Ok(Vec::new())
            }
        }
    }

    /// Validate block headers form a valid chain
    pub fn validate_headers(&self, headers: &[BlockHeader]) -> Result<(), NetworkError> {
        if headers.is_empty() {
            return Ok(());
        }

        // Check headers are sequential and properly linked
        for i in 1..headers.len() {
            let prev = &headers[i - 1];
            let current = &headers[i];

            // Check height is sequential
            if current.height != prev.height + 1 {
                return Err(NetworkError::InvalidChain(format!(
                    "Non-sequential heights: {} -> {}",
                    prev.height, current.height
                )));
            }

            // Check previous hash links correctly
            if current.previous_hash != prev.hash() {
                return Err(NetworkError::InvalidChain(format!(
                    "Invalid previous hash at height {}",
                    current.height
                )));
            }

            // Check timestamp is increasing
            if current.timestamp <= prev.timestamp {
                return Err(NetworkError::InvalidChain(format!(
                    "Non-increasing timestamp at height {}",
                    current.height
                )));
            }
        }

        Ok(())
    }

    /// Get sync status
    pub async fn get_sync_status(&self) -> SyncStatus {
        let is_syncing = *self.syncing.read().await;
        let peer_count = {
            let peer_manager = self.peer_manager.read().await;
            peer_manager.peer_count()
        };

        SyncStatus {
            is_syncing,
            peer_count,
        }
    }
}

/// Sync status information
#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub peer_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::PeerManager;

    #[tokio::test]
    async fn test_sync_manager_creation() {
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(50)));
        let sync_manager = SyncManager::new(peer_manager);

        assert!(!sync_manager.is_syncing().await);
    }

    #[tokio::test]
    async fn test_validate_headers() {
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(50)));
        let sync_manager = SyncManager::new(peer_manager);

        // Empty headers should be valid
        assert!(sync_manager.validate_headers(&[]).is_ok());
    }

    #[tokio::test]
    async fn test_validate_headers_sequential() {
        use luxtensor_core::block::BlockHeader;

        let peer_manager = Arc::new(RwLock::new(PeerManager::new(50)));
        let sync_manager = SyncManager::new(peer_manager);

        // Create sequential headers
        let header1 = BlockHeader::new(
            1, 0, 1000, [0u8; 32], [0u8; 32], [0u8; 32], [0u8; 32],
            [0u8; 32], [0u8; 64], 0, 1000000, vec![],
        );

        let header2 = BlockHeader::new(
            1, 1, 1001, header1.hash(), [0u8; 32], [0u8; 32], [0u8; 32],
            [0u8; 32], [0u8; 64], 0, 1000000, vec![],
        );

        let headers = vec![header1, header2];
        assert!(sync_manager.validate_headers(&headers).is_ok());
    }

    #[tokio::test]
    async fn test_validate_headers_non_sequential() {
        use luxtensor_core::block::BlockHeader;

        let peer_manager = Arc::new(RwLock::new(PeerManager::new(50)));
        let sync_manager = SyncManager::new(peer_manager);

        // Create non-sequential headers (gap in height)
        let header1 = BlockHeader::new(
            1, 0, 1000, [0u8; 32], [0u8; 32], [0u8; 32], [0u8; 32],
            [0u8; 32], [0u8; 64], 0, 1000000, vec![],
        );

        let header2 = BlockHeader::new(
            1, 2, 1001, header1.hash(), [0u8; 32], [0u8; 32], [0u8; 32],
            [0u8; 32], [0u8; 64], 0, 1000000, vec![],
        );

        let headers = vec![header1, header2];
        assert!(sync_manager.validate_headers(&headers).is_err());
    }

    #[tokio::test]
    async fn test_get_sync_status() {
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(50)));
        let sync_manager = SyncManager::new(peer_manager);

        let status = sync_manager.get_sync_status().await;
        assert!(!status.is_syncing);
        assert_eq!(status.peer_count, 0);
    }

    #[tokio::test]
    async fn test_no_peers_available() {
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(50)));
        let sync_manager = SyncManager::new(peer_manager);

        let result = sync_manager.start_sync(0, |_| async { Ok(()) }).await;
        assert!(matches!(result, Err(NetworkError::NoPeersAvailable)));
    }
}
