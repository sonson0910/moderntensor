//! PostgreSQL storage layer

use crate::error::Result;
use crate::models::*;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::info;

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

    /// Insert or update block
    pub async fn upsert_block(&self, block: &Block) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO blocks (number, hash, parent_hash, timestamp, tx_count)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (number) DO UPDATE SET
                hash = EXCLUDED.hash,
                parent_hash = EXCLUDED.parent_hash,
                timestamp = EXCLUDED.timestamp,
                tx_count = EXCLUDED.tx_count
        "#)
        .bind(block.number)
        .bind(&block.hash)
        .bind(&block.parent_hash)
        .bind(block.timestamp)
        .bind(block.tx_count)
        .execute(&self.pool)
        .await?;

        Ok(())
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

    /// Insert transaction
    pub async fn insert_transaction(&self, tx: &Transaction) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO transactions (hash, block_number, chain_id, from_address, to_address, value, gas_used, status, tx_type)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (hash) DO NOTHING
        "#)
        .bind(&tx.hash)
        .bind(tx.block_number)
        .bind(tx.chain_id)
        .bind(&tx.from_address)
        .bind(&tx.to_address)
        .bind(&tx.value)
        .bind(tx.gas_used)
        .bind(tx.status)
        .bind(&tx.tx_type)
        .execute(&self.pool)
        .await?;

        Ok(())
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

    /// Insert token transfer
    pub async fn insert_token_transfer(&self, transfer: &TokenTransfer) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO token_transfers (tx_hash, block_number, from_address, to_address, amount, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#)
        .bind(&transfer.tx_hash)
        .bind(transfer.block_number)
        .bind(&transfer.from_address)
        .bind(&transfer.to_address)
        .bind(&transfer.amount)
        .bind(transfer.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
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

    /// Insert stake event
    pub async fn insert_stake_event(&self, stake: &StakeEvent) -> Result<()> {
        sqlx::query(r#"
            INSERT INTO stakes (block_number, coldkey, hotkey, amount, action, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#)
        .bind(stake.block_number)
        .bind(&stake.coldkey)
        .bind(&stake.hotkey)
        .bind(&stake.amount)
        .bind(&stake.action)
        .bind(stake.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
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

    /// Update sync status
    pub async fn update_sync_status(&self, block_number: i64, is_syncing: bool) -> Result<()> {
        sqlx::query(r#"
            UPDATE sync_status
            SET last_indexed_block = $1, last_indexed_at = NOW(), is_syncing = $2
            WHERE id = 1
        "#)
        .bind(block_number)
        .bind(is_syncing)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get database pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
