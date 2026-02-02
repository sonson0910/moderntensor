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

        let wallet = LocalWallet::from_str(&config.private_key)
            .map_err(|e| OracleError::Connection(format!("Invalid private key: {}", e)))?
            .with_chain_id(777u64); // Hardcoded chain ID for now

        let client = Arc::new(SignerMiddleware::new(provider, wallet));

        let contract = AIOracle::new(config.oracle_contract_address, client);

        Ok(Self { contract })
    }

    pub async fn submit_fulfillment(
        &self,
        request_id: [u8; 32],
        result: Bytes,
        proof_hash: [u8; 32],
    ) -> Result<H256> {
        info!("Submitting fulfillment for request {:?}", hex::encode(request_id));

        let tx = self.contract
            .fulfill_request(request_id, result, proof_hash);

        let pending_tx = tx.send().await
            .map_err(|e| OracleError::Transaction(e.to_string()))?;

        let receipt = pending_tx.await
            .map_err(|e| OracleError::Transaction(e.to_string()))?
            .ok_or(OracleError::Transaction("Transaction dropped".to_string()))?;

        info!("Transaction confirmed: {:?}", receipt.transaction_hash);
        Ok(receipt.transaction_hash)
    }
}
