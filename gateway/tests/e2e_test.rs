// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// End-to-end tests: spin up a live gateway and exercise it through
// the UPP SDK client, validating the full HTTP round-trip including
// serialization, deserialization, status codes, and response shapes.

use upp_gateway::test_harness::start_test_server;

// ─── Health & Status ─────────────────────────────────────────

#[tokio::test]
async fn e2e_health_check() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let health = client.health().await.unwrap();
    assert_eq!(health.status, "healthy");
}

#[tokio::test]
async fn e2e_ready_check() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let ready = client.ready().await.unwrap();
    assert!(ready.ready);
}

#[tokio::test]
async fn e2e_metrics() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let metrics = client.metrics().await.unwrap();
    // Metrics returns a JSON value — just assert it's an object
    assert!(metrics.data.is_object());
}

// ─── Market Data ─────────────────────────────────────────────

#[tokio::test]
async fn e2e_list_markets() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let markets = client.list_markets(None, None, None, None, None).await.unwrap();
    assert_eq!(markets.markets.len(), 2);

    let first = &markets.markets[0];
    assert_eq!(first.id, "kalshi.com:BTC-2026-Q1");
    assert_eq!(first.provider, "kalshi.com");
    assert_eq!(first.status, "open");
    assert!(!first.outcomes.is_empty());
    assert_eq!(first.outcomes[0].id, "yes");
    assert!(first.outcomes[0].price.is_some());
}

#[tokio::test]
async fn e2e_search_markets() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let search = client
        .search_markets(Some("bitcoin"), None, None, Some(5))
        .await
        .unwrap();
    assert!(search.total >= 1);
    assert!(!search.results.is_empty());
    assert!(search.results[0].title.to_lowercase().contains("bitcoin"));
}

#[tokio::test]
async fn e2e_get_market() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let resp = client.get_market("kalshi.com:BTC-2026-Q1").await.unwrap();
    assert_eq!(resp.market.id, "kalshi.com:BTC-2026-Q1");
    assert_eq!(resp.market.outcomes.len(), 2);
}

#[tokio::test]
async fn e2e_get_orderbook() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let book = client.get_orderbook("kalshi.com:BTC-2026-Q1").await.unwrap();
    assert_eq!(book.market_id, "kalshi.com:BTC-2026-Q1");
    assert!(!book.bids.is_empty());
    assert!(!book.asks.is_empty());
    // Bids should be sorted descending
    assert!(book.bids[0].price >= book.bids[1].price);
    // Asks should be sorted ascending
    assert!(book.asks[0].price <= book.asks[1].price);
}

// ─── Arbitrage ───────────────────────────────────────────────

#[tokio::test]
async fn e2e_arbitrage_list() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let arb = client.list_arbitrage().await.unwrap();
    // Test server returns empty opportunities (no live providers)
    assert!(arb.opportunities.is_empty());
}

#[tokio::test]
async fn e2e_arbitrage_summary() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let summary = client.arbitrage_summary().await.unwrap();
    assert_eq!(summary.total_opportunities, 0);
    assert!(summary.best_opportunity.is_none());
}

// ─── Price Index ─────────────────────────────────────────────

#[tokio::test]
async fn e2e_price_index_stats() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let stats = client.price_index_stats().await.unwrap();
    assert_eq!(stats.index_id, "upp-global");
    assert_eq!(stats.volume_24h, 0.0);
}

// ─── Backtest ────────────────────────────────────────────────

#[tokio::test]
async fn e2e_list_strategies() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let resp = client.list_strategies().await.unwrap();
    assert_eq!(resp.strategies.len(), 4);

    let names: Vec<&str> = resp.strategies.iter().map(|s| s.id.as_str()).collect();
    assert!(names.contains(&"momentum"));
    assert!(names.contains(&"mean_reversion"));
    assert!(names.contains(&"breakout"));
    assert!(names.contains(&"macd"));
}

// ─── Feeds ───────────────────────────────────────────────────

#[tokio::test]
async fn e2e_feed_status() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let status = client.feed_status().await.unwrap();
    assert!(status.feeds.is_empty());
}

