use crate::config::OracleConfig;
use crate::error::{OracleError, Result};
use ethers::prelude::*;
use std::str::FromStr;
use std::sync::Arc;
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
        let chain_id_u256 = provider
            .get_chainid()
            .await
            .map_err(|e| OracleError::Connection(format!("Failed to get chain ID: {}", e)))?;

        // SECURITY: Validate chain_id fits in u64 to prevent silent truncation
        if chain_id_u256 > ethers::types::U256::from(u64::MAX) {
            return Err(OracleError::Connection(format!(
                "Chain ID {} exceeds u64::MAX",
                chain_id_u256
            )));
        }
        let chain_id = chain_id_u256.as_u64();

        let wallet = LocalWallet::from_str(&config.private_key)
            .map_err(|e| OracleError::Connection(format!("Invalid private key: {}", e)))?
            .with_chain_id(chain_id);

        let client = Arc::new(SignerMiddleware::new(provider, wallet));

        let contract = AIOracle::new(config.oracle_contract_address, client);

        Ok(Self { contract })
    }

    /// Maximum number of retry attempts for transaction submission.
    const MAX_RETRIES: u32 = 3;

    /// Submit a fulfillment transaction to the oracle smart contract.
    ///
    /// # Gas Configuration
    /// SECURITY(ORACLE-14): Currently relies on provider defaults for gas pricing.
    /// TODO: Add explicit `gas()` / `max_fee_per_gas` / `max_priority_fee_per_gas`
    /// via config for production deployments to prevent stuck or overpriced transactions.
    pub async fn submit_fulfillment(
        &self,
        request_id: [u8; 32],
        result: Bytes,
        proof_hash: [u8; 32],
    ) -> Result<H256> {
        info!("Submitting fulfillment for request {:?}", hex::encode(request_id));

        let mut backoff = std::time::Duration::from_secs(2);

        for attempt in 0..=Self::MAX_RETRIES {
            let call = self.contract.fulfill_request(request_id, result.clone(), proof_hash);
            let send_result = call.send().await;

            match send_result {
                Ok(pending_tx) => {
                    // SECURITY(ORACLE-03): Timeout on tx confirmation to prevent
                    // indefinite hangs when transactions are stuck in the mempool.
                    let receipt =
                        tokio::time::timeout(std::time::Duration::from_secs(120), pending_tx)
                            .await
                            .map_err(|_| OracleError::Timeout(std::time::Duration::from_secs(120)))?
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
