// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use axum::http::StatusCode;

use crate::{AppState, upp_error};

/// GET /upp/v1/feeds/status
pub async fn feed_status(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let connections = state.live_feed.status().await;
    Json(serde_json::json!({ "connections": connections }))
}

/// GET /upp/v1/feeds/stats
pub async fn feed_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let stats = state.live_feed.stats();
    Json(serde_json::json!(stats))
}

/// POST /upp/v1/feeds/subscribe
pub async fn subscribe_markets(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let provider_id = body.get("provider_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let market_ids: Vec<String> = body.get("market_ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if provider_id.is_empty() || market_ids.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "provider_id and market_ids required"))).into_response();
    }

    state.live_feed.subscribe_markets(provider_id, market_ids.clone()).await;

    Json(serde_json::json!({
        "status": "subscribed",
        "provider_id": provider_id,
        "market_ids": market_ids,
    })).into_response()
}
