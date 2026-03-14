// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Comprehensive Criterion benchmarks for UPP Gateway hot paths.
// Covers: price indexing, arbitrage detection, smart routing, auth, rate limiting, and circuit breakers.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

// ─── Price Index Benchmarks ──────────────────────────────────

fn bench_price_index(c: &mut Criterion) {
    use upp_gateway::core::price_index::{PriceIndex, PriceTick, Resolution};
    use chrono::Utc;

    let index = PriceIndex::new();

    c.bench_function("price_index_ingest_single", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let tick = PriceTick {
                market_id: format!("bench-market-{}", i % 10),
                outcome_id: "yes".to_string(),
                price: 0.55 + (i as f64 % 100.0) * 0.001,
                timestamp: Utc::now(),
            };
            index.ingest(tick);
        });
    });

    let mut group = c.benchmark_group("price_index_ingest_batch");
    for batch_size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |b, &size| {
                let idx = PriceIndex::new();
                let mut counter = 0u64;
                b.iter(|| {
                    for _ in 0..size {
                        counter += 1;
                        let tick = PriceTick {
                            market_id: format!("bench-market-{}", counter % 50),
                            outcome_id: "yes".to_string(),
                            price: 0.50 + (counter as f64 % 50.0) * 0.01,
                            timestamp: Utc::now(),
                        };
                        idx.ingest(tick);
                    }
                });
            },
        );
    }
    group.finish();

    // NEW: Concurrent ingest from multiple markets
    c.bench_function("price_index_concurrent_ingest", |b| {
        let idx = PriceIndex::new();
        let mut i = 0u64;
        b.iter(|| {
            for m in 0..10 {
                for o in 0..5 {
                    i += 1;
                    idx.ingest(PriceTick {
                        market_id: format!("market-{}", m),
                        outcome_id: format!("outcome-{}", o),
                        price: 0.50 + ((i + m + o) as f64 * 0.001) % 0.5,
                        timestamp: Utc::now(),
                    });
                }
            }
        });
    });

    c.bench_function("price_index_query_candles", |b| {
        let idx = PriceIndex::new();
        // Pre-populate with data
        for i in 0..1000u64 {
            let tick = PriceTick {
                market_id: format!("bench-market-{}", i % 5),
                outcome_id: "yes".to_string(),
                price: 0.50 + (i as f64 % 50.0) * 0.01,
                timestamp: Utc::now(),
            };
            idx.ingest(tick);
        }
        b.iter(|| {
            let _ = idx.query_candles("bench-market-0", "yes", Resolution::OneMinute, None, None, 100);
        });
    });

    // NEW: Query with time range filtering
    let mut group = c.benchmark_group("price_index_query_time_range");
    for range_size in [10, 100, 500] {
        group.bench_with_input(
            BenchmarkId::from_parameter(range_size),
            &range_size,
            |b, &range| {
                let idx = PriceIndex::new();
                let base_ts = 1768560000i64;
                for i in 0..1000 {
                    idx.ingest(PriceTick {
                        market_id: "bench-market-0".to_string(),
                        outcome_id: "yes".to_string(),
                        price: 0.50 + (i as f64 % 50.0) * 0.01,
                        timestamp: chrono::DateTime::from_timestamp(base_ts + i * 60, 0).unwrap(),
                    });
                }
                b.iter(|| {
                    let from = Some(base_ts + 100 * 60);
                    let to = Some(base_ts + (100 + range) * 60);
                    let _ = idx.query_candles("bench-market-0", "yes", Resolution::OneMinute, from, to, 1000);
                });
            },
        );
    }
    group.finish();

    // NEW: Latest candle lookup
    c.bench_function("price_index_latest_candle", |b| {
        let idx = PriceIndex::new();
        for i in 0..100 {
            idx.ingest(PriceTick {
                market_id: "bench-market-0".to_string(),
                outcome_id: "yes".to_string(),
                price: 0.50 + (i as f64 * 0.001),
                timestamp: Utc::now(),
            });
        }
        b.iter(|| {
            let _ = idx.latest_candle("bench-market-0", "yes", Resolution::OneMinute);
        });
    });

    c.bench_function("price_index_stats", |b| {
        let idx = PriceIndex::new();
        for i in 0..100u64 {
            idx.ingest(PriceTick {
                market_id: format!("m-{}", i % 10),
                outcome_id: "yes".to_string(),
                price: 0.5,
                timestamp: Utc::now(),
            });
        }
        b.iter(|| {
            let _ = idx.stats();
        });
    });
}

