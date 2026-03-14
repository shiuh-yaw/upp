// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use axum::http::StatusCode;
use std::collections::HashMap;

use crate::{AppState, upp_error};
use crate::core::backtest as bt;

/// GET /upp/v1/backtest/strategies
pub async fn list_strategies() -> impl IntoResponse {
    let strategies = bt::available_strategies();
    Json(serde_json::json!({ "strategies": strategies }))
}

/// POST /upp/v1/backtest/run
pub async fn run_backtest(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let strategy_name = body.get("strategy").and_then(|v| v.as_str()).unwrap_or("");
    let market_id = body.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
    let outcome_id = body.get("outcome_id").and_then(|v| v.as_str()).unwrap_or("yes");
    let resolution_str = body.get("resolution").and_then(|v| v.as_str()).unwrap_or("1m");

    if strategy_name.is_empty() || market_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "strategy and market_id required"))).into_response();
    }

    let resolution = match crate::core::price_index::Resolution::parse(resolution_str) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "Invalid resolution. Use: 1m, 5m, 1h, 1d"))).into_response(),
    };

    let params: HashMap<String, f64> = body.get("params")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                .collect()
        })
        .unwrap_or_default();

    let mut strategy = match bt::create_strategy(strategy_name, &params) {
        Some(s) => s,
        None => return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", &format!("Unknown strategy: {}", strategy_name)))).into_response(),
    };

    let config = bt::BacktestConfig {
        initial_capital: body.get("initial_capital").and_then(|v| v.as_f64()).unwrap_or(10_000.0),
        fee_rate: body.get("fee_rate").and_then(|v| v.as_f64()).unwrap_or(0.02),
        slippage_rate: body.get("slippage_rate").and_then(|v| v.as_f64()).unwrap_or(0.005),
        max_position: body.get("max_position").and_then(|v| v.as_i64()).unwrap_or(1000),
        risk_free_rate: body.get("risk_free_rate").and_then(|v| v.as_f64()).unwrap_or(0.05),
    };

    match bt::run_backtest_from_index(strategy.as_mut(), &state.price_index, market_id, outcome_id, resolution, &config) {
        Some(result) => Json(serde_json::json!(result)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(upp_error("NOT_FOUND", "Insufficient candle data for backtest (need >= 2 candles)"))).into_response(),
    }
}

/// POST /upp/v1/backtest/compare
pub async fn compare_strategies(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let market_id = body.get("market_id").and_then(|v| v.as_str()).unwrap_or("");
    let outcome_id = body.get("outcome_id").and_then(|v| v.as_str()).unwrap_or("yes");
    let resolution_str = body.get("resolution").and_then(|v| v.as_str()).unwrap_or("1m");
    let strategy_names: Vec<String> = body.get("strategies")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    if market_id.is_empty() || strategy_names.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "market_id and strategies array required"))).into_response();
    }

    let resolution = match crate::core::price_index::Resolution::parse(resolution_str) {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, Json(upp_error("BAD_REQUEST", "Invalid resolution"))).into_response(),
    };

    let config = bt::BacktestConfig {
        initial_capital: body.get("initial_capital").and_then(|v| v.as_f64()).unwrap_or(10_000.0),
        fee_rate: body.get("fee_rate").and_then(|v| v.as_f64()).unwrap_or(0.02),
        slippage_rate: body.get("slippage_rate").and_then(|v| v.as_f64()).unwrap_or(0.005),
        max_position: body.get("max_position").and_then(|v| v.as_i64()).unwrap_or(1000),
        risk_free_rate: body.get("risk_free_rate").and_then(|v| v.as_f64()).unwrap_or(0.05),
    };

    let mut results = Vec::new();
    for name in &strategy_names {
        if let Some(mut strategy) = bt::create_strategy(name, &HashMap::new()) {
            if let Some(result) = bt::run_backtest_from_index(strategy.as_mut(), &state.price_index, market_id, outcome_id, resolution, &config) {
                results.push(result.metrics);
            }
        }
    }

    if results.is_empty() {
        return (StatusCode::NOT_FOUND, Json(upp_error("NOT_FOUND", "No valid results — check strategies and candle data availability"))).into_response();
    }

    results.sort_by(|a, b| b.total_return_pct.partial_cmp(&a.total_return_pct).unwrap_or(std::cmp::Ordering::Equal));

    Json(serde_json::json!({
        "market_id": market_id,
        "outcome_id": outcome_id,
        "resolution": resolution_str,
        "results": results,
        "best_strategy": results.first().map(|r| &r.strategy_name),
    })).into_response()
}
