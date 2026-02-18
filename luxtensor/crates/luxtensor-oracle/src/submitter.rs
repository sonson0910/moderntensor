use crate::config::OracleConfig;
use crate::error::{OracleError, Result};
use ethers::prelude::*;
use std::sync::Arc;
use std::str::FromStr;
use tracing::info;

// Generate contract bindings again for the submitter
abigen!(
    AIOracle,
    r#"[
        function fulfillRequest(bytes32 requestId, bytes calldata result, bytes32 proofHash) external
    ]"#
);

pub struct TxSubmitter {
    contract: AIOracle<SignerMiddleware<Provider<Ws>, LocalWallet>>,
}

impl TxSubmitter {
    pub async fn new(config: &OracleConfig) -> Result<Self> {
        let provider = Provider::<Ws>::connect(&config.node_ws_url)
            .await
            .map_err(|e| OracleError::Connection(e.to_string()))?;

        // Get chain ID from the connected node instead of hardcoding
        let chain_id = provider.get_chainid()
            .await
            .map_err(|e| OracleError::Connection(format!("Failed to get chain ID: {}", e)))?
            .as_u64();

        let wallet = LocalWallet::from_str(&config.private_key)
            .map_err(|e| OracleError::Connection(format!("Invalid private key: {}", e)))?
            .with_chain_id(chain_id);

        let client = Arc::new(SignerMiddleware::new(provider, wallet));

        let contract = AIOracle::new(config.oracle_contract_address, client);

        Ok(Self { contract })
    }

    /// Maximum number of retry attempts for transaction submission.
    const MAX_RETRIES: u32 = 3;

    pub async fn submit_fulfillment(
        &self,
        request_id: [u8; 32],
        result: Bytes,
        proof_hash: [u8; 32],
    ) -> Result<H256> {
        info!("Submitting fulfillment for request {:?}", hex::encode(request_id));

        let mut backoff = std::time::Duration::from_secs(2);

        for attempt in 0..=Self::MAX_RETRIES {
            let call = self.contract
                .fulfill_request(request_id, result.clone(), proof_hash);
            let send_result = call.send().await;

            match send_result {
                Ok(pending_tx) => {
                    let receipt = pending_tx.await
                        .map_err(|e| OracleError::Transaction(e.to_string()))?
                        .ok_or(OracleError::Transaction("Transaction dropped".to_string()))?;

                    info!("Transaction confirmed: {:?}", receipt.transaction_hash);
                    return Ok(receipt.transaction_hash);
                }
                Err(e) if attempt < Self::MAX_RETRIES => {
                    tracing::warn!(
                        "TX attempt {}/{} failed: {}, retrying in {:?}",
                        attempt + 1,
                        Self::MAX_RETRIES,
                        e,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                }
                Err(e) => {
                    return Err(OracleError::Transaction(format!(
                        "Transaction failed after {} attempts: {}",
                        Self::MAX_RETRIES + 1,
                        e
                    )));
                }
            }
        }

        // Unreachable, but satisfy the compiler
        Err(OracleError::Transaction("Transaction submission exhausted all retries".to_string()))
    }
}
