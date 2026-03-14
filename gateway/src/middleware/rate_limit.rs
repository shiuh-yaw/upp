// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Token-bucket rate limiter middleware with configurable multi-tier support.
//
// Supports per-IP and per-API-key rate limiting with per-endpoint rate classes,
// configurable burst capacity and refill rate. Uses DashMap for lock-free
// concurrent access across Tokio tasks.

#![allow(dead_code)]

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};


// ─── Rate Limit Tiers ───────────────────────────────────────

/// Classification of endpoints by resource consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitTier {
    /// Lightweight endpoints: /health, /metrics, /.well-known/*
    Light,
    /// Standard API: /markets, /search, /categories, /mcp/tools, /discovery
    Standard,
    /// Heavy compute: /mcp/execute, /orders/estimate
    Heavy,
    /// WebSocket upgrades
    WebSocket,
}

/// Per-tier rate limit configuration.
#[derive(Debug, Clone)]
pub struct RateLimitTierConfig {
    /// Maximum burst size (tokens).
    pub max_burst: u32,
    /// Sustained requests per second.
    pub requests_per_second: f64,
}

/// Result of a rate limit check.
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Remaining tokens after this request.
    pub remaining: u32,
    /// Total limit for this tier.
    pub limit: u32,
    /// Seconds to wait before next request is allowed (0 if allowed).
    pub retry_after: f64,
}

// ─── Token Bucket ───────────────────────────────────────────

/// A single token bucket for one client (IP or API key).
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

    /// Try to consume one token. Returns true if allowed.
    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill = now;
    }

    /// Seconds until the next token is available.
    fn retry_after(&self) -> f64 {
        if self.tokens >= 1.0 {
            return 0.0;
        }
        (1.0 - self.tokens) / self.refill_rate
    }

    /// Remaining tokens (floored to integer for headers).
    fn remaining(&self) -> u32 {
        self.tokens.floor().max(0.0) as u32
    }
}

// ─── Rate Limit Config ──────────────────────────────────────

/// Configuration for the multi-tier rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Per-tier rate limit configurations.
    pub tiers: HashMap<RateLimitTier, RateLimitTierConfig>,
    /// How often to clean up expired buckets (seconds).
    pub cleanup_interval_secs: u64,
    /// Time after which an idle bucket is removed (seconds).
    pub bucket_expiry_secs: u64,
    /// Optional Redis URL for distributed rate limiting.
    pub redis_url: Option<String>,
    /// Use sliding window limiter instead of token bucket.
    pub use_sliding_window: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut tiers = HashMap::new();
        tiers.insert(
            RateLimitTier::Light,
            RateLimitTierConfig {
                max_burst: 200,
                requests_per_second: 100.0,
            },
        );
        tiers.insert(
            RateLimitTier::Standard,
            RateLimitTierConfig {
                max_burst: 50,
                requests_per_second: 20.0,
            },
        );
        tiers.insert(
            RateLimitTier::Heavy,
            RateLimitTierConfig {
                max_burst: 20,
                requests_per_second: 5.0,
            },
        );
        tiers.insert(
            RateLimitTier::WebSocket,
            RateLimitTierConfig {
                max_burst: 10,
                requests_per_second: 2.0,
            },
        );
        Self {
            tiers,
            cleanup_interval_secs: 60,
            bucket_expiry_secs: 300,
            redis_url: None,
            use_sliding_window: false,
        }
    }
}

// ─── Redis Rate Limiter ─────────────────────────────────────

/// Redis-backed rate limiter using INCR + EXPIRE for sliding window.
#[derive(Clone)]
pub struct RedisRateLimiter {
    client: Arc<Option<redis::Client>>,
    window_secs: usize,
}

