//! PostgreSQL storage layer
//!
//! All write operations use [`retry_db_write`] to automatically retry on
//! transient database errors (connection drops, timeouts, serialization
//! conflicts). The default policy is **3 attempts** with exponential backoff
//! starting at 100 ms.

use crate::error::{IndexerError, Result};
use crate::models::*;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::{info, warn};
use std::future::Future;

/// Maximum number of retry attempts for transient DB write failures.
const MAX_RETRIES: u32 = 3;
/// Initial delay between retry attempts.
const INITIAL_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// Retry a fallible async DB operation with exponential backoff.
///
/// Returns the result if successful within [`MAX_RETRIES`] attempts,
/// or `IndexerError::RetryExhausted` wrapping the last error.
async fn retry_db_write<F, Fut, T>(op_name: &str, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, sqlx::Error>>,
{
    let mut delay = INITIAL_RETRY_DELAY;
    let mut last_err = None;

    for attempt in 1..=MAX_RETRIES {
        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                let is_transient = matches!(
                    &e,
                    sqlx::Error::Io(_)
                    | sqlx::Error::PoolTimedOut
                    | sqlx::Error::PoolClosed
                    | sqlx::Error::WorkerCrashed
                );

                if !is_transient || attempt == MAX_RETRIES {
                    last_err = Some(e);
                    break;
                }

                warn!(
                    op = op_name,
                    attempt = attempt,
                    delay_ms = delay.as_millis() as u64,
                    error = %e,
                    "Transient DB error, retrying"
                );
                tokio::time::sleep(delay).await;
                delay *= 2; // exponential backoff
            }
        }
    }

    Err(IndexerError::RetryExhausted {
        operation: op_name.to_string(),
        attempts: MAX_RETRIES,
        last_error: last_err.map(|e| e.to_string()).unwrap_or_default(),
    })
}

/// PostgreSQL storage for indexed data
pub struct Storage {
    pool: PgPool,
}

