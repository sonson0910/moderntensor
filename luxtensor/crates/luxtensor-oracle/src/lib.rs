//! LuxTensor AI Oracle Node
//!
//! Bridges on-chain "Verifiable Intelligence" requests with off-chain AI computation.

pub mod config;
pub mod listener;
pub mod processor;
pub mod submitter;
pub mod error;

pub use config::OracleConfig;
pub use listener::EventWatcher;
pub use processor::RequestProcessor;
pub use submitter::TxSubmitter;

use tracing::{info, error};

pub async fn run(config: OracleConfig) -> anyhow::Result<()> {
    info!("Starting AI Oracle Node...");

    // 1. Initialize components
    let watcher = EventWatcher::new(&config).await?;
    let processor = RequestProcessor::new();
    let submitter = TxSubmitter::new(&config).await?;

    info!("Oracle Node initialized. Listening for events...");

    // 2. Watch events with handler
    watcher.watch_events(|event| {
        // Clone references for the async block
        let watcher_clone = watcher.clone();
        let processor_ref = &processor;
        let submitter_ref = &submitter;

        async move {
            info!("Event received: RequestID={:?}", hex::encode(event.request_id));

            // Fetch full input data from contract
            let input_data = match watcher_clone.get_request_input(event.request_id).await {
                Ok(data) => data,
                Err(e) => {
                    error!("Failed to fetch request input: {}", e);
                    return;
                }
            };

            // Process off-chain
            match processor_ref.process_request(
                event.request_id.into(),
                event.model_hash.into(),
                input_data
            ).await {
                Ok((result, proof_hash)) => {
                    // Submit on-chain
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
    }).await?;

    Ok(())
}
