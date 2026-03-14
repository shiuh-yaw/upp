// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Redis rate limiter integration tests.
// These tests require a running Redis instance and are gated behind #[ignore].
// Run with: cargo test --test redis_rate_limit_test -- --ignored --nocapture

use upp_gateway::middleware::rate_limit::RedisRateLimiter;
use std::time::Duration;

/// Redis URL for testing. Override with REDIS_URL environment variable.
fn get_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

#[tokio::test]
#[ignore]
async fn redis_rate_limit_basic() {
    let redis_url = get_redis_url();

    let limiter = match RedisRateLimiter::new(&redis_url, 60) {
        Some(limiter) => limiter,
        None => {
            eprintln!("Skipping test: Redis not available at {}", redis_url);
            return;
        }
    };

    let limit = 10;
    let key = format!("test:basic:{}", uuid::Uuid::new_v4());

    // First request should be allowed
    let (allowed, count, limit_result, _retry) = limiter.check(&key, limit);
    assert!(allowed, "First request should be allowed");
    assert_eq!(count, 1, "Count should be 1 after first request");
    assert_eq!(limit_result, limit, "Limit should match configured limit");

    // Second request should also be allowed
    let (allowed, count, _, _) = limiter.check(&key, limit);
    assert!(allowed, "Second request should be allowed");
    assert_eq!(count, 2, "Count should be 2 after second request");
}

#[tokio::test]
#[ignore]
async fn redis_rate_limit_exceeded() {
    let redis_url = get_redis_url();

    let limiter = match RedisRateLimiter::new(&redis_url, 60) {
        Some(limiter) => limiter,
        None => {
            eprintln!("Skipping test: Redis not available at {}", redis_url);
            return;
        }
    };

    let limit = 5;
    let key = format!("test:exceeded:{}", uuid::Uuid::new_v4());

    // Fill up the limit
    for i in 1..=limit {
        let (allowed, count, _, _) = limiter.check(&key, limit);
        assert!(allowed, "Request {} should be allowed", i);
        assert_eq!(count as u32, i, "Count should be {}", i);
    }

    // Next request should be denied
    let (allowed, count, limit_result, retry_after) = limiter.check(&key, limit);
    assert!(!allowed, "Request after limit should be denied");
    assert_eq!(count, (limit + 1) as u32, "Count should exceed limit");
    assert_eq!(limit_result, limit, "Limit should still be configured value");
    assert!(
        retry_after > 0.0,
        "retry_after should indicate time to wait"
    );
}

#[tokio::test]
#[ignore]
async fn redis_rate_limit_window_reset() {
    let redis_url = get_redis_url();

    // Use a 1-second window for faster testing
    let window_secs = 1;
    let limiter = match RedisRateLimiter::new(&redis_url, window_secs) {
        Some(limiter) => limiter,
        None => {
            eprintln!("Skipping test: Redis not available at {}", redis_url);
            return;
        }
    };

    let limit = 3;
    let key = format!("test:reset:{}", uuid::Uuid::new_v4());

    // Fill the limit in the first window
    for _ in 0..limit {
        let (allowed, _, _, _) = limiter.check(&key, limit);
        assert!(allowed, "Request should be allowed within window");
    }

    // Exceed limit
    let (allowed, _, _, _) = limiter.check(&key, limit);
    assert!(!allowed, "Request should be denied when limit exceeded");

    // Wait for window to expire
    tokio::time::sleep(Duration::from_millis(1100)).await;

    // Counter should reset after window expires
    let (allowed, count, _, _) = limiter.check(&key, limit);
    assert!(allowed, "Request should be allowed after window reset");
    assert_eq!(
        count, 1,
        "Count should be 1 at start of new window (Redis key expired and recreated)"
    );
}

#[tokio::test]
#[ignore]
async fn redis_rate_limit_different_keys() {
    let redis_url = get_redis_url();

    let limiter = match RedisRateLimiter::new(&redis_url, 60) {
        Some(limiter) => limiter,
        None => {
            eprintln!("Skipping test: Redis not available at {}", redis_url);
            return;
        }
    };

    let limit = 3;
    let key1 = format!("test:key1:{}", uuid::Uuid::new_v4());
    let key2 = format!("test:key2:{}", uuid::Uuid::new_v4());

    // Exhaust limit for key1
    for _ in 0..limit {
        let (allowed, _, _, _) = limiter.check(&key1, limit);
        assert!(allowed, "Request to key1 should be allowed within limit");
    }

    // key1 should be exhausted
    let (allowed, _, _, _) = limiter.check(&key1, limit);
    assert!(!allowed, "Request to key1 should be denied after limit");

    // key2 should still have full limit available (independent buckets)
    let (allowed, count, _, _) = limiter.check(&key2, limit);
    assert!(allowed, "Request to key2 should be allowed (different key)");
    assert_eq!(
        count, 1,
        "key2 should have its own independent counter starting at 1"
    );

    // key2 should have room for more requests
    for _ in 1..limit {
        let (allowed, _, _, _) = limiter.check(&key2, limit);
        assert!(allowed, "key2 should have independent limit");
    }

    // key2 should now be exhausted too
    let (allowed, _, _, _) = limiter.check(&key2, limit);
    assert!(!allowed, "key2 should be denied after its own limit is reached");
}

#[tokio::test]
#[ignore]
async fn redis_rate_limit_connection_failure() {
    // Try to connect to an invalid Redis URL
    let invalid_url = "redis://invalid-host-that-does-not-exist:6379";

    let limiter = RedisRateLimiter::new(invalid_url, 60);
    assert!(
        limiter.is_none(),
        "Should return None when connection fails"
    );
}

#[tokio::test]
#[ignore]
async fn redis_rate_limit_high_volume() {
    let redis_url = get_redis_url();

    let limiter = match RedisRateLimiter::new(&redis_url, 60) {
        Some(limiter) => limiter,
        None => {
            eprintln!("Skipping test: Redis not available at {}", redis_url);
            return;
        }
    };

    let limit = 100;
    let key = format!("test:highvolume:{}", uuid::Uuid::new_v4());

    // Send 100 requests
    for i in 1..=limit {
        let (allowed, count, _, _) = limiter.check(&key, limit);
        assert!(allowed, "Request {} should be allowed", i);
        assert_eq!(count as u32, i, "Count mismatch at request {}", i);
    }

    // 101st request should be denied
    let (allowed, _count, _, _) = limiter.check(&key, limit);
    assert!(!allowed, "Request 101 should be denied");
}
