// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Path, Query, State}, response::IntoResponse, Json};
use serde::Deserialize;

use crate::{AppState, not_found};

#[derive(Debug, Deserialize, Default)]
pub struct CandleParams {
    pub outcome_id: Option<String>,
    pub resolution: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub limit: Option<usize>,
}

/// GET /upp/v1/markets/:market_id/candles
pub async fn get_candles(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    Query(params): Query<CandleParams>,
) -> impl IntoResponse {
    let outcome_id = params.outcome_id.as_deref().unwrap_or("yes");
    let resolution = params.resolution.as_deref()
        .and_then(crate::core::price_index::Resolution::parse)
        .unwrap_or(crate::core::price_index::Resolution::FiveMinute);
    let limit = params.limit.unwrap_or(100).min(1000);

    let candles = state.price_index.query_candles(
        &market_id, outcome_id, resolution,
        params.from, params.to, limit,
    );

    Json(serde_json::json!({
        "market_id": market_id,
        "outcome_id": outcome_id,
        "resolution": resolution,
        "candles": candles,
        "count": candles.len(),
    }))
}

/// GET /upp/v1/markets/:market_id/candles/latest
pub async fn get_latest_candle(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    Query(params): Query<CandleParams>,
) -> impl IntoResponse {
    let outcome_id = params.outcome_id.as_deref().unwrap_or("yes");
    let resolution = params.resolution.as_deref()
        .and_then(crate::core::price_index::Resolution::parse)
        .unwrap_or(crate::core::price_index::Resolution::OneMinute);

    let candle = state.price_index.latest_candle(&market_id, outcome_id, resolution);

    match candle {
        Some(c) => Json(serde_json::to_value(&c).unwrap_or_default()).into_response(),
        None => not_found("No candle data for this market").into_response(),
    }
}

/// GET /upp/v1/price-index/stats
pub async fn get_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let stats = state.price_index.stats();
    Json(serde_json::to_value(&stats).unwrap_or_default())
}
