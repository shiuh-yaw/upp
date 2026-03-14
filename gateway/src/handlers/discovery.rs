// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::{Path, State}, response::IntoResponse, Json};
use axum::http::StatusCode;

use crate::{AppState, not_found};

/// GET /upp/v1/discovery/manifest/:provider
pub async fn get_manifest(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    match state.registry.get_manifest(&provider).await {
        Ok(manifest) => Json(manifest).into_response(),
        Err(e) => not_found(&e.to_string()).into_response(),
    }
}

/// GET /upp/v1/discovery/providers
pub async fn list_providers(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let manifests = state.registry.list_providers().await;
    Json(serde_json::json!({
        "providers": manifests,
        "total": manifests.len(),
    }))
}

/// POST /upp/v1/discovery/negotiate
pub async fn negotiate(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let provider_id = req.get("provider").and_then(|v| v.as_str()).unwrap_or("");
    match state.registry.get_manifest(provider_id).await {
        Ok(manifest) => Json(serde_json::json!({
            "active_capabilities": manifest.capabilities,
            "selected_transport": "rest",
            "selected_auth": manifest.authentication.first().unwrap_or(&"none".to_string()),
        })).into_response(),
        Err(e) => not_found(&e.to_string()).into_response(),
    }
}

/// GET /upp/v1/discovery/health/:provider
pub async fn health_check(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    match state.registry.health_check(&provider).await {
        Ok(health) => Json(health).into_response(),
        Err(e) => (StatusCode::SERVICE_UNAVAILABLE, Json(crate::upp_error("PROVIDER_ERROR", &e.to_string()))).into_response(),
    }
}

/// GET /upp/v1/discovery/health
pub async fn health_check_all(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let results = state.registry.health_check_all().await;
    Json(serde_json::json!({
        "providers": results,
        "total": results.len(),
    }))
}

/// GET /.well-known/upp
pub async fn well_known(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let providers = state.registry.list_providers().await;
    Json(serde_json::json!({
        "upp_version": "2026-03-11",
        "gateway": {
            "version": env!("CARGO_PKG_VERSION"),
            "transports": ["rest", "websocket"],
        },
        "providers": providers,
    }))
}
