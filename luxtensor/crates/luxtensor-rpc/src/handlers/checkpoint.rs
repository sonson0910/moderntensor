// Checkpoint RPC handlers for snapshot management
// Provides endpoints for creating, listing, and managing checkpoints

use jsonrpc_core::{Params, Value};
use luxtensor_storage::{BlockchainDB, CheckpointManager, CHECKPOINT_INTERVAL};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

/// Register checkpoint-related RPC methods
/// Endpoints for managing state snapshots for faster node sync
pub fn register_checkpoint_handlers(
    io: &mut jsonrpc_core::IoHandler,
    db: Arc<BlockchainDB>,
    data_dir: PathBuf,
) {
    let db_clone = db.clone();
    let data_dir_clone = data_dir.clone();

    // checkpoint_list - List all available checkpoints
    io.add_sync_method("checkpoint_list", move |_params: Params| {
        let checkpoint_dir = data_dir_clone.join("checkpoints");
        let manager = CheckpointManager::new(&checkpoint_dir, db_clone.inner_db());

        let checkpoints = manager.list_checkpoints();

        let result: Vec<Value> = checkpoints
            .iter()
            .map(|c| {
                serde_json::json!({
                    "height": c.height,
                    "block_hash": format!("0x{}", hex::encode(&c.block_hash)),
                    "state_root": format!("0x{}", hex::encode(&c.state_root)),
                    "created_at": c.created_at,
                    "size_bytes": c.size_bytes,
                    "checksum": c.checksum,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "checkpoints": result,
            "count": result.len(),
            "interval": CHECKPOINT_INTERVAL,
        }))
    });

    let db_clone = db.clone();
    let data_dir_clone = data_dir.clone();

    // checkpoint_status - Get checkpoint status and next checkpoint height
    io.add_sync_method("checkpoint_status", move |_params: Params| {
        // Get best height, handling Result and Option
        let current_height = db_clone
            .get_best_height()
            .ok()
            .flatten()
            .unwrap_or(0);

        let next_checkpoint_height =
            ((current_height / CHECKPOINT_INTERVAL) + 1) * CHECKPOINT_INTERVAL;

        let checkpoint_dir = data_dir_clone.join("checkpoints");
        let manager = CheckpointManager::new(&checkpoint_dir, db_clone.inner_db());

        let should_create = manager.should_create_checkpoint(current_height);
        let latest_checkpoint = manager.get_nearest_checkpoint(current_height);

        Ok(serde_json::json!({
            "current_height": current_height,
            "next_checkpoint_height": next_checkpoint_height,
            "blocks_until_checkpoint": next_checkpoint_height.saturating_sub(current_height),
            "should_create_now": should_create,
            "latest_checkpoint": latest_checkpoint.map(|c| serde_json::json!({
                "height": c.height,
                "size_bytes": c.size_bytes,
            })),
        }))
    });

    let db_clone = db.clone();
    let data_dir_clone = data_dir.clone();

    // checkpoint_get - Get checkpoint by height
    io.add_sync_method("checkpoint_get", move |params: Params| {
        let parsed: Vec<serde_json::Value> = params.parse()?;
        let height = parsed
            .first()
            .and_then(|v| v.as_u64())
            .ok_or_else(|| jsonrpc_core::Error::invalid_params("Missing height parameter"))?;

        let checkpoint_dir = data_dir_clone.join("checkpoints");
        let manager = CheckpointManager::new(&checkpoint_dir, db_clone.inner_db());

        match manager.get_checkpoint(height) {
            Some(metadata) => Ok(serde_json::json!({
                "found": true,
                "height": metadata.height,
                "block_hash": format!("0x{}", hex::encode(&metadata.block_hash)),
                "state_root": format!("0x{}", hex::encode(&metadata.state_root)),
                "created_at": metadata.created_at,
                "size_bytes": metadata.size_bytes,
                "checksum": metadata.checksum,
            })),
            None => Ok(serde_json::json!({
                "found": false,
                "height": height,
            })),
        }
    });

    info!("📸 Checkpoint RPC handlers registered");
}
