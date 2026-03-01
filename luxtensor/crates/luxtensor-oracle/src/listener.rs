use crate::config::OracleConfig;
use crate::error::{OracleError, Result};
use ethers::prelude::*;
use futures::StreamExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::info;

// Define the event and function we need
abigen!(
    AIOracle,
    r#"[
        event AIRequestCreated(bytes32 indexed requestId, address indexed requester, bytes32 modelHash, uint256 reward)
        function requests(bytes32 requestId) external view returns (address requester, bytes32 modelHash, bytes inputData, uint256 reward, uint256 createdAt, uint256 deadline, uint8 status, bytes result, address fulfiller, bytes32 proofHash)
    ]"#
);

/// SECURITY(ORACLE-17): Timeout for WebSocket connections to prevent indefinite hangs.
const WS_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// SECURITY(ORACLE-02): Timeout for contract RPC calls.
const RPC_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
/// SECURITY(ORACLE-06): Only log every Nth consecutive error to prevent log flooding.
const ERROR_LOG_INTERVAL: u64 = 10;

#[derive(Clone)]
pub struct EventWatcher {
    pub client: Arc<Provider<Ws>>,
    pub contract: Arc<AIOracle<Provider<Ws>>>,
    /// SECURITY(ORACLE-06): Counter for rate-limited error logging.
    error_count: Arc<AtomicU64>,
}

impl EventWatcher {
    pub async fn new(config: &OracleConfig) -> Result<Self> {
        // SECURITY(ORACLE-17): Wrap WS connection in timeout to prevent
        // indefinite hangs on unresponsive DNS/TCP targets.
        let provider =
            tokio::time::timeout(WS_CONNECT_TIMEOUT, Provider::<Ws>::connect(&config.node_ws_url))
                .await
                .map_err(|_| OracleError::Timeout(WS_CONNECT_TIMEOUT))?
                .map_err(|e| OracleError::Connection(e.to_string()))?;

        let client = Arc::new(provider);
        let contract = Arc::new(AIOracle::new(config.oracle_contract_address, Arc::clone(&client)));

        Ok(Self { client, contract, error_count: Arc::new(AtomicU64::new(0)) })
    }

    pub async fn watch_events<F, Fut>(&self, mut handler: F) -> Result<()>
    where
        F: FnMut(AirequestCreatedFilter) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        // Create event filter first
        let event_filter = self.contract.event::<AirequestCreatedFilter>();

        // Subscribe to the event filter
        let mut stream =
            event_filter.subscribe().await.map_err(|e| OracleError::Contract(e.to_string()))?;

        info!("Listening for AIRequestCreated events...");

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    // Reset error counter on success
                    self.error_count.store(0, Ordering::Relaxed);
                    handler(event).await;
                }
                Err(e) => {
                    // SECURITY(ORACLE-06): Rate-limited error logging to prevent
                    // log flooding from unstable/malicious nodes.
                    let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
                    if count == 1 || count % ERROR_LOG_INTERVAL == 0 {
                        tracing::error!(
                            error_count = count,
                            "Error receiving event (showing every {}th): {}",
                            ERROR_LOG_INTERVAL,
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn get_request_input(&self, request_id: [u8; 32]) -> Result<Bytes> {
        // SECURITY(ORACLE-02): Timeout on RPC call to prevent indefinite hang.
        let request = tokio::time::timeout(RPC_CALL_TIMEOUT, async {
            self.contract
                .requests(request_id)
                .call()
                .await
                .map_err(|e| OracleError::Contract(e.to_string()))
        })
        .await
        .map_err(|_| OracleError::Timeout(RPC_CALL_TIMEOUT))??;

        // SECURITY(ORACLE-07): Validate contract returned non-empty input data.
        let input = request.2;
        if input.is_empty() {
            return Err(OracleError::Contract(
                "Request input data is empty — contract may have returned invalid data".to_string(),
            ));
        }

        Ok(input)
    }
}
