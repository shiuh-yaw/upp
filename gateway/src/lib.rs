// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Library crate — re-exports core modules for integration testing
// and SDK generation. The binary entry point is in main.rs.

pub mod adapters;
pub mod core;
pub mod middleware;
pub mod transport;
mod gen;

/// Test harness for spinning up a live server — available in integration tests.
pub mod test_harness {
    use std::net::SocketAddr;

    /// A running test server with its base URL.
    pub struct TestServer {
        pub base_url: String,
        pub addr: SocketAddr,
    }

    /// Start a gateway on an OS-assigned port and return the TestServer.
    ///
    /// Uses inline handlers that return realistic JSON matching the SDK's
    /// type definitions. The full handler/middleware stack is covered by
    /// the 243+ tests in integration_test.rs — these e2e tests validate
    /// the SDK client ↔ live HTTP round-trip.
    pub async fn start_test_server() -> TestServer {
        use axum::{routing::get, Router};

        // Bind to port 0 for OS-assigned port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let base_url = format!("http://127.0.0.1:{}", addr.port());

        let app = Router::new()
            .route("/health", get(|| async {
                axum::Json(serde_json::json!({"status": "healthy"}))
            }))
            .route("/ready", get(|| async {
                axum::Json(serde_json::json!({"ready": true}))
            }))
            .route("/metrics", get(|| async {
                axum::Json(serde_json::json!({"data": {}}))
            }))
            .route("/upp/v1/markets", get(|| async {
                axum::Json(serde_json::json!({
                    "markets": [
                        {
                            "id": "kalshi.com:BTC-2026-Q1",
                            "title": "Bitcoin above $100k Q1 2026",
                            "description": "Will Bitcoin close above $100,000 on March 31, 2026?",
                            "provider": "kalshi.com",
                            "status": "open",
                            "category": "crypto",
                            "outcomes": [
                                {"id": "yes", "title": "Yes", "price": 0.65},
                                {"id": "no", "title": "No", "price": 0.35}
                            ],
                            "volume": 50000.0,
                            "volume_24h": 1200.0,
                            "created_at": "2025-12-01T00:00:00Z",
                            "closes_at": "2026-03-31T23:59:59Z"
                        },
                        {
                            "id": "polymarket.com:ETH-MERGE-SUCCESS",
                            "title": "Ethereum network upgrade successful",
                            "provider": "polymarket.com",
                            "status": "open",
                            "category": "crypto",
                            "outcomes": [
                                {"id": "yes", "title": "Yes", "price": 0.88},
                                {"id": "no", "title": "No", "price": 0.12}
                            ],
                            "volume": 120000.0,
                            "volume_24h": 8500.0,
                            "created_at": "2025-11-15T00:00:00Z",
                            "closes_at": "2026-06-30T23:59:59Z"
                        }
                    ],
                    "pagination": {"limit": 50, "cursor": null}
                }))
            }))
            .route("/upp/v1/markets/search", get(|| async {
                axum::Json(serde_json::json!({
                    "results": [
                        {
                            "id": "kalshi.com:BTC-2026-Q1",
                            "title": "Bitcoin above $100k Q1 2026",
                            "provider": "kalshi.com",
                            "status": "open",
                            "category": "crypto",
                            "outcomes": [
                                {"id": "yes", "title": "Yes", "price": 0.65}
                            ],
                            "volume": 50000.0,
                            "volume_24h": 1200.0,
                            "created_at": null,
                            "closes_at": null
                        }
                    ],
                    "total": 1
                }))
            }))
            .route("/upp/v1/markets/:market_id", get(|| async {
                axum::Json(serde_json::json!({
                    "market": {
                        "id": "kalshi.com:BTC-2026-Q1",
                        "title": "Bitcoin above $100k Q1 2026",
                        "description": "Will Bitcoin close above $100,000?",
                        "provider": "kalshi.com",
                        "status": "open",
                        "category": "crypto",
                        "outcomes": [
                            {"id": "yes", "title": "Yes", "price": 0.65},
                            {"id": "no", "title": "No", "price": 0.35}
                        ],
                        "volume": 50000.0,
                        "volume_24h": 1200.0,
                        "created_at": "2025-12-01T00:00:00Z",
                        "closes_at": "2026-03-31T23:59:59Z"
                    }
                }))
            }))
            .route("/upp/v1/markets/:market_id/orderbook", get(|| async {
                axum::Json(serde_json::json!({
                    "market_id": "kalshi.com:BTC-2026-Q1",
                    "bids": [
                        {"price": 0.64, "size": 500.0, "count": 3},
                        {"price": 0.63, "size": 200.0, "count": 1}
                    ],
                    "asks": [
                        {"price": 0.66, "size": 300.0, "count": 2},
                        {"price": 0.67, "size": 100.0, "count": 1}
                    ],
                    "timestamp": "2026-03-14T12:00:00Z"
                }))
            }))
            .route("/upp/v1/arbitrage", get(|| async {
                axum::Json(serde_json::json!({
                    "opportunities": []
                }))
            }))
            .route("/upp/v1/arbitrage/summary", get(|| async {
                axum::Json(serde_json::json!({
                    "total_opportunities": 0,
                    "total_profit_24h": 0.0,
                    "best_opportunity": null
                }))
            }))
            .route("/upp/v1/price-index/stats", get(|| async {
                axum::Json(serde_json::json!({
                    "index_id": "upp-global",
                    "price": 0.0,
                    "change_24h": 0.0,
                    "change_percent_24h": 0.0,
                    "high_24h": 0.0,
                    "low_24h": 0.0,
                    "volume_24h": 0.0
                }))
            }))
            .route("/upp/v1/backtest/strategies", get(|| async {
                axum::Json(serde_json::json!({
                    "strategies": [
                        {"id": "momentum", "name": "Momentum", "description": "Trend following"},
                        {"id": "mean_reversion", "name": "Mean Reversion", "description": "Revert to mean"},
                        {"id": "breakout", "name": "Breakout", "description": "Price breakout"},
                        {"id": "macd", "name": "MACD", "description": "Moving average convergence divergence"}
                    ]
                }))
            }))
            .route("/upp/v1/feeds/status", get(|| async {
                axum::Json(serde_json::json!({
                    "feeds": [],
                }))
            }))
            .route("/upp/v1/feeds/stats", get(|| async {
                axum::Json(serde_json::json!({
                    "total_feeds": 0,
                    "active_feeds": 0,
                    "total_messages": 0,
                    "uptime_percent": 0.0
                }))
            }));

        // Spawn server in background
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server");
        });

        // Small delay to ensure server is ready
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        TestServer { base_url, addr }
    }

}