impl RedisRateLimiter {
    /// Create a new Redis-backed rate limiter.
    /// Returns None if Redis connection fails.
    pub fn new(redis_url: &str, window_secs: usize) -> Option<Self> {
        match redis::Client::open(redis_url) {
            Ok(client) => {
                // Test the connection
                match client.get_connection() {
                    Ok(_) => {
                        debug!("Redis rate limiter connected to {}", redis_url);
                        Some(Self {
                            client: Arc::new(Some(client)),
                            window_secs,
                        })
                    }
                    Err(e) => {
                        warn!("Failed to connect to Redis: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                warn!("Failed to create Redis client: {}", e);
                None
            }
        }
    }

    /// Check if a request is allowed using sliding window (INCR + EXPIRE).
    /// Returns (allowed: bool, current_count: u32, limit: u32, retry_after: f64)
    pub fn check(&self, key: &str, limit: u32) -> (bool, u32, u32, f64) {
        if let Some(ref client) = *self.client {
            if let Ok(mut conn) = client.get_connection() {
                use redis::Commands;

                // Use a key with timestamp-based window
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let window_key = format!("ratelimit:{}:{}", key, now / self.window_secs as u64);

                // INCR the counter
                let current: u32 = match conn.incr(&window_key, 1) {
                    Ok(count) => count,
                    Err(e) => {
                        warn!("Redis INCR failed: {}", e);
                        return (false, 0, limit, 0.0);
                    }
                };

                // Set expiration on first increment
                if current == 1 {
                    let _: bool = conn.expire(&window_key, self.window_secs as i64).unwrap_or(false);
                }

                let allowed = current <= limit;
                let _remaining = if allowed { limit - current } else { 0 };

                (allowed, current, limit, if allowed { 0.0 } else { self.window_secs as f64 })
            } else {
                // Connection error, fall back to denied (safe default)
                (false, 0, limit, 0.0)
            }
        } else {
            // No Redis client
            (false, 0, limit, 0.0)
        }
    }
}

// ─── Shared State ───────────────────────────────────────────

/// Shared rate limit state accessible from handlers or middleware.
/// Supports both in-memory and Redis-backed limiting.
#[derive(Clone)]
pub struct RateLimitState {
    buckets: Arc<DashMap<String, TokenBucket>>,
    config: RateLimitConfig,
    /// Per-client tier overrides (for Enterprise clients).
    client_tier_overrides: Arc<DashMap<String, f64>>,
    /// Optional Redis-backed limiter (fallback to in-memory if None).
    redis_limiter: Arc<Option<RedisRateLimiter>>,
}

impl RateLimitState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
            client_tier_overrides: Arc::new(DashMap::new()),
            redis_limiter: Arc::new(None),
        }
    }

    /// Create a new rate limit state with Redis backing.
    pub fn new_with_redis(config: RateLimitConfig, redis_url: &str) -> Self {
        let redis_limiter = RedisRateLimiter::new(redis_url, 60);
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
            client_tier_overrides: Arc::new(DashMap::new()),
            redis_limiter: Arc::new(redis_limiter),
        }
    }

    /// Set a rate limit multiplier override for a specific client.
    /// This is used to give Enterprise clients higher limits (e.g., 10x).
    pub fn set_client_override(&self, client_key: &str, multiplier: f64) {
        self.client_tier_overrides.insert(client_key.to_string(), multiplier);
    }

    /// Remove a rate limit override for a client.
    pub fn remove_client_override(&self, client_key: &str) {
        self.client_tier_overrides.remove(client_key);
    }

    /// Check if a request from the given key and tier is allowed.
    /// Returns RateLimitResult with detailed information.
    /// Tries Redis first if available, falls back to in-memory token bucket.
    pub fn check(&self, key: &str, tier: RateLimitTier) -> RateLimitResult {
        // Create a composite bucket key: "client_key:tier"
        let tier_name = match tier {
            RateLimitTier::Light => "light",
            RateLimitTier::Standard => "standard",
            RateLimitTier::Heavy => "heavy",
            RateLimitTier::WebSocket => "ws",
        };
        let bucket_key = format!("{}:{}", key, tier_name);

        // Get tier config
        let mut tier_config = self.config.tiers.get(&tier).cloned().unwrap_or_else(|| {
            // Fallback to standard if tier not configured
            RateLimitTierConfig {
                max_burst: 50,
                requests_per_second: 20.0,
            }
        });

        // Apply client-specific override if present
        if let Some(override_entry) = self.client_tier_overrides.get(key) {
            let multiplier = *override_entry;
            tier_config.max_burst = (tier_config.max_burst as f64 * multiplier).ceil() as u32;
            tier_config.requests_per_second *= multiplier;
        }

        // Try Redis if available
        if let Some(ref redis_limiter) = *self.redis_limiter {
            let (_allowed, _current, _limit, _retry) = redis_limiter.check(&bucket_key, tier_config.max_burst);
            // Note: Redis sliding window is simpler; we use max_burst as the limit
            // For production, you might want more sophisticated logic
            debug!("Redis rate limit check: key={}, allowed={}", bucket_key, _allowed);
            return RateLimitResult {
                allowed: _allowed,
                remaining: if _allowed { tier_config.max_burst - _current } else { 0 },
                limit: tier_config.max_burst,
                retry_after: _retry,
            };
        }

        // Fall back to in-memory token bucket
        let mut bucket = self.buckets
            .entry(bucket_key)
            .or_insert_with(|| TokenBucket::new(
                tier_config.max_burst as f64,
                tier_config.requests_per_second,
            ));

        let allowed = bucket.try_consume();
        let remaining = bucket.remaining();
        let retry_after = bucket.retry_after();

        RateLimitResult {
            allowed,
            remaining,
            limit: tier_config.max_burst,
            retry_after,
        }
    }

    /// Check rate limit using old tuple return format for backward compatibility.
    pub fn check_legacy(&self, key: &str, tier: RateLimitTier) -> (bool, u32, u32, f64) {
        let result = self.check(key, tier);
        (result.allowed, result.remaining, result.limit, result.retry_after)
    }

    /// Start a background task that periodically removes idle buckets.
    pub fn start_cleanup(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let state = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(state.config.cleanup_interval_secs),
            );
            loop {
                interval.tick().await;
                let expiry = Duration::from_secs(state.config.bucket_expiry_secs);
                let now = Instant::now();
                let before = state.buckets.len();
                state.buckets.retain(|_key, bucket| {
                    now.duration_since(bucket.last_refill) < expiry
                });
                let removed = before - state.buckets.len();
                if removed > 0 {
                    tracing::debug!(removed = removed, "Cleaned up idle rate limit buckets");
                }
            }
        })
    }

    /// Get the number of tracked clients.
    pub fn tracked_clients(&self) -> usize {
        self.buckets.len()
    }
}

