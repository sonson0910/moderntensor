// Rate limiting module for P2P network protection
// Uses token bucket algorithm for efficient rate limiting

use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::warn;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Maximum requests per second
    pub requests_per_second: u32,
    /// Burst allowance (max tokens)
    pub burst_size: u32,
    /// Time to store banned peers
    pub ban_duration: Duration,
    /// Number of violations before ban
    pub violations_before_ban: u32,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 100,
            burst_size: 200,
            ban_duration: Duration::from_secs(300), // 5 minutes
            violations_before_ban: 5,
        }
    }
}

/// Token bucket for rate limiting
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
    violations: u32,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
            violations: 0,
        }
    }

    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            self.violations += 1;
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }
}

/// Ban record
struct BanRecord {
    banned_at: Instant,
    duration: Duration,
    reason: String,
}

impl BanRecord {
    fn is_expired(&self) -> bool {
        self.banned_at.elapsed() > self.duration
    }
}

/// Rate limiter for P2P requests
pub struct RateLimiter {
    config: RateLimiterConfig,
    buckets: RwLock<HashMap<String, TokenBucket>>,
    bans: RwLock<HashMap<String, BanRecord>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            config: config.clone(),
            buckets: RwLock::new(HashMap::new()),
            bans: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a request is allowed
    /// Returns true if allowed, false if rate limited
    pub fn check(&self, peer_id: &str) -> bool {
        // Check if banned first
        if self.is_banned(peer_id) {
            return false;
        }

        let mut buckets = self.buckets.write();

        let bucket = buckets.entry(peer_id.to_string()).or_insert_with(|| {
            TokenBucket::new(
                self.config.burst_size as f64,
                self.config.requests_per_second as f64,
            )
        });

        let allowed = bucket.try_consume(1.0);

        if !allowed {
            // Check if should ban
            if bucket.violations >= self.config.violations_before_ban {
                drop(buckets);
                self.ban(peer_id, "Too many rate limit violations");
            }
        }

        allowed
    }

    /// Check if request is allowed with custom cost
    pub fn check_with_cost(&self, peer_id: &str, cost: f64) -> bool {
        if self.is_banned(peer_id) {
            return false;
        }

        let mut buckets = self.buckets.write();

        let bucket = buckets.entry(peer_id.to_string()).or_insert_with(|| {
            TokenBucket::new(
                self.config.burst_size as f64,
                self.config.requests_per_second as f64,
            )
        });

        bucket.try_consume(cost)
    }

    /// Check if a peer is banned
    pub fn is_banned(&self, peer_id: &str) -> bool {
        let bans = self.bans.read();
        if let Some(record) = bans.get(peer_id) {
            if record.is_expired() {
                drop(bans);
                self.unban(peer_id);
                false
            } else {
                true
            }
        } else {
            false
        }
    }

    /// Ban a peer
    pub fn ban(&self, peer_id: &str, reason: &str) {
        warn!("Banning peer {} for: {}", peer_id, reason);

        self.bans.write().insert(
            peer_id.to_string(),
            BanRecord {
                banned_at: Instant::now(),
                duration: self.config.ban_duration,
                reason: reason.to_string(),
            },
        );
    }

    /// Ban with custom duration
    pub fn ban_for(&self, peer_id: &str, duration: Duration, reason: &str) {
        warn!("Banning peer {} for {:?}: {}", peer_id, duration, reason);

        self.bans.write().insert(
            peer_id.to_string(),
            BanRecord {
                banned_at: Instant::now(),
                duration,
                reason: reason.to_string(),
            },
        );
    }

    /// Unban a peer
    pub fn unban(&self, peer_id: &str) {
        self.bans.write().remove(peer_id);
    }

    /// Reset rate limit for a peer
    pub fn reset(&self, peer_id: &str) {
        self.buckets.write().remove(peer_id);
    }

    /// Clean up expired bans and unused buckets
    pub fn cleanup(&self) {
        // Remove expired bans
        self.bans.write().retain(|_, record| !record.is_expired());

        // Remove very old buckets (not used in 10 minutes)
        let threshold = Duration::from_secs(600);
        self.buckets.write().retain(|_, bucket| {
            bucket.last_refill.elapsed() < threshold
        });
    }

    /// Get ban info
    pub fn get_ban_info(&self, peer_id: &str) -> Option<(Duration, String)> {
        let bans = self.bans.read();
        bans.get(peer_id).map(|record| {
            let remaining = record.duration.saturating_sub(record.banned_at.elapsed());
            (remaining, record.reason.clone())
        })
    }

    /// Get number of active bans
    pub fn ban_count(&self) -> usize {
        self.bans.read().len()
    }

    /// Get statistics
    pub fn stats(&self) -> RateLimiterStats {
        RateLimiterStats {
            tracked_peers: self.buckets.read().len(),
            banned_peers: self.bans.read().len(),
        }
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub tracked_peers: usize,
    pub banned_peers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_basic_rate_limiting() {
        let config = RateLimiterConfig {
            requests_per_second: 10,
            burst_size: 5,
            ban_duration: Duration::from_secs(60),
            violations_before_ban: 10,
        };

        let limiter = RateLimiter::new(config);
        let peer = "peer1";

        // First 5 requests should pass (burst)
        for _ in 0..5 {
            assert!(limiter.check(peer));
        }

        // 6th request should fail (exceeded burst)
        assert!(!limiter.check(peer));
    }

    #[test]
    fn test_token_refill() {
        let config = RateLimiterConfig {
            requests_per_second: 100,
            burst_size: 10,
            ban_duration: Duration::from_secs(60),
            violations_before_ban: 100,
        };

        let limiter = RateLimiter::new(config);
        let peer = "peer1";

        // Exhaust tokens
        for _ in 0..10 {
            limiter.check(peer);
        }

        // Should be rate limited
        assert!(!limiter.check(peer));

        // Wait for refill
        sleep(Duration::from_millis(100));

        // Should have some tokens now
        assert!(limiter.check(peer));
    }

    #[test]
    fn test_banning() {
        let config = RateLimiterConfig {
            requests_per_second: 10,
            burst_size: 2,
            ban_duration: Duration::from_millis(100),
            violations_before_ban: 3,
        };

        let limiter = RateLimiter::new(config);
        let peer = "peer1";

        // Exhaust burst and trigger violations
        for _ in 0..5 {
            limiter.check(peer);
        }

        // Should be banned
        assert!(limiter.is_banned(peer));
        assert!(!limiter.check(peer));

        // Wait for ban to expire
        sleep(Duration::from_millis(150));

        // Should be unbanned
        assert!(!limiter.is_banned(peer));
    }

    #[test]
    fn test_manual_ban() {
        let limiter = RateLimiter::new(RateLimiterConfig::default());
        let peer = "peer1";

        limiter.ban(peer, "Test ban");
        assert!(limiter.is_banned(peer));

        limiter.unban(peer);
        assert!(!limiter.is_banned(peer));
    }
}
