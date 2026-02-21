//! LuxTensor AI Oracle Node
//!
//! Bridges on-chain "Verifiable Intelligence" requests with off-chain AI computation.

pub mod config;
pub mod dispute;
pub mod listener;
pub mod processor;
pub mod submitter;
pub mod error;

pub use config::OracleConfig;
pub use dispute::{DisputeManager, DisputeConfig, DisputeStatus, FraudProof};
pub use listener::EventWatcher;
pub use processor::RequestProcessor;
pub use submitter::TxSubmitter;

// Re-export ethers types used in the dispute API so downstream crates
// (like luxtensor-rpc) don't need a direct ethers dependency.
pub use ethers::types::{H256, Bytes as EthBytes};

use tracing::{info, warn, error};
use std::time::Duration;

/// Maximum number of reconnection attempts before giving up.
const MAX_RECONNECT_ATTEMPTS: u32 = 10;
/// Initial backoff delay between reconnection attempts.
const INITIAL_BACKOFF: Duration = Duration::from_secs(2);
/// Maximum backoff delay between reconnection attempts.
const MAX_BACKOFF: Duration = Duration::from_secs(120);

pub async fn run(config: OracleConfig) -> anyhow::Result<()> {
    info!("Starting AI Oracle Node...");

    let processor = RequestProcessor::new();

    let mut backoff = INITIAL_BACKOFF;
    let mut consecutive_failures: u32 = 0;

    loop {
        // 1. Initialize components (re-create on each reconnection)
        let watcher = match EventWatcher::new(&config).await {
            Ok(w) => {
                consecutive_failures = 0;
                backoff = INITIAL_BACKOFF;
                w
            }
            Err(e) => {
                consecutive_failures += 1;
                if consecutive_failures > MAX_RECONNECT_ATTEMPTS {
                    error!(
                        "Failed to connect after {} attempts, giving up: {}",
                        MAX_RECONNECT_ATTEMPTS, e
                    );
                    return Err(e.into());
                }
                warn!(
                    "Connection failed (attempt {}/{}): {}, retrying in {:?}",
                    consecutive_failures, MAX_RECONNECT_ATTEMPTS, e, backoff
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
                continue;
            }
        };

        let submitter = match TxSubmitter::new(&config).await {
            Ok(s) => s,
            Err(e) => {
                consecutive_failures += 1;
                if consecutive_failures > MAX_RECONNECT_ATTEMPTS {
                    error!(
                        "Failed to create submitter after {} attempts, giving up: {}",
                        MAX_RECONNECT_ATTEMPTS, e
                    );
                    return Err(e.into());
                }
                warn!(
                    "Submitter init failed (attempt {}/{}): {}, retrying in {:?}",
                    consecutive_failures, MAX_RECONNECT_ATTEMPTS, e, backoff
                );
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(MAX_BACKOFF);
                continue;
            }
        };

        info!("Oracle Node initialized. Listening for events...");

        // 2. Watch events with handler
        let result = watcher.watch_events(|event| {
            let watcher_clone = watcher.clone();
            let processor_ref = &processor;
            let submitter_ref = &submitter;

            async move {
                info!("Event received: RequestID={:?}", hex::encode(event.request_id));

                let input_data = match watcher_clone.get_request_input(event.request_id).await {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Failed to fetch request input: {}", e);
                        return;
                    }
                };

                match processor_ref.process_request(
                    event.request_id.into(),
                    event.model_hash.into(),
                    input_data
                ).await {
                    Ok((result, proof_hash)) => {
                        if let Err(e) = submitter_ref.submit_fulfillment(
                            event.request_id,
                            result,
                            proof_hash.into()
                        ).await {
                            error!("Failed to submit transaction: {}", e);
                        }
                    },
                    Err(e) => error!("Failed to process request: {}", e),
                }
            }
        }).await;

        // 3. Event stream ended — reconnect with backoff
        match result {
            Ok(()) => {
                warn!("WebSocket event stream ended unexpectedly, reconnecting...");
            }
            Err(e) => {
                warn!("WebSocket event stream error: {}, reconnecting...", e);
            }
        }

        consecutive_failures += 1;
        if consecutive_failures > MAX_RECONNECT_ATTEMPTS {
            error!(
                "Too many consecutive reconnection failures ({}), shutting down",
                consecutive_failures
            );
            anyhow::bail!("Oracle exceeded maximum reconnection attempts");
        }

        warn!("Reconnecting in {:?} (attempt {}/{})", backoff, consecutive_failures, MAX_RECONNECT_ATTEMPTS);
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}
