//! Event decoder - parses transactions and extracts events

use crate::error::Result;
use crate::models::{Transaction, TokenTransfer, StakeEvent};
use crate::storage::Storage;
use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, warn};

/// Event decoder for parsing blockchain data
pub struct EventDecoder {
    storage: Arc<Storage>,
}

impl EventDecoder {
    /// Create new event decoder
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// Decode a transaction and extract events
    pub async fn decode_transaction(
        &self,
        block_number: i64,
        timestamp: i64,
        tx_data: &serde_json::Value,
    ) -> Result<()> {
        let hash = tx_data.get("hash")
            .and_then(|h| h.as_str())
            .unwrap_or("")
            .to_string();

        let from_address = tx_data.get("from")
            .and_then(|f| f.as_str())
            .unwrap_or("")
            .to_string();

        let to_address = tx_data.get("to")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        let value = tx_data.get("value")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0")
            .to_string();

        let gas_used = tx_data.get("gasUsed")
            .and_then(|g| g.as_str())
            .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0);

        let status = tx_data.get("status")
            .and_then(|s| s.as_str())
            .map(|s| if s == "0x1" { 1i16 } else { 0i16 })
            .unwrap_or(1);

        let input = tx_data.get("input")
            .and_then(|i| i.as_str())
            .unwrap_or("0x");

        // Determine transaction type
        let tx_type = self.classify_transaction(input, &to_address);

        debug!("Decoded tx {} type={}", &hash, &tx_type);

