//! # Faucet Configuration and Rate Limiting
//!
//! Provides faucet configuration and per-address rate limiting for dev/test networks.

use std::collections::HashMap;
use std::time::Instant;

/// Faucet configuration passed from node config to RPC layer.
#[derive(Debug, Clone)]
pub struct FaucetRpcConfig {
    pub enabled: bool,
    pub drip_amount: u128,
    pub cooldown_secs: u64,
    pub max_daily_drips: u32,
}

impl Default for FaucetRpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            drip_amount: 1_000_000_000_000_000_000_000,
            cooldown_secs: 60,
            max_daily_drips: 10,
        }
    }
}

/// Track faucet drip history per address
pub(crate) struct FaucetDripRecord {
    timestamps: Vec<Instant>,
}

/// Faucet rate limiter — per-address cooldown and daily limits
pub struct FaucetRateLimiter {
    records: HashMap<[u8; 20], FaucetDripRecord>,
    config: FaucetRpcConfig,
}

impl FaucetRateLimiter {
    pub fn new(config: FaucetRpcConfig) -> Self {
        Self { records: HashMap::new(), config }
    }

    pub fn check_and_record(&mut self, address: &[u8; 20]) -> Result<(u32, Option<u64>), String> {
        let now = Instant::now();
        let cooldown = std::time::Duration::from_secs(self.config.cooldown_secs);
        let day = std::time::Duration::from_secs(86400);

        let record = self.records.entry(*address).or_insert_with(|| FaucetDripRecord {
            timestamps: Vec::new(),
        });

        record.timestamps.retain(|t| now.duration_since(*t) < day);

        if let Some(last) = record.timestamps.last() {
            let elapsed = now.duration_since(*last);
            if elapsed < cooldown {
                let remaining = (cooldown - elapsed).as_secs();
                return Err(format!("Cooldown active: {} seconds remaining.", remaining));
            }
        }

        let drips_today = record.timestamps.len() as u32;
        if drips_today >= self.config.max_daily_drips {
            return Err(format!(
                "Daily limit reached: {}/{} drips used.",
                drips_today, self.config.max_daily_drips
            ));
        }

        record.timestamps.push(now);
        let drips_remaining = self.config.max_daily_drips - drips_today - 1;
        let next_available = if drips_remaining > 0 { Some(self.config.cooldown_secs) } else { None };
        Ok((drips_remaining, next_available))
    }
}
