use crate::config::OracleConfig;
use crate::error::{OracleError, Result};
use ethers::prelude::*;
use std::sync::Arc;
use tracing::info;
use futures::StreamExt;

// Define the event and function we need
abigen!(
    AIOracle,
    r#"[
        event AIRequestCreated(bytes32 indexed requestId, address indexed requester, bytes32 modelHash, uint256 reward)
        function requests(bytes32 requestId) external view returns (address requester, bytes32 modelHash, bytes inputData, uint256 reward, uint256 createdAt, uint256 deadline, uint8 status, bytes result, address fulfiller, bytes32 proofHash)
    ]"#
);

#[derive(Clone)]
pub struct EventWatcher {
    pub client: Arc<Provider<Ws>>,
    pub contract: Arc<AIOracle<Provider<Ws>>>,
}

impl EventWatcher {
    pub async fn new(config: &OracleConfig) -> Result<Self> {
        let provider = Provider::<Ws>::connect(&config.node_ws_url)
            .await
            .map_err(|e| OracleError::Connection(e.to_string()))?;

        let client = Arc::new(provider);
        let contract = Arc::new(AIOracle::new(config.oracle_contract_address, Arc::clone(&client)));

        Ok(Self {
            client,
            contract,
        })
    }

    pub async fn watch_events<F, Fut>(&self, mut handler: F) -> Result<()>
    where
        F: FnMut(AirequestCreatedFilter) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        // Create event filter first
        let event_filter = self.contract.event::<AirequestCreatedFilter>();

        // Subscribe to the event filter
        let mut stream = event_filter
            .subscribe()
            .await
            .map_err(|e| OracleError::Contract(e.to_string()))?;

        info!("Listening for AIRequestCreated events...");

        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => handler(event).await,
                Err(e) => tracing::error!("Error receiving event: {}", e),
            }
        }

        Ok(())
    }

    pub async fn get_request_input(&self, request_id: [u8; 32]) -> Result<Bytes> {
        let request = self.contract
            .requests(request_id)
            .call()
            .await
            .map_err(|e| OracleError::Contract(e.to_string()))?;

        Ok(request.2)
    }
}
