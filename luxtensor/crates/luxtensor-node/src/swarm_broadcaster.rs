// SwarmBroadcaster - Direct P2P broadcaster that sends SwarmCommand
// This bypasses the tx_relay task for more reliable transaction propagation

use luxtensor_core::Transaction;
use luxtensor_network::SwarmCommand;
use luxtensor_rpc::{TransactionBroadcaster, BroadcastError};
use tokio::sync::mpsc::Sender;
use tracing::info;

/// Direct broadcaster that sends SwarmCommand::BroadcastTransaction to P2P swarm
pub struct SwarmBroadcaster {
    sender: Sender<SwarmCommand>,
}

impl SwarmBroadcaster {
    pub fn new(sender: Sender<SwarmCommand>) -> Self {
        Self { sender }
    }
}

impl TransactionBroadcaster for SwarmBroadcaster {
    fn broadcast(&self, tx: &Transaction) -> Result<(), BroadcastError> {
        self.sender
            .try_send(SwarmCommand::BroadcastTransaction(tx.clone()))
            .map_err(|e| BroadcastError::ChannelClosed(e.to_string()))?;

        info!("📤 SwarmBroadcaster: Sent TX directly to swarm: 0x{}", hex::encode(tx.hash()));
        Ok(())
    }

    fn name(&self) -> &'static str {
        "SwarmDirect"
    }
}
