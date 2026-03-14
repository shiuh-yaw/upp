// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use std::sync::atomic::Ordering;

use crate::AppState;

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": "UPP/2026-03-11",
    }))
}

pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let providers = state.registry.list_providers().await;
    Json(serde_json::json!({
        "ready": true,
        "providers": providers.len(),
        "provider_ids": state.registry.provider_ids(),
    }))
}

pub async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let total = state.metrics.requests_total.load(Ordering::Relaxed);
    let ok = state.metrics.requests_ok.load(Ordering::Relaxed);
    let err = state.metrics.requests_err.load(Ordering::Relaxed);
    let rl = state.metrics.requests_rate_limited.load(Ordering::Relaxed);
    let ws = state.metrics.ws_connections.load(Ordering::Relaxed);
    let ws_channels = state.ws_manager.active_channels().await;
    let ws_subs = state.ws_manager.total_subscribers().await;
    let rl_clients = state.rate_limiter.tracked_clients();
    let stored_orders = state.storage.order_count().await.unwrap_or(0);
    let stored_trades = state.storage.trade_count().await.unwrap_or(0);
    let arb_summary = state.arbitrage_scanner.get_summary().await;
    let arb_active = arb_summary.active_opportunities;
    let arb_scans = arb_summary.total_scans;
    let arb_detected = arb_summary.total_detected;
    let pi_stats = state.price_index.stats();
    let router_stats = state.smart_router.stats();
    let feed_stats = state.live_feed.stats();
    let ingestion_stats = state.ingestion.stats();

    format!(
        "# HELP upp_requests_total Total requests received\n\
         # TYPE upp_requests_total counter\n\
         upp_requests_total {}\n\
         # HELP upp_requests_ok Successful requests\n\
         # TYPE upp_requests_ok counter\n\
         upp_requests_ok {}\n\
         # HELP upp_requests_error Failed requests\n\
         # TYPE upp_requests_error counter\n\
         upp_requests_error {}\n\
         # HELP upp_requests_rate_limited Rate-limited requests\n\
         # TYPE upp_requests_rate_limited counter\n\
         upp_requests_rate_limited {}\n\
         # HELP upp_ws_connections Total WebSocket connections\n\
         # TYPE upp_ws_connections counter\n\
         upp_ws_connections {}\n\
         # HELP upp_ws_active_channels Active broadcast channels\n\
         # TYPE upp_ws_active_channels gauge\n\
         upp_ws_active_channels {}\n\
         # HELP upp_ws_subscribers Total WebSocket subscribers\n\
         # TYPE upp_ws_subscribers gauge\n\
         upp_ws_subscribers {}\n\
         # HELP upp_rate_limit_tracked_clients Tracked rate limit clients\n\
         # TYPE upp_rate_limit_tracked_clients gauge\n\
         upp_rate_limit_tracked_clients {}\n\
         # HELP upp_storage_orders_total Total orders in persistent storage\n\
         # TYPE upp_storage_orders_total gauge\n\
         upp_storage_orders_total {}\n\
         # HELP upp_storage_trades_total Total trades in persistent storage\n\
         # TYPE upp_storage_trades_total gauge\n\
         upp_storage_trades_total {}\n\
         # HELP upp_arbitrage_scans_total Total arbitrage scans performed\n\
         # TYPE upp_arbitrage_scans_total counter\n\
         upp_arbitrage_scans_total {}\n\
         # HELP upp_arbitrage_active Currently active arbitrage opportunities\n\
         # TYPE upp_arbitrage_active gauge\n\
         upp_arbitrage_active {}\n\
         # HELP upp_arbitrage_detected_total Total arbitrage opportunities detected\n\
         # TYPE upp_arbitrage_detected_total counter\n\
         upp_arbitrage_detected_total {}\n\
         # HELP upp_price_index_ticks_total Total price ticks ingested\n\
         # TYPE upp_price_index_ticks_total counter\n\
         upp_price_index_ticks_total {}\n\
         # HELP upp_price_index_markets Markets tracked by price indexer\n\
         # TYPE upp_price_index_markets gauge\n\
         upp_price_index_markets {}\n\
         # HELP upp_router_routes_computed Total routing plans computed\n\
         # TYPE upp_router_routes_computed counter\n\
         upp_router_routes_computed {}\n\
         # HELP upp_router_orders_routed Total orders routed via smart router\n\
         # TYPE upp_router_orders_routed counter\n\
         upp_router_orders_routed {}\n\
         # HELP upp_live_feed_messages_total Total messages received from live feeds\n\
         # TYPE upp_live_feed_messages_total counter\n\
         upp_live_feed_messages_total {}\n\
         # HELP upp_live_feed_reconnects_total Total provider reconnections\n\
         # TYPE upp_live_feed_reconnects_total counter\n\
         upp_live_feed_reconnects_total {}\n\
         # HELP upp_live_feed_providers Registered live feed providers\n\
         # TYPE upp_live_feed_providers gauge\n\
         upp_live_feed_providers {}\n\
         # HELP upp_ingestion_ticks_total Total historical ticks ingested\n\
         # TYPE upp_ingestion_ticks_total counter\n\
         upp_ingestion_ticks_total {}\n\
         # HELP upp_ingestion_markets_processed Markets processed by ingestion pipeline\n\
         # TYPE upp_ingestion_markets_processed counter\n\
         upp_ingestion_markets_processed {}\n\
         # HELP upp_api_keys_total Total API keys created\n\
         # TYPE upp_api_keys_total gauge\n\
         upp_api_keys_total {}\n\
         # HELP upp_api_keys_active Active (non-revoked) API keys\n\
         # TYPE upp_api_keys_active gauge\n\
         upp_api_keys_active {}\n",
        total, ok, err, rl, ws, ws_channels, ws_subs, rl_clients,
        stored_orders, stored_trades,
        arb_scans, arb_active, arb_detected,
        pi_stats.ticks_ingested, pi_stats.markets_tracked,
        router_stats.routes_computed, router_stats.orders_routed,
        feed_stats.messages_received_total, feed_stats.reconnects_total,
        feed_stats.providers_registered,
        ingestion_stats.ticks_ingested,
        ingestion_stats.markets_processed,
        state.api_keys.count(),
        state.api_keys.active_count(),
    )
}
