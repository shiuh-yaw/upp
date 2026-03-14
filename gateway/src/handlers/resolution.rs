// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Path, State}, response::IntoResponse, Json};
use crate::AppState;

pub async fn get_resolution(
    State(_state): State<AppState>,
    Path(_id): Path<String>,
) -> impl IntoResponse {
    Json(serde_json::json!({ "status": "not_implemented" }))
}

pub async fn list_resolutions(
    State(_state): State<AppState>,
) -> impl IntoResponse {
    Json(serde_json::json!({ "resolutions": [] }))
}
