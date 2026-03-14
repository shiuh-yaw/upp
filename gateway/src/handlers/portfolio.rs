// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Query, State}, response::IntoResponse, Json};
use serde::Deserialize;
use tracing::warn;

use crate::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct PortfolioParams {
    pub provider: Option<String>,
}

pub async fn list_positions(
    State(state): State<AppState>,
    Query(params): Query<PortfolioParams>,
) -> impl IntoResponse {
    let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
        vec![pid.clone()]
    } else {
        state.registry.provider_ids()
    };

    let mut all_positions = Vec::new();
    for pid in &provider_ids {
        if let Some(adapter) = state.registry.get(pid) {
            match adapter.get_positions().await {
                Ok(positions) => all_positions.extend(positions),
                Err(e) => warn!(provider = %pid, "get_positions: {}", e),
            }
        }
    }

    Json(serde_json::json!({
        "positions": all_positions,
        "total": all_positions.len(),
    }))
}

pub async fn get_summary(
    State(state): State<AppState>,
    Query(params): Query<PortfolioParams>,
) -> impl IntoResponse {
    let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
        vec![pid.clone()]
    } else {
        state.registry.provider_ids()
    };

    let mut total_value = 0.0_f64;
    let mut total_pnl = 0.0_f64;
    let mut position_count = 0;
    let mut provider_summaries = Vec::new();

    for pid in &provider_ids {
        if let Some(adapter) = state.registry.get(pid) {
            if let Ok(positions) = adapter.get_positions().await {
                let prov_value: f64 = positions.iter()
                    .map(|p| p.current_value.parse::<f64>().unwrap_or(0.0)).sum();
                let prov_pnl: f64 = positions.iter()
                    .map(|p| p.unrealized_pnl.parse::<f64>().unwrap_or(0.0)).sum();
                total_value += prov_value;
                total_pnl += prov_pnl;
                position_count += positions.len();
                provider_summaries.push(serde_json::json!({
                    "provider": pid,
                    "positions": positions.len(),
                    "value": format!("{:.2}", prov_value),
                    "unrealized_pnl": format!("{:.2}", prov_pnl),
                }));
            }
        }
    }

    Json(serde_json::json!({
        "total_value": format!("{:.2}", total_value),
        "unrealized_pnl": format!("{:.2}", total_pnl),
        "position_count": position_count,
        "providers": provider_summaries,
    }))
}

pub async fn list_balances(
    State(state): State<AppState>,
    Query(params): Query<PortfolioParams>,
) -> impl IntoResponse {
    let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
        vec![pid.clone()]
    } else {
        state.registry.provider_ids()
    };

    let mut all_balances = Vec::new();
    for pid in &provider_ids {
        if let Some(adapter) = state.registry.get(pid) {
            match adapter.get_balances().await {
                Ok(balances) => all_balances.extend(balances),
                Err(e) => warn!(provider = %pid, "get_balances: {}", e),
            }
        }
    }

    Json(serde_json::json!({ "balances": all_balances }))
}

/// GET /upp/v1/portfolio/analytics — Full portfolio analytics with risk scoring
pub async fn get_analytics(
    State(state): State<AppState>,
    Query(params): Query<PortfolioParams>,
) -> impl IntoResponse {
    let provider_ids: Vec<String> = if let Some(ref pid) = params.provider {
        vec![pid.clone()]
    } else {
        state.registry.provider_ids()
    };

    let mut all_positions = Vec::new();
    let mut all_trades = Vec::new();
    let market_map = std::collections::HashMap::new();

    for pid in &provider_ids {
        if let Some(adapter) = state.registry.get(pid) {
            if let Ok(positions) = adapter.get_positions().await {
                all_positions.extend(positions);
            }
            let trade_filter = crate::adapters::TradeFilter::default();
            if let Ok(trades) = adapter.get_trade_history(trade_filter).await {
                all_trades.extend(trades);
            }
        }
    }

    let analytics = crate::core::portfolio::compute_analytics(
        &all_positions,
        &all_trades,
        &market_map,
    );

    Json(serde_json::to_value(&analytics).unwrap_or_default())
}
