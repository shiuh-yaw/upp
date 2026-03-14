// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Criterion benchmarks for UPP Gateway hot paths.

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
    use upp_gateway::core::arbitrage::ArbitrageScanner;

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
}

// ─── Smart Router Benchmarks ─────────────────────────────────

fn bench_smart_router(c: &mut Criterion) {
    use upp_gateway::core::smart_router::SmartRouter;

    c.bench_function("smart_router_stats", |b| {
        let router = SmartRouter::new(0.02);
        b.iter(|| {
            let _ = router.stats();
        });
    });
}

// ─── Auth & Rate Limit Benchmarks ────────────────────────────

fn bench_auth(c: &mut Criterion) {
    use upp_gateway::middleware::auth::{AuthState, ApiKeyManager, CreateApiKeyRequest, ClientTier};

    c.bench_function("auth_dev_mode_check", |b| {
        let auth = AuthState::dev_mode();
        let headers = axum::http::HeaderMap::new();
        b.iter(|| {
            let _ = auth.authenticate(&headers, "/upp/v1/orders");
        });
    });

    c.bench_function("api_key_manager_create", |b| {
        let mgr = ApiKeyManager::new();
        let mut i = 0u64;
        b.iter(|| {
            i += 1;
            let _ = mgr.create_key(CreateApiKeyRequest {
                client_name: format!("bench-client-{}", i),
                tier: Some(ClientTier::Pro),
                expires_in_days: Some(90),
                providers: None,
                label: None,
            });
        });
    });

    c.bench_function("api_key_manager_authenticate_hit", |b| {
        let mgr = ApiKeyManager::new();
        let resp = mgr.create_key(CreateApiKeyRequest {
            client_name: "bench-client".to_string(),
            tier: Some(ClientTier::Pro),
            expires_in_days: Some(90),
            providers: None,
            label: None,
        });
        let key = resp.key.clone();
        b.iter(|| {
            let _ = mgr.authenticate_key(&key);
        });
    });

    c.bench_function("api_key_manager_authenticate_miss", |b| {
        let mgr = ApiKeyManager::new();
        b.iter(|| {
            let _ = mgr.authenticate_key("upp_k_nonexistent_key_12345");
        });
    });
}

fn bench_rate_limit(c: &mut Criterion) {
    use upp_gateway::middleware::rate_limit::{RateLimitState, RateLimitConfig, classify_endpoint};

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

    c.bench_function("rate_limit_classify_endpoint", |b| {
        let endpoints = [
            "/upp/v1/markets", "/upp/v1/orders", "/health",
            "/upp/v1/backtest/run", "/upp/v1/ws",
        ];
        let mut i = 0usize;
        b.iter(|| {
            i = (i + 1) % endpoints.len();
            let _ = classify_endpoint(endpoints[i]);
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
);
criterion_main!(benches);
