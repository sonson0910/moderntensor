//! Rate limiter for RPC endpoints
//! Prevents DoS attacks by limiting requests per IP

use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tracing::warn;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window for rate limiting
    pub window: Duration,
    /// Cleanup interval for expired entries
    pub cleanup_interval: Duration,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            max_requests: 100,           // 100 requests
            window: Duration::from_secs(60), // per minute
            cleanup_interval: Duration::from_secs(300), // cleanup every 5 min
        }
    }
}

/// Per-IP request tracking
struct RequestTracker {
    count: u32,
    window_start: Instant,
}

/// Rate limiter for IP-based request limiting
pub struct RateLimiter {
    config: RateLimiterConfig,
    requests: RwLock<HashMap<IpAddr, RequestTracker>>,
    last_cleanup: RwLock<Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter with default config
    pub fn new() -> Self {
        Self::with_config(RateLimiterConfig::default())
    }

    /// Create rate limiter with custom config
    pub fn with_config(config: RateLimiterConfig) -> Self {
        Self {
            config,
            requests: RwLock::new(HashMap::new()),
            last_cleanup: RwLock::new(Instant::now()),
        }
    }

    /// Check if request from IP should be allowed
    /// Returns true if allowed, false if rate limited
    pub fn check(&self, ip: IpAddr) -> bool {
        self.maybe_cleanup();

        let mut requests = self.requests.write();
        let now = Instant::now();

        let tracker = requests.entry(ip).or_insert(RequestTracker {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(tracker.window_start) >= self.config.window {
            tracker.count = 0;
            tracker.window_start = now;
        }

        // Check rate limit
        if tracker.count >= self.config.max_requests {
            warn!("🚫 Rate limited IP: {} ({} requests in {:?})",
                  ip, tracker.count, self.config.window);
            return false;
        }

        tracker.count += 1;
        true
    }

    /// Check and return remaining quota
    pub fn check_with_remaining(&self, ip: IpAddr) -> (bool, u32) {
        self.maybe_cleanup();

        let mut requests = self.requests.write();
        let now = Instant::now();

        let tracker = requests.entry(ip).or_insert(RequestTracker {
            count: 0,
            window_start: now,
        });

        if now.duration_since(tracker.window_start) >= self.config.window {
            tracker.count = 0;
            tracker.window_start = now;
        }

        let remaining = self.config.max_requests.saturating_sub(tracker.count);

        if tracker.count >= self.config.max_requests {
            return (false, 0);
        }

        tracker.count += 1;
        (true, remaining.saturating_sub(1))
    }

    /// Cleanup expired entries
    fn maybe_cleanup(&self) {
        let now = Instant::now();
        let should_cleanup = {
            let last = self.last_cleanup.read();
            now.duration_since(*last) >= self.config.cleanup_interval
        };

        if should_cleanup {
            let mut requests = self.requests.write();
            let window = self.config.window;

            requests.retain(|_, tracker| {
                now.duration_since(tracker.window_start) < window
            });

            *self.last_cleanup.write() = now;
        }
    }

    /// Get current request count for IP
    pub fn get_count(&self, ip: IpAddr) -> u32 {
        self.requests.read()
            .get(&ip)
            .map(|t| t.count)
            .unwrap_or(0)
    }

    /// Reset rate limit for an IP (for testing)
    pub fn reset(&self, ip: IpAddr) {
        self.requests.write().remove(&ip);
    }

    /// Clear all rate limits
    pub fn clear(&self) {
        self.requests.write().clear();
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_rate_limiter_allows_requests() {
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            max_requests: 5,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        });

        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // First 5 requests should pass
        for _ in 0..5 {
            assert!(limiter.check(ip));
        }

        // 6th request should fail
        assert!(!limiter.check(ip));
    }

    #[test]
    fn test_rate_limiter_different_ips() {
        let limiter = RateLimiter::with_config(RateLimiterConfig {
            max_requests: 2,
            window: Duration::from_secs(60),
            cleanup_interval: Duration::from_secs(300),
        });

        let ip1 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2));

        // Both IPs should have their own limits
        assert!(limiter.check(ip1));
        assert!(limiter.check(ip1));
        assert!(!limiter.check(ip1)); // rate limited

        assert!(limiter.check(ip2)); // different IP, should work
        assert!(limiter.check(ip2));
        assert!(!limiter.check(ip2)); // rate limited
    }
}