#[tokio::test]
async fn e2e_feed_stats() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    let stats = client.feed_stats().await.unwrap();
    assert_eq!(stats.total_feeds, 0);
    assert_eq!(stats.active_feeds, 0);
}

// ─── SDK Client Configuration ────────────────────────────────

#[tokio::test]
async fn e2e_sdk_builder_pattern() {
    let server = start_test_server().await;

    let client = upp_sdk::UppClient::builder()
        .base_url(&server.base_url)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    // Verify base URL is correctly set
    assert_eq!(
        client.base_url().as_str(),
        &format!("{}/", server.base_url)
    );

    // Verify it works
    let health = client.health().await.unwrap();
    assert_eq!(health.status, "healthy");
}

#[tokio::test]
async fn e2e_sdk_with_api_key() {
    let server = start_test_server().await;

    let client = upp_sdk::UppClient::builder()
        .base_url(&server.base_url)
        .api_key("test-key-123")
        .build()
        .unwrap();

    // Public endpoints should still work with an API key set
    let health = client.health().await.unwrap();
    assert_eq!(health.status, "healthy");
}

// ─── Error Handling ──────────────────────────────────────────

#[tokio::test]
async fn e2e_sdk_invalid_url_fails() {
    let result = upp_sdk::UppClient::new("not-a-valid-url");
    assert!(result.is_err());
}

#[tokio::test]
async fn e2e_sdk_connection_refused() {
    // Connect to a port where nothing is listening
    let client = upp_sdk::UppClient::new("http://127.0.0.1:1").unwrap();
    let result = client.health().await;
    assert!(result.is_err());
}

// ─── Multiple Concurrent Requests ────────────────────────────

#[tokio::test]
async fn e2e_concurrent_requests() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    // Fire 10 requests concurrently — validates the server handles
    // parallel connections and the SDK client is Clone-safe
    let mut handles = vec![];
    for _ in 0..10 {
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            c.health().await.unwrap()
        }));
    }

    for handle in handles {
        let health = handle.await.unwrap();
        assert_eq!(health.status, "healthy");
    }
}

// ─── Full Lifecycle (multi-step scenario) ────────────────────

#[tokio::test]
async fn e2e_full_lifecycle_public_endpoints() {
    let server = start_test_server().await;
    let client = upp_sdk::UppClient::new(&server.base_url).unwrap();

    // Step 1: Health check
    let health = client.health().await.unwrap();
    assert_eq!(health.status, "healthy");

    // Step 2: List markets
    let markets = client.list_markets(None, None, None, None, None).await.unwrap();
    assert!(!markets.markets.is_empty());
    let market_id = &markets.markets[0].id;

    // Step 3: Get specific market
    let market = client.get_market(market_id).await.unwrap();
    assert_eq!(&market.market.id, market_id);

    // Step 4: Get orderbook for that market
    let book = client.get_orderbook(market_id).await.unwrap();
    assert_eq!(&book.market_id, market_id);

    // Step 5: Search for markets
    let search = client.search_markets(Some("bitcoin"), None, None, None).await.unwrap();
    assert!(search.total >= 1);

    // Step 6: Check arbitrage
    let _arb = client.list_arbitrage().await.unwrap();

    // Step 7: Check price index
    let _stats = client.price_index_stats().await.unwrap();

    // Step 8: List strategies
    let strategies = client.list_strategies().await.unwrap();
    assert!(!strategies.strategies.is_empty());

    // Step 9: Check feed status
    let _feeds = client.feed_status().await.unwrap();
}

// ─── Raw HTTP Verification ───────────────────────────────────

#[tokio::test]
async fn e2e_raw_http_health() {
    let server = start_test_server().await;

    // Bypass the SDK — hit the server directly with reqwest
    let resp = reqwest::get(format!("{}/health", server.base_url))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn e2e_raw_http_404() {
    let server = start_test_server().await;

    let resp = reqwest::get(format!("{}/nonexistent", server.base_url))
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn e2e_raw_http_markets_json() {
    let server = start_test_server().await;

    let resp = reqwest::get(format!("{}/upp/v1/markets", server.base_url))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["markets"].is_array());
    assert!(body["markets"].as_array().unwrap().len() > 0);
}