// ─── Arbitrage Scanner Benchmarks ────────────────────────────

fn bench_arbitrage_scanner(c: &mut Criterion) {
    use upp_gateway::core::arbitrage::{ArbitrageScanner, ArbitrageOpportunity};

    c.bench_function("arbitrage_scanner_new", |b| {
        b.iter(|| {
            let _ = ArbitrageScanner::new(0.5, 0.02);
        });
    });

    c.bench_function("arbitrage_get_active_alerts", |b| {
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        b.iter(|| {
            let _ = scanner.get_active_alerts();
        });
    });

    // NEW: process_opportunity benchmark
    c.bench_function("arbitrage_process_opportunity", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scanner = ArbitrageScanner::new(0.5, 0.02);
            let arb = ArbitrageOpportunity {
                description: "Test arbitrage".to_string(),
                bid_provider: "kalshi.com".to_string(),
                bid_price: "0.70".to_string(),
                ask_provider: "polymarket.com".to_string(),
                ask_price: "0.55".to_string(),
                spread_pct: 27.27,
                profit_per_contract: "0.15".to_string(),
            };
            b.to_async(&rt).iter(|| async {
                let _ = scanner.process_opportunity("market-1", "yes", &arb, 100).await;
            });
        });
    });

    // NEW: get_summary benchmark
    c.bench_function("arbitrage_get_summary", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scanner = ArbitrageScanner::new(0.5, 0.02);
            // Pre-populate with alerts
            let arb = ArbitrageOpportunity {
                description: "Test".to_string(),
                bid_provider: "kalshi".to_string(),
                bid_price: "0.70".to_string(),
                ask_provider: "poly".to_string(),
                ask_price: "0.55".to_string(),
                spread_pct: 27.27,
                profit_per_contract: "0.15".to_string(),
            };
            for i in 0..5 {
                let _ = scanner.process_opportunity(&format!("market-{}", i), "yes", &arb, 100).await;
            }

            b.to_async(&rt).iter(|| async {
                let _ = scanner.get_summary().await;
            });
        });
    });

    // NEW: history lookup
    c.bench_function("arbitrage_get_history", |b| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scanner = ArbitrageScanner::new(0.5, 0.02);
            let arb = ArbitrageOpportunity {
                description: "Test".to_string(),
                bid_provider: "k".to_string(),
                bid_price: "0.70".to_string(),
                ask_provider: "p".to_string(),
                ask_price: "0.55".to_string(),
                spread_pct: 27.27,
                profit_per_contract: "0.15".to_string(),
            };
            for i in 0..20 {
                let _ = scanner.process_opportunity(&format!("m-{}", i), "yes", &arb, 50).await;
            }

            b.to_async(&rt).iter(|| async {
                let _ = scanner.get_history(10).await;
            });
        });
    });
}

// ─── Smart Router Benchmarks ─────────────────────────────────

fn bench_smart_router(c: &mut Criterion) {
    use upp_gateway::core::smart_router::{SmartRouter, RoutingStrategy};

    c.bench_function("smart_router_creation", |b| {
        b.iter(|| {
            let _ = SmartRouter::new(0.02);
        });
    });

    c.bench_function("smart_router_stats", |b| {
        let router = SmartRouter::new(0.02);
        b.iter(|| {
            let _ = router.stats();
        });
    });

    // NEW: RoutingStrategy::parse
    let mut group = c.benchmark_group("smart_router_strategy_parse");
    for strategy_str in &["best_price", "split", "direct", "optimal"] {
        group.bench_with_input(
            BenchmarkId::from_parameter(strategy_str),
            strategy_str,
            |b, &s| {
                b.iter(|| {
                    let _ = RoutingStrategy::parse(s);
                });
            },
        );
    }
    group.finish();

    // NEW: Router stats under concurrent load (simulated)
    c.bench_function("smart_router_concurrent_stats", |b| {
        let router = SmartRouter::new(0.02);
        // Simulate prior routing activity
        for _ in 0..100 {
            let _ = router.stats();
        }
        b.iter(|| {
            for _ in 0..10 {
                let _ = router.stats();
            }
        });
    });
}

// ─── Auth Benchmarks ────────────────────────────────────────