/// Extract a rate-limit key from request headers.
/// Priority: X-API-Key > Authorization Bearer > X-Forwarded-For > fallback
/// Returns the full key (e.g., "apikey:...", "bearer:...", "ip:...").
pub fn extract_client_key(headers: &axum::http::HeaderMap) -> String {
    // Check for API key header (preferred)
    if let Some(key) = headers.get("X-API-Key").and_then(|v| v.to_str().ok()) {
        return format!("apikey:{}", key);
    }

    // Check for Bearer token
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            let short = &token[..token.len().min(16)];
            return format!("bearer:{}", short);
        }
    }

    // Check X-Forwarded-For (first IP in chain)
    if let Some(xff) = headers.get("X-Forwarded-For").and_then(|v| v.to_str().ok()) {
        if let Some(first_ip) = xff.split(',').next() {
            return format!("ip:{}", first_ip.trim());
        }
    }

    // Fallback to X-Real-IP
    if let Some(real_ip) = headers.get("X-Real-IP").and_then(|v| v.to_str().ok()) {
        return format!("ip:{}", real_ip);
    }

    // Fallback
    "ip:unknown".to_string()
}

/// Extract API key from headers (if present).
pub fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    headers.get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Extract Bearer token from headers (if present).
pub fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers.get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|auth| auth.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

/// Extract client IP from headers.
/// Priority: X-Forwarded-For > X-Real-IP
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> Option<String> {
    // Check X-Forwarded-For (first IP in chain)
    if let Some(xff) = headers.get("X-Forwarded-For").and_then(|v| v.to_str().ok()) {
        if let Some(first_ip) = xff.split(',').next() {
            return Some(first_ip.trim().to_string());
        }
    }

    // Fallback to X-Real-IP
    if let Some(real_ip) = headers.get("X-Real-IP").and_then(|v| v.to_str().ok()) {
        return Some(real_ip.to_string());
    }

    None
}

