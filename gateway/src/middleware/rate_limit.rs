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
        }
    }
}

// ─── Shared State ───────────────────────────────────────────

/// Shared rate limit state accessible from handlers or middleware.
#[derive(Clone)]
pub struct RateLimitState {
    buckets: Arc<DashMap<String, TokenBucket>>,
    config: RateLimitConfig,
}

impl RateLimitState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Check if a request from the given key and tier is allowed.
    /// Returns (allowed, remaining, limit, retry_after_secs).
    pub fn check(&self, key: &str, tier: RateLimitTier) -> (bool, u32, u32, f64) {
        // Create a composite bucket key: "client_key:tier"
        let tier_name = match tier {
            RateLimitTier::Light => "light",
            RateLimitTier::Standard => "standard",
            RateLimitTier::Heavy => "heavy",
            RateLimitTier::WebSocket => "ws",
        };
        let bucket_key = format!("{}:{}", key, tier_name);

        // Get tier config
        let tier_config = self.config.tiers.get(&tier).cloned().unwrap_or_else(|| {
            // Fallback to standard if tier not configured
            RateLimitTierConfig {
                max_burst: 50,
                requests_per_second: 20.0,
            }
        });

        let mut bucket = self.buckets
            .entry(bucket_key)
            .or_insert_with(|| TokenBucket::new(
                tier_config.max_burst as f64,
                tier_config.requests_per_second,
            ));

        let allowed = bucket.try_consume();
        let remaining = bucket.remaining();
        let retry_after = bucket.retry_after();
        (allowed, remaining, tier_config.max_burst, retry_after)
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
pub fn extract_client_key(headers: &axum::http::HeaderMap) -> String {
    // Check for API key header
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

    // Fallback
    "ip:unknown".to_string()
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
            let (allowed, _, _, _) = state.check("test-client", RateLimitTier::Light);
            assert!(allowed);
        }
        // 201st should be denied
        let (allowed, _, _, _) = state.check("test-client", RateLimitTier::Light);
        assert!(!allowed);

        // Same client can still use Standard tier (separate bucket)
        let (allowed, _, _, _) = state.check("test-client", RateLimitTier::Standard);
        assert!(allowed);
    }

    #[test]
    fn test_rate_limit_state_heavy_tier_lower_limit() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Heavy tier allows only 20 bursts
        for _ in 0..20 {
            let (allowed, _, _, _) = state.check("heavy-client", RateLimitTier::Heavy);
            assert!(allowed);
        }
        let (allowed, _, limit, _) = state.check("heavy-client", RateLimitTier::Heavy);
        assert!(!allowed);
        assert_eq!(limit, 20);
    }

    #[test]
    fn test_rate_limit_different_clients_independent() {
        let config = RateLimitConfig::default();
        let state = RateLimitState::new(config);

        // Exhaust client A's heavy tier
        for _ in 0..20 {
            state.check("client-a", RateLimitTier::Heavy);
        }
        let (allowed, _, _, _) = state.check("client-a", RateLimitTier::Heavy);
        assert!(!allowed);

        // Client B should still be allowed
        let (allowed, _, _, _) = state.check("client-b", RateLimitTier::Heavy);
        assert!(allowed);
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

        let (allowed, remaining, limit, retry_after) = state.check("hdr-client", RateLimitTier::Standard);
        assert!(allowed);
        assert_eq!(limit, 50);
        assert_eq!(remaining, 49);
        assert_eq!(retry_after, 0.0);
    }
}