impl Storage {
    /// Connect to PostgreSQL
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<()> {
        // Create tables
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS blocks (
                number BIGINT PRIMARY KEY,
                hash VARCHAR(66) NOT NULL,
                parent_hash VARCHAR(66),
                timestamp BIGINT NOT NULL,
                tx_count INT NOT NULL DEFAULT 0,
                indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS transactions (
                hash VARCHAR(66) PRIMARY KEY,
                block_number BIGINT NOT NULL,
                chain_id BIGINT NOT NULL DEFAULT 0,
                from_address VARCHAR(42) NOT NULL,
                to_address VARCHAR(42),
                value VARCHAR(78) NOT NULL,
                gas_used BIGINT NOT NULL,
                status SMALLINT NOT NULL,
                tx_type VARCHAR(50) NOT NULL,
                indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS token_transfers (
                id BIGSERIAL PRIMARY KEY,
                tx_hash VARCHAR(66) NOT NULL,
                block_number BIGINT NOT NULL,
                from_address VARCHAR(42) NOT NULL,
                to_address VARCHAR(42) NOT NULL,
                amount VARCHAR(78) NOT NULL,
                timestamp BIGINT NOT NULL
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS stakes (
                id BIGSERIAL PRIMARY KEY,
                block_number BIGINT NOT NULL,
                coldkey VARCHAR(42) NOT NULL,
                hotkey VARCHAR(42) NOT NULL,
                amount VARCHAR(78) NOT NULL,
                action VARCHAR(20) NOT NULL,
                timestamp BIGINT NOT NULL
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS neurons (
                id BIGSERIAL PRIMARY KEY,
                block_number BIGINT NOT NULL,
                subnet_id BIGINT NOT NULL,
                uid BIGINT NOT NULL,
                hotkey VARCHAR(42) NOT NULL,
                coldkey VARCHAR(42) NOT NULL,
                stake VARCHAR(78) NOT NULL,
                trust DOUBLE PRECISION NOT NULL,
                consensus DOUBLE PRECISION NOT NULL,
                incentive DOUBLE PRECISION NOT NULL,
                dividends DOUBLE PRECISION NOT NULL,
                emission VARCHAR(78) NOT NULL,
                timestamp BIGINT NOT NULL
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS weight_commits (
                id BIGSERIAL PRIMARY KEY,
                block_number BIGINT NOT NULL,
                subnet_id BIGINT NOT NULL,
                validator_uid BIGINT NOT NULL,
                weights_hash VARCHAR(66) NOT NULL,
                timestamp BIGINT NOT NULL
            )
        "#).execute(&self.pool).await?;

        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS sync_status (
                id INT PRIMARY KEY DEFAULT 1,
                last_indexed_block BIGINT NOT NULL DEFAULT 0,
                last_indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                is_syncing BOOLEAN NOT NULL DEFAULT FALSE
            )
        "#).execute(&self.pool).await?;

        // Create indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tx_from ON transactions(from_address)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tx_to ON transactions(to_address)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_tx_block ON transactions(block_number)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transfers_from ON token_transfers(from_address)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_transfers_to ON token_transfers(to_address)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_stakes_hotkey ON stakes(hotkey)")
            .execute(&self.pool).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_neurons_subnet ON neurons(subnet_id, uid)")
            .execute(&self.pool).await?;

        // Initialize sync status
        sqlx::query(r#"
            INSERT INTO sync_status (id, last_indexed_block, is_syncing)
            VALUES (1, 0, FALSE)
            ON CONFLICT (id) DO NOTHING
        "#).execute(&self.pool).await?;

        info!("Database migrations complete");
        Ok(())
    }

    // ======== Block operations ========

    /// Insert or update block (with retry).
    pub async fn upsert_block(&self, block: &Block) -> Result<()> {
        let pool = &self.pool;
        let number = block.number;
        let hash = block.hash.clone();
        let parent_hash = block.parent_hash.clone();
        let timestamp = block.timestamp;
        let tx_count = block.tx_count;

        retry_db_write("upsert_block", || async {
            sqlx::query(r#"
                INSERT INTO blocks (number, hash, parent_hash, timestamp, tx_count)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (number) DO UPDATE SET
                    hash = EXCLUDED.hash,
                    parent_hash = EXCLUDED.parent_hash,
                    timestamp = EXCLUDED.timestamp,
                    tx_count = EXCLUDED.tx_count
            "#)
            .bind(number)
            .bind(&hash)
            .bind(&parent_hash)
            .bind(timestamp)
            .bind(tx_count)
            .execute(pool)
            .await?;
            Ok(())
        }).await
    }

    /// Get block by number
    pub async fn get_block(&self, number: i64) -> Result<Option<Block>> {
        let block = sqlx::query_as::<_, Block>(
            "SELECT * FROM blocks WHERE number = $1"
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await?;

        Ok(block)
    }

    /// Get latest block
    pub async fn get_latest_block(&self) -> Result<Option<Block>> {
        let block = sqlx::query_as::<_, Block>(
            "SELECT * FROM blocks ORDER BY number DESC LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(block)
    }

    /// Get blocks in range
    pub async fn get_blocks(&self, from: i64, to: i64) -> Result<Vec<Block>> {
        let blocks = sqlx::query_as::<_, Block>(
            "SELECT * FROM blocks WHERE number >= $1 AND number <= $2 ORDER BY number"
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;

        Ok(blocks)
    }

    // ======== Transaction operations ========

    /// Insert transaction (with retry).
    pub async fn insert_transaction(&self, tx: &Transaction) -> Result<()> {
        let pool = &self.pool;
        let hash = tx.hash.clone();
        let block_number = tx.block_number;
        let chain_id = tx.chain_id;
        let from_address = tx.from_address.clone();
        let to_address = tx.to_address.clone();
        let value = tx.value.clone();
        let gas_used = tx.gas_used;
        let status = tx.status;
        let tx_type = tx.tx_type.clone();

        retry_db_write("insert_transaction", || async {
            sqlx::query(r#"
                INSERT INTO transactions (hash, block_number, chain_id, from_address, to_address, value, gas_used, status, tx_type)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (hash) DO NOTHING
            "#)
            .bind(&hash)
            .bind(block_number)
            .bind(chain_id)
            .bind(&from_address)
            .bind(&to_address)
            .bind(&value)
            .bind(gas_used)
            .bind(status)
            .bind(&tx_type)
            .execute(pool)
            .await?;
            Ok(())
        }).await
    }

    /// Get transactions by address
    pub async fn get_transactions_by_address(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Transaction>> {
        let txs = sqlx::query_as::<_, Transaction>(r#"
            SELECT * FROM transactions
            WHERE from_address = $1 OR to_address = $1
            ORDER BY block_number DESC
            LIMIT $2 OFFSET $3
        "#)
        .bind(address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(txs)
    }

    /// Get transaction by hash
    pub async fn get_transaction_by_hash(&self, hash: &str) -> Result<Option<Transaction>> {
        let tx = sqlx::query_as::<_, Transaction>(
            "SELECT * FROM transactions WHERE hash = $1"
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;

        Ok(tx)
    }

    // ======== Token transfer operations ========

    /// Insert token transfer (with retry).
    pub async fn insert_token_transfer(&self, transfer: &TokenTransfer) -> Result<()> {
        let pool = &self.pool;
        let tx_hash = transfer.tx_hash.clone();
        let block_number = transfer.block_number;
        let from_address = transfer.from_address.clone();
        let to_address = transfer.to_address.clone();
        let amount = transfer.amount.clone();
        let timestamp = transfer.timestamp;

        retry_db_write("insert_token_transfer", || async {
            sqlx::query(r#"
                INSERT INTO token_transfers (tx_hash, block_number, from_address, to_address, amount, timestamp)
                VALUES ($1, $2, $3, $4, $5, $6)
            "#)
            .bind(&tx_hash)
            .bind(block_number)
            .bind(&from_address)
            .bind(&to_address)
            .bind(&amount)
            .bind(timestamp)
            .execute(pool)
            .await?;
            Ok(())
        }).await
    }

    /// Get token transfers by address
    pub async fn get_transfers_by_address(
        &self,
        address: &str,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<TokenTransfer>> {
        let transfers = sqlx::query_as::<_, TokenTransfer>(r#"
            SELECT * FROM token_transfers
            WHERE from_address = $1 OR to_address = $1
            ORDER BY block_number DESC
            LIMIT $2 OFFSET $3
        "#)
        .bind(address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(transfers)
    }

    // ======== Stake operations ========

    /// Insert stake event (with retry).
    pub async fn insert_stake_event(&self, stake: &StakeEvent) -> Result<()> {
        let pool = &self.pool;
        let block_number = stake.block_number;
        let coldkey = stake.coldkey.clone();
        let hotkey = stake.hotkey.clone();
        let amount = stake.amount.clone();
        let action = stake.action.clone();
        let timestamp = stake.timestamp;

        retry_db_write("insert_stake_event", || async {
            sqlx::query(r#"
                INSERT INTO stakes (block_number, coldkey, hotkey, amount, action, timestamp)
                VALUES ($1, $2, $3, $4, $5, $6)
            "#)
            .bind(block_number)
            .bind(&coldkey)
            .bind(&hotkey)
            .bind(&amount)
            .bind(&action)
            .bind(timestamp)
            .execute(pool)
            .await?;
            Ok(())
        }).await
    }

    /// Get stake history for hotkey
    pub async fn get_stake_history(&self, hotkey: &str, limit: i32) -> Result<Vec<StakeEvent>> {
        let stakes = sqlx::query_as::<_, StakeEvent>(r#"
            SELECT * FROM stakes
            WHERE hotkey = $1
            ORDER BY block_number DESC
            LIMIT $2
        "#)
        .bind(hotkey)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(stakes)
    }

    // ======== Sync status operations ========

    /// Get sync status
    pub async fn get_sync_status(&self) -> Result<SyncStatus> {
        let status = sqlx::query_as::<_, SyncStatus>(
            "SELECT * FROM sync_status WHERE id = 1"
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(status)
    }

    /// Update sync status (with retry).
    pub async fn update_sync_status(&self, block_number: i64, is_syncing: bool) -> Result<()> {
        let pool = &self.pool;

        retry_db_write("update_sync_status", || async {
            sqlx::query(r#"
                UPDATE sync_status
                SET last_indexed_block = $1, last_indexed_at = NOW(), is_syncing = $2
                WHERE id = 1
            "#)
            .bind(block_number)
            .bind(is_syncing)
            .execute(pool)
            .await?;
            Ok(())
        }).await
    }

    /// Get database pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
