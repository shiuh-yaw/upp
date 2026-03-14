// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use axum::http::StatusCode;

use crate::{AppState, upp_error};

/// GET /upp/v1/ingestion/stats
pub async fn stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let s = state.ingestion.stats();
    Json(serde_json::json!(s))
}

/// POST /upp/v1/ingestion/ingest
pub async fn ingest_market(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let provider_id = body.get("provider_id").and_then(|v| v.as_str()).unwrap_or("");
    let market_id = body.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
    let hours_back = body.get("hours_back").and_then(|v| v.as_u64()).unwrap_or(24);

    if provider_id.is_empty() || market_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "provider_id and market_id required"))).into_response();
    }

    let to = chrono::Utc::now();
    let from = to - chrono::Duration::hours(hours_back as i64);

    match state.ingestion.ingest_market(provider_id, market_id, from, to).await {
        Ok(count) => Json(serde_json::json!({
            "status": "ok",
            "ticks_ingested": count,
            "provider_id": provider_id,
            "market_id": market_id,
            "hours_back": hours_back,
        })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INGESTION_ERROR", &e.to_string()))).into_response(),
    }
}

/// POST /upp/v1/ingestion/ingest-recent
pub async fn ingest_recent(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let hours_back = body.get("hours_back").and_then(|v| v.as_u64()).unwrap_or(1);

    match state.ingestion.ingest_all_recent(hours_back).await {
        Ok(count) => Json(serde_json::json!({
            "status": "ok",
            "ticks_ingested": count,
            "hours_back": hours_back,
        })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(upp_error("INGESTION_ERROR", &e.to_string()))).into_response(),
    }
}