        // Get chain_id from transaction data
        let chain_id = tx_data.get("chainId")
            .and_then(|c| c.as_str())
            .and_then(|s| i64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
            .unwrap_or(8898); // Default to LuxTensor devnet chain_id

        // Store transaction
        let transaction = Transaction {
            hash: hash.clone(),
            block_number,
            chain_id,
            from_address: from_address.clone(),
            to_address: to_address.clone(),
            value: value.clone(),
            gas_used,
            status,
            tx_type: tx_type.clone(),
            indexed_at: Some(Utc::now()),
        };

        self.storage.insert_transaction(&transaction).await?;

        // Extract events based on transaction type
        match tx_type.as_str() {
            "transfer" => {
                self.handle_transfer(block_number, timestamp, &hash, &from_address, &to_address, &value).await?;
            }
            "stake" => {
                self.handle_stake(block_number, timestamp, &from_address, input).await?;
            }
            "unstake" => {
                self.handle_unstake(block_number, timestamp, &from_address, input).await?;
            }
            "erc20_transfer" => {
                self.handle_erc20_transfer(block_number, timestamp, &hash, &from_address, input).await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Classify transaction by input data
    fn classify_transaction(&self, input: &str, to_address: &Option<String>) -> String {
        if input == "0x" || input.is_empty() {
            return "transfer".to_string();
        }

        // Get function selector (first 4 bytes after 0x)
        let selector = if input.len() >= 10 {
            &input[0..10]
        } else {
            return "unknown".to_string();
        };

        match selector {
            // ERC20 transfer
            "0xa9059cbb" => "erc20_transfer".to_string(),
            // ERC20 transferFrom
            "0x23b872dd" => "erc20_transfer".to_string(),
            // Stake
            "0xa694fc3a" => "stake".to_string(),
            // Unstake
            "0x2e1a7d4d" => "unstake".to_string(),
            // Add more selectors as needed
            _ => {
                if to_address.is_none() {
                    "contract_deploy".to_string()
                } else {
                    "contract_call".to_string()
                }
            }
        }
    }

    /// Handle native token transfer
    async fn handle_transfer(
        &self,
        block_number: i64,
        timestamp: i64,
        tx_hash: &str,
        from: &str,
        to: &Option<String>,
        value: &str,
    ) -> Result<()> {
        if let Some(to_addr) = to {
            // Parse value (hex to decimal string)
            let amount = self.parse_hex_amount(value);

            let transfer = TokenTransfer {
                id: 0, // Auto-generated
                tx_hash: tx_hash.to_string(),
                block_number,
                from_address: from.to_string(),
                to_address: to_addr.clone(),
                amount,
                timestamp,
            };

            self.storage.insert_token_transfer(&transfer).await?;
        }

        Ok(())
    }

    /// Handle ERC20 transfer event
    async fn handle_erc20_transfer(
        &self,
        block_number: i64,
        timestamp: i64,
        tx_hash: &str,
        from: &str,
        input: &str,
    ) -> Result<()> {
        // Parse ERC20 transfer input data
        // transfer(address,uint256): 0xa9059cbb + address (32 bytes) + amount (32 bytes)
        if input.len() < 138 {
            warn!("Invalid ERC20 transfer input length");
            return Ok(());
        }

        let to_address = format!("0x{}", &input[34..74]);
        let amount_hex = &input[74..138];
        let amount = self.parse_hex_amount(&format!("0x{}", amount_hex));

        let transfer = TokenTransfer {
            id: 0,
            tx_hash: tx_hash.to_string(),
            block_number,
            from_address: from.to_string(), // Transaction sender
            to_address,
            amount,
            timestamp,
        };

        self.storage.insert_token_transfer(&transfer).await?;

        Ok(())
    }

    /// Handle stake event
    /// Calldata layout: stake(address hotkey, uint256 amount)
    /// selector (4 bytes) + hotkey (32 bytes, left-padded address) + amount (32 bytes)
    async fn handle_stake(
        &self,
        block_number: i64,
        timestamp: i64,
        from: &str,
        input: &str,
    ) -> Result<()> {
        // Parse hotkey and amount from calldata
        // Minimum: 8 (selector) + 64 (address) + 64 (uint256) = 136 hex chars
        let (hotkey, amount) = if input.len() >= 136 {
            let hotkey = format!("0x{}", &input[32..72]); // bytes 12..32 of first param (address)
            let amount_hex = &input[72..136];
            let amount = self.parse_hex_amount(&format!("0x{}", amount_hex));
            (hotkey, amount)
        } else {
            warn!("Stake calldata too short ({} chars), recording with partial data", input.len());
            (from.to_string(), "0".to_string())
        };

        let stake_event = StakeEvent {
            id: 0,
            block_number,
            coldkey: from.to_string(),
            hotkey,
            amount,
            action: "stake".to_string(),
            timestamp,
        };

        self.storage.insert_stake_event(&stake_event).await?;

        Ok(())
    }

    /// Handle unstake event
    /// Same calldata layout as stake: unstake(address hotkey, uint256 amount)
    async fn handle_unstake(
        &self,
        block_number: i64,
        timestamp: i64,
        from: &str,
        input: &str,
    ) -> Result<()> {
        let (hotkey, amount) = if input.len() >= 136 {
            let hotkey = format!("0x{}", &input[32..72]);
            let amount_hex = &input[72..136];
            let amount = self.parse_hex_amount(&format!("0x{}", amount_hex));
            (hotkey, amount)
        } else {
            warn!("Unstake calldata too short ({} chars), recording with partial data", input.len());
            (from.to_string(), "0".to_string())
        };

        let stake_event = StakeEvent {
            id: 0,
            block_number,
            coldkey: from.to_string(),
            hotkey,
            amount,
            action: "unstake".to_string(),
            timestamp,
        };

        self.storage.insert_stake_event(&stake_event).await?;

        Ok(())
    }

    /// Parse hex amount to decimal string
    fn parse_hex_amount(&self, hex: &str) -> String {
        let clean = hex.trim_start_matches("0x");
        if clean.is_empty() {
            return "0".to_string();
        }

        // For large numbers, keep as hex string
        // In production, use a big integer library
        match u128::from_str_radix(clean, 16) {
            Ok(n) => n.to_string(),
            Err(_) => format!("0x{}", clean), // Keep as hex if too large
        }
    }
}
