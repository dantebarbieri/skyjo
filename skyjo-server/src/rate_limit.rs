use dashmap::DashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// A single token bucket for one IP/resource combination.
#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one token. Returns true if allowed, false if rate limited.
    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Rate limiter configuration for different endpoint types.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_tokens: f64,
    pub refill_rate: f64,
}

impl RateLimitConfig {
    pub const fn new(max_tokens: f64, refill_rate: f64) -> Self {
        Self {
            max_tokens,
            refill_rate,
        }
    }
}

/// Predefined rate limit configurations.
pub mod limits {
    use super::RateLimitConfig;

    /// Room creation: 5 per minute (burst 5, refill ~0.083/sec)
    pub const ROOM_CREATION: RateLimitConfig = RateLimitConfig::new(5.0, 5.0 / 60.0);

    /// Room joining: 10 per minute
    pub const ROOM_JOIN: RateLimitConfig = RateLimitConfig::new(10.0, 10.0 / 60.0);

    /// WebSocket messages: 30 per second (burst 30)
    pub const WS_MESSAGE: RateLimitConfig = RateLimitConfig::new(30.0, 30.0);

    /// Genetic API: 1 per 10 seconds
    pub const GENETIC_API: RateLimitConfig = RateLimitConfig::new(1.0, 0.1);
}

/// Per-IP rate limiter using token buckets.
pub struct RateLimiter {
    buckets: DashMap<(IpAddr, &'static str), TokenBucket>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: DashMap::new(),
        }
    }

    /// Check if the given IP is allowed for the given resource.
    /// Returns true if allowed, false if rate-limited.
    pub fn check(&self, ip: IpAddr, resource: &'static str, config: &RateLimitConfig) -> bool {
        let key = (ip, resource);
        let mut entry = self
            .buckets
            .entry(key)
            .or_insert_with(|| TokenBucket::new(config.max_tokens, config.refill_rate));
        entry.try_consume()
    }

    /// Clean up stale entries (call periodically). Removes buckets that have been full for a while.
    pub fn cleanup(&self, max_idle: Duration) {
        let now = Instant::now();
        self.buckets
            .retain(|_, bucket| now.duration_since(bucket.last_refill) < max_idle);
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
    use std::net::{IpAddr, Ipv4Addr};
    use std::thread;

    const TEST_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    const TEST_IP2: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

    #[test]
    fn token_bucket_allows_within_limit() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(5.0, 1.0);

        // Should allow 5 requests (burst)
        for _ in 0..5 {
            assert!(limiter.check(TEST_IP, "test", &config));
        }
    }

    #[test]
    fn token_bucket_blocks_over_limit() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(3.0, 0.0001); // Very slow refill

        // Exhaust tokens
        for _ in 0..3 {
            assert!(limiter.check(TEST_IP, "test", &config));
        }

        // Should be blocked
        assert!(!limiter.check(TEST_IP, "test", &config));
    }

    #[test]
    fn token_bucket_refills_over_time() {
        let limiter = RateLimiter::new();
        // 1 token, refills 10 per second
        let config = RateLimitConfig::new(1.0, 10.0);

        // Consume the token
        assert!(limiter.check(TEST_IP, "test", &config));
        assert!(!limiter.check(TEST_IP, "test", &config));

        // Wait for refill
        thread::sleep(Duration::from_millis(200));

        // Should have tokens again
        assert!(limiter.check(TEST_IP, "test", &config));
    }

    #[test]
    fn different_ips_have_independent_limits() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(1.0, 0.0001);

        assert!(limiter.check(TEST_IP, "test", &config));
        assert!(!limiter.check(TEST_IP, "test", &config));

        // Different IP should still have tokens
        assert!(limiter.check(TEST_IP2, "test", &config));
    }

    #[test]
    fn different_resources_have_independent_limits() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(1.0, 0.0001);

        assert!(limiter.check(TEST_IP, "resource_a", &config));
        assert!(!limiter.check(TEST_IP, "resource_a", &config));

        // Different resource should still have tokens
        assert!(limiter.check(TEST_IP, "resource_b", &config));
    }

    #[test]
    fn cleanup_removes_stale_entries() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig::new(5.0, 1.0);

        limiter.check(TEST_IP, "test", &config);
        assert!(!limiter.buckets.is_empty());

        // Cleanup with 0 idle time should remove everything
        limiter.cleanup(Duration::ZERO);
        assert!(limiter.buckets.is_empty());
    }

    #[test]
    fn predefined_limits_compile() {
        // Just verify the constants are valid
        let _ = limits::ROOM_CREATION;
        let _ = limits::ROOM_JOIN;
        let _ = limits::WS_MESSAGE;
        let _ = limits::GENETIC_API;
    }
}