/// Classify a request path into a rate limit tier.
pub fn classify_endpoint(path: &str) -> RateLimitTier {
    // Light tier: infrastructure & metadata endpoints
    if path == "/health"
        || path == "/ready"
        || path.starts_with("/metrics")
        || path.starts_with("/.well-known/") {
        return RateLimitTier::Light;
    }

    // Heavy tier: compute-intensive operations
    if path.contains("/mcp/execute")
        || path.contains("/orders/estimate") {
        return RateLimitTier::Heavy;
    }

    // WebSocket tier: streaming connections
    if path.contains("/ws") || path.ends_with("/upgrade") {
        return RateLimitTier::WebSocket;
    }

    // Standard tier: everything else (markets, search, discovery, portfolio, trading)
    RateLimitTier::Standard
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_endpoint_light() {
        assert_eq!(classify_endpoint("/health"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/ready"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/metrics"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/metrics/prometheus"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/.well-known/agent.json"), RateLimitTier::Light);
    }

    #[test]
    fn test_classify_endpoint_heavy() {
        assert_eq!(classify_endpoint("/upp/v1/mcp/execute"), RateLimitTier::Heavy);
        assert_eq!(classify_endpoint("/upp/v1/orders/estimate"), RateLimitTier::Heavy);
    }

    #[test]
    fn test_classify_endpoint_websocket() {
        assert_eq!(classify_endpoint("/upp/v1/ws"), RateLimitTier::WebSocket);
        assert_eq!(classify_endpoint("/upp/v1/ws/upgrade"), RateLimitTier::WebSocket);
    }

    #[test]
    fn test_classify_endpoint_standard() {
        assert_eq!(classify_endpoint("/upp/v1/markets"), RateLimitTier::Standard);
        assert_eq!(classify_endpoint("/upp/v1/markets/search"), RateLimitTier::Standard);
        assert_eq!(classify_endpoint("/upp/v1/markets/categories"), RateLimitTier::Standard);
        assert_eq!(classify_endpoint("/upp/v1/discovery/providers"), RateLimitTier::Standard);
        assert_eq!(classify_endpoint("/upp/v1/mcp/tools"), RateLimitTier::Standard);
    }

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(3.0, 1.0);
        assert!(bucket.try_consume()); // 1st
        assert!(bucket.try_consume()); // 2nd
        assert!(bucket.try_consume()); // 3rd — bucket empty
        assert!(!bucket.try_consume()); // 4th — denied
    }

    #[test]
    fn test_token_bucket_remaining() {
        let bucket = TokenBucket::new(10.0, 5.0);
        assert_eq!(bucket.remaining(), 10);
    }

    #[test]
    fn test_token_bucket_retry_after() {
        let mut bucket = TokenBucket::new(1.0, 1.0);
        bucket.try_consume(); // use the only token
        let retry = bucket.retry_after();
        assert!(retry > 0.0 && retry <= 1.0);
    }

    #[test]
    fn test_rate_limit_state_multi_tier() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Light tier should allow 200 bursts
        for _ in 0..200 {
            let result = state.check("test-client", RateLimitTier::Light);
            assert!(result.allowed);
        }
        // 201st should be denied
        let result = state.check("test-client", RateLimitTier::Light);
        assert!(!result.allowed);

        // Same client can still use Standard tier (separate bucket)
        let result = state.check("test-client", RateLimitTier::Standard);
        assert!(result.allowed);
    }

    #[test]
    fn test_rate_limit_state_heavy_tier_lower_limit() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Heavy tier allows only 20 bursts
        for _ in 0..20 {
            let result = state.check("heavy-client", RateLimitTier::Heavy);
            assert!(result.allowed);
        }
        let result = state.check("heavy-client", RateLimitTier::Heavy);
        assert!(!result.allowed);
        assert_eq!(result.limit, 20);
    }

    #[test]
    fn test_rate_limit_different_clients_independent() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Exhaust client A's heavy tier
        for _ in 0..20 {
            state.check("client-a", RateLimitTier::Heavy);
        }
        let result = state.check("client-a", RateLimitTier::Heavy);
        assert!(!result.allowed);

        // Client B should still be allowed
        let result = state.check("client-b", RateLimitTier::Heavy);
        assert!(result.allowed);
    }

    #[test]
    fn test_tracked_clients() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        state.check("client-1", RateLimitTier::Light);
        state.check("client-2", RateLimitTier::Standard);
        state.check("client-1", RateLimitTier::Heavy);

        // client-1 has 2 buckets (light + heavy), client-2 has 1
        assert_eq!(state.tracked_clients(), 3);
    }

    #[test]
    fn test_rate_limit_headers() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        let result = state.check("hdr-client", RateLimitTier::Standard);
        assert!(result.allowed);
        assert_eq!(result.limit, 50);
        assert_eq!(result.remaining, 49);
        assert_eq!(result.retry_after, 0.0);
    }
}
