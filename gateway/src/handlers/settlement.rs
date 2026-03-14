// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use crate::AppState;

pub async fn list_instruments(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "instruments": [
            { "type": "usd", "name": "US Dollar", "providers": ["kalshi.com"] },
            { "type": "usdc", "name": "USDC (Polygon)", "providers": ["polymarket.com"] },
            { "type": "usdc_bnb", "name": "USDC (BNB Chain)", "providers": ["opinion.trade"] },
        ]
    }))
}

pub async fn list_handlers(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "handlers": [
            { "type": "custodial_usd", "provider": "kalshi.com" },
            { "type": "onchain_ctf", "provider": "polymarket.com" },
            { "type": "onchain_bnb", "provider": "opinion.trade" },
        ]
    }))
}