fn bench_auth(c: &mut Criterion) {
    use upp_gateway::middleware::auth::{AuthState, AuthConfig, ClientInfo, ClientTier};
    use std::collections::HashMap;

    // Dev mode auth (always allows)
    c.bench_function("auth_dev_mode_check", |b| {
        let auth = AuthState::dev_mode();
        let headers = axum::http::HeaderMap::new();
        b.iter(|| {
            let _ = auth.authenticate(&headers, "/upp/v1/orders");
        });
    });

    // NEW: Production mode auth with valid key
    c.bench_function("auth_production_mode_valid_key", |b| {
        let mut config = AuthConfig::default();
        config.required = true;
        let mut keys = HashMap::new();
        keys.insert("upp_k_test_key_12345".to_string(), ClientInfo {
            client_id: "client-1".to_string(),
            name: "Test Client".to_string(),
            tier: ClientTier::Pro,
            providers: vec!["polymarket".to_string()],
        });
        config.api_keys = keys;
        let auth = AuthState::production(config);

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-API-Key", "upp_k_test_key_12345".parse().unwrap());

        b.iter(|| {
            let _ = auth.authenticate(&headers, "/upp/v1/orders");
        });
    });

    // NEW: JWT validation benchmark
    c.bench_function("auth_jwt_validation", |b| {
        let mut config = AuthConfig::default();
        config.required = true;
        config.jwt_secret = Some("test-secret-key-for-jwt".to_string());
        let auth = AuthState::production(config);

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Authorization", "Bearer invalid.jwt.token".parse().unwrap());

        b.iter(|| {
            let _ = auth.authenticate(&headers, "/upp/v1/orders");
        });
    });

    // NEW: IP allowlist check
    c.bench_function("auth_ip_allowlist_check", |b| {
        let mut config = AuthConfig::default();
        config.required = true;
        config.ip_allowlist = Some(vec![
            "192.168.1.0/24".to_string(),
            "10.0.0.0/8".to_string(),
        ]);
        let auth = AuthState::production(config);

        let headers = axum::http::HeaderMap::new();

        b.iter(|| {
            let _ = auth.authenticate(&headers, "/health");
        });
    });

    // NEW: HMAC signature validation (via API key lookup performance)
    c.bench_function("auth_api_key_lookup_hit", |b| {
        let mut config = AuthConfig::default();
        config.required = true;
        for i in 0..100 {
            config.api_keys.insert(format!("upp_k_key_{}", i), ClientInfo {
                client_id: format!("client-{}", i),
                name: format!("Client {}", i),
                tier: ClientTier::Standard,
                providers: vec![],
            });
        }
        let auth = AuthState::production(config);

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-API-Key", "upp_k_key_50".parse().unwrap());

        b.iter(|| {
            let _ = auth.authenticate(&headers, "/upp/v1/orders");
        });
    });

    c.bench_function("auth_api_key_lookup_miss", |b| {
        let mut config = AuthConfig::default();
        config.required = true;
        for i in 0..100 {
            config.api_keys.insert(format!("upp_k_key_{}", i), ClientInfo {
                client_id: format!("client-{}", i),
                name: format!("Client {}", i),
                tier: ClientTier::Standard,
                providers: vec![],
            });
        }
        let auth = AuthState::production(config);

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-API-Key", "upp_k_nonexistent_999".parse().unwrap());

        b.iter(|| {
            let _ = auth.authenticate(&headers, "/upp/v1/orders");
        });
    });
}

// ─── Rate Limiter Benchmarks ────────────────────────────────

