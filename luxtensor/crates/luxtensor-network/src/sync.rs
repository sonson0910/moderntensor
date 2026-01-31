use crate::error::NetworkError;
use crate::peer::PeerManager;
use libp2p::PeerId;
use luxtensor_core::block::{Block, BlockHeader};
use luxtensor_core::types::Hash;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Sync manager for blockchain synchronization
pub struct SyncManager {
    peer_manager: Arc<RwLock<PeerManager>>,
    syncing: Arc<RwLock<bool>>,
}

impl SyncManager {
    /// Create a new sync manager
    pub fn new(peer_manager: Arc<RwLock<PeerManager>>) -> Self {
        Self {
            peer_manager,
            syncing: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if currently syncing
    pub async fn is_syncing(&self) -> bool {
        *self.syncing.read().await
    }

    /// Start syncing from best peer
    pub async fn start_sync<F, Fut>(
        &self,
        current_height: u64,
        _process_block: F,
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

        // Find best peer
        let best_peer = {
            let peer_manager = self.peer_manager.read().await;
            peer_manager
                .get_best_peer()
                .ok_or(NetworkError::NoPeersAvailable)?
                .clone()
        };

        info!(
            "Starting sync from peer {} at height {}",
            best_peer.peer_id, best_peer.best_height
        );

        // Check if we need to sync
        if best_peer.best_height <= current_height {
            info!("Already synced to latest height");
            *self.syncing.write().await = false;
            return Ok(current_height);
        }

        let blocks_to_sync = best_peer.best_height - current_height;
        info!("Need to sync {} blocks", blocks_to_sync);

        // In a real implementation, we would:
        // 1. Request block headers from best peer
        // 2. Validate headers form a valid chain
        // 3. Download and process blocks
        // 4. Update local state
        
        // For now, we'll just simulate the process
        let synced_height = current_height;

        *self.syncing.write().await = false;
        
        Ok(synced_height)
    }

    /// Request block from peer
    pub async fn request_block(
        &self,
        peer_id: &PeerId,
        block_hash: Hash,
    ) -> Result<Block, NetworkError> {
        debug!("Requesting block {:?} from peer {}", block_hash, peer_id);
        
        // In a real implementation, we would send a request-response message
        // For now, return an error
        Err(NetworkError::BlockNotFound(block_hash))
    }

    /// Request block headers from peer
    pub async fn request_block_headers(
        &self,
        peer_id: &PeerId,
        start_hash: Hash,
        max_count: u32,
    ) -> Result<Vec<BlockHeader>, NetworkError> {
        debug!(
            "Requesting {} block headers from {} starting at {:?}",
            max_count, peer_id, start_hash
        );
        
        // In a real implementation, we would send a request-response message
        // For now, return empty vec
        Ok(Vec::new())
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
