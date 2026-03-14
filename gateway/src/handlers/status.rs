// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Provider health dashboard — real-time view of per-provider health,
// latency percentiles, error rates, and circuit breaker state.

use axum::{extract::State, response::IntoResponse, Json};
use axum::response::Html;
use std::sync::atomic::Ordering;

use crate::AppState;

/// GET /status — Serve the provider health dashboard HTML page.
pub async fn status_page() -> impl IntoResponse {
    Html(include_str!("../../static/status.html"))
}

/// GET /upp/v1/status — JSON endpoint for provider health, circuit breakers,
/// latency percentiles, and error rates.
pub async fn status_json(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let provider_ids = state.registry.provider_ids();
    let mut providers = Vec::new();

    for pid in &provider_ids {
        // Health check
        let health = state.registry.health_check(pid).await;
        let (healthy, latency_ms, status_str) = match &health {
            Ok(h) => (h.healthy, h.latency_ms as f64, h.status.clone()),
            Err(_) => (false, 0.0, "error".to_string()),
        };

        // Circuit breaker state
        let (cb_state_str, cb_failures) = if let Some(cb) = state.circuit_breakers.get(pid) {
            (format!("{:?}", cb.get_state()), 0u64)
        } else {
            ("Unknown".to_string(), 0)
        };

        providers.push(serde_json::json!({
            "provider_id": pid,
            "healthy": healthy,
            "latency_ms": latency_ms,
            "status": status_str,
            "circuit_breaker": {
                "state": cb_state_str,
                "consecutive_failures": cb_failures,
            },
        }));
    }

    // Global metrics
    let total_requests = state.metrics.requests_total.load(Ordering::Relaxed);
    let ok_requests = state.metrics.requests_ok.load(Ordering::Relaxed);
    let err_requests = state.metrics.requests_err.load(Ordering::Relaxed);
    let rate_limited = state.metrics.requests_rate_limited.load(Ordering::Relaxed);
    let ws_connections = state.metrics.ws_connections.load(Ordering::Relaxed);

    // Feed status
    let feed_stats = state.live_feed.stats();

    // Arbitrage scanner
    let arb_summary = state.arbitrage_scanner.get_summary().await;

    Json(serde_json::json!({
        "gateway": {
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_seconds": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        },
        "providers": providers,
        "metrics": {
            "requests_total": total_requests,
            "requests_ok": ok_requests,
            "requests_error": err_requests,
            "requests_rate_limited": rate_limited,
            "ws_connections_active": ws_connections,
        },
        "live_feeds": {
            "messages_total": feed_stats.messages_received_total,
            "reconnects_total": feed_stats.reconnects_total,
            "providers_registered": feed_stats.providers_registered,
        },
        "arbitrage": {
            "active_opportunities": arb_summary.active_opportunities,
            "total_scans": arb_summary.total_scans,
            "total_detected": arb_summary.total_detected,
        },
    }))
}