fn bench_rate_limit(c: &mut Criterion) {
    use upp_gateway::middleware::rate_limit::{
        RateLimitState, RateLimitConfig, RateLimitTier, classify_endpoint,
        extract_client_key,
    };

    let config = RateLimitConfig::default();
    let state = RateLimitState::new(config);

    c.bench_function("rate_limit_check", |b| {
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let key = format!("client-{}", i % 100);
            let tier = classify_endpoint("/upp/v1/markets");
            let _ = state.check(&key, tier);
        });
    });

    // NEW: High-contention check (many clients, same tier)
    c.bench_function("rate_limit_high_contention_check", |b| {
        b.iter(|| {
            for i in 0..50 {
                let key = format!("client-{}", i);
                let _ = state.check(&key, RateLimitTier::Standard);
            }
        });
    });

    // NEW: Client override multiplier
    c.bench_function("rate_limit_client_override", |b| {
        let state = RateLimitState::new(RateLimitConfig::default());
        state.set_client_override("enterprise-client", 10.0);
        b.iter(|| {
            let _ = state.check("enterprise-client", RateLimitTier::Standard);
        });
    });

    // NEW: extract_client_key from headers
    c.bench_function("rate_limit_extract_client_key", |b| {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("X-API-Key", "upp_k_test_key".parse().unwrap());
        b.iter(|| {
            let _ = extract_client_key(&headers);
        });
    });

    // NEW: tracked_clients count
    c.bench_function("rate_limit_tracked_clients", |b| {
        let state = RateLimitState::new(RateLimitConfig::default());
        // Pre-populate buckets
        for i in 0..100 {
            let key = format!("client-{}", i);
            let _ = state.check(&key, RateLimitTier::Standard);
        }
        b.iter(|| {
            let _ = state.tracked_clients();
        });
    });

    c.bench_function("rate_limit_classify_endpoint", |b| {
        let endpoints = [
            "/upp/v1/markets", "/upp/v1/orders", "/health",
            "/upp/v1/backtest/run", "/upp/v1/ws", "/metrics",
        ];
        let mut i = 0usize;
        b.iter(|| {
            i = (i + 1) % endpoints.len();
            let _ = classify_endpoint(endpoints[i]);
        });
    });

    // NEW: Cleanup sweep
    let mut group = c.benchmark_group("rate_limit_cleanup");
    for client_count in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::from_parameter(client_count),
            &client_count,
            |b, &count| {
                b.iter(|| {
                    let state = RateLimitState::new(RateLimitConfig::default());
                    for i in 0..count {
                        let key = format!("client-{}", i);
                        let _ = state.check(&key, RateLimitTier::Standard);
                    }
                    // Simulated cleanup: accessing tracked_clients count
                    let _ = state.tracked_clients();
                });
            },
        );
    }
    group.finish();
}

// ─── Circuit Breaker Benchmarks ──────────────────────────────

fn bench_circuit_breaker(c: &mut Criterion) {
    use upp_gateway::core::hardening::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerRegistry};

    // NEW: CircuitBreaker creation
    c.bench_function("circuit_breaker_creation", |b| {
        let config = CircuitBreakerConfig::default();
        b.iter(|| {
            let _ = CircuitBreaker::new(config.clone());
        });
    });

    // NEW: record_success
    c.bench_function("circuit_breaker_record_success", |b| {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
        b.iter(|| {
            breaker.record_success();
        });
    });

    // NEW: record_failure
    c.bench_function("circuit_breaker_record_failure", |b| {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
        b.iter(|| {
            breaker.record_failure();
        });
    });

    // NEW: State transitions (closed -> open -> half-open -> closed)
    c.bench_function("circuit_breaker_state_transitions", |b| {
        b.iter(|| {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
            // Trip the circuit
            for _ in 0..5 {
                breaker.record_failure();
            }
            // Check state transition
            let _ = breaker.get_state();
        });
    });

    // NEW: registry get/get_or_create
    c.bench_function("circuit_breaker_registry_get_or_create", |b| {
        let registry = CircuitBreakerRegistry::new(CircuitBreakerConfig::default());
        let mut i = 0;
        b.iter(|| {
            i = (i + 1) % 10;
            let provider = format!("provider-{}", i);
            let _ = registry.get_or_create(&provider);
        });
    });

    // NEW: registry get (after populated)
    c.bench_function("circuit_breaker_registry_get", |b| {
        let registry = CircuitBreakerRegistry::new(CircuitBreakerConfig::default());
        // Pre-populate
        for i in 0..20 {
            let _ = registry.get_or_create(&format!("provider-{}", i));
        }
        let mut i = 0;
        b.iter(|| {
            i = (i + 1) % 20;
            let _ = registry.get(&format!("provider-{}", i));
        });
    });

    // NEW: Concurrent failure recording leading to circuit open
    c.bench_function("circuit_breaker_concurrent_failures", |b| {
        b.iter(|| {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
            for _ in 0..10 {
                breaker.record_failure();
                let _ = breaker.get_state();
            }
        });
    });

    // NEW: Recovery cycle (open -> half-open -> closed)
    c.bench_function("circuit_breaker_recovery_cycle", |b| {
        b.iter(|| {
            let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
            // Trip open
            for _ in 0..5 {
                breaker.record_failure();
            }
            let _ = breaker.get_state();
            // In half-open, record successes
            for _ in 0..3 {
                breaker.record_success();
            }
            let _ = breaker.get_state();
        });
    });
}

// ─── Group Registration ──────────────────────────────────────

criterion_group!(
    benches,
    bench_price_index,
    bench_arbitrage_scanner,
    bench_smart_router,
    bench_auth,
    bench_rate_limit,
    bench_circuit_breaker,
);
criterion_main!(benches);
