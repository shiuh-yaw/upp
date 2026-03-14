// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Query, State}, response::IntoResponse, Json};
use serde::Deserialize;
use std::sync::atomic::Ordering;

use crate::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct ArbitrageParams {
    pub min_spread: Option<f64>,
    pub min_confidence: Option<f64>,
    pub provider: Option<String>,
    pub limit: Option<usize>,
}

/// GET /upp/v1/arbitrage — List active arbitrage opportunities
pub async fn list_opportunities(
    State(state): State<AppState>,
    Query(params): Query<ArbitrageParams>,
) -> impl IntoResponse {
    let mut alerts = state.arbitrage_scanner.get_active_alerts();

    if let Some(min_spread) = params.min_spread {
        alerts.retain(|a| a.spread_pct >= min_spread);
    }
    if let Some(min_conf) = params.min_confidence {
        alerts.retain(|a| a.confidence >= min_conf);
    }
    if let Some(ref provider) = params.provider {
        alerts.retain(|a| &a.bid_provider == provider || &a.ask_provider == provider);
    }

    alerts.sort_by(|a, b| b.net_profit_per_contract
        .partial_cmp(&a.net_profit_per_contract)
        .unwrap_or(std::cmp::Ordering::Equal));

    let limit = params.limit.unwrap_or(50);
    alerts.truncate(limit);

    Json(serde_json::json!({
        "opportunities": alerts,
        "total": alerts.len(),
        "scanner": {
            "scans_total": state.arbitrage_scanner.scans_total.load(Ordering::Relaxed),
            "total_detected": state.arbitrage_scanner.opportunities_detected.load(Ordering::Relaxed),
        },
    }))
}

/// GET /upp/v1/arbitrage/summary
pub async fn get_summary(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let summary = state.arbitrage_scanner.get_summary().await;
    Json(serde_json::to_value(&summary).unwrap_or_default())
}

/// GET /upp/v1/arbitrage/history
pub async fn get_history(
    State(state): State<AppState>,
    Query(params): Query<ArbitrageParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let mut history = state.arbitrage_scanner.get_history(limit).await;

    if let Some(min_spread) = params.min_spread {
        history.retain(|a| a.spread_pct >= min_spread);
    }

    Json(serde_json::json!({
        "history": history,
        "total": history.len(),
    }))
}
