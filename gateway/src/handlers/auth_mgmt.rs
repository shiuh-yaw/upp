// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use axum::http::StatusCode;

use crate::AppState;
use crate::middleware::auth::CreateApiKeyRequest;

/// POST /upp/v1/auth/keys — Create a new API key.
pub async fn create_key(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let client_name = body.get("client_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unnamed")
        .to_string();

    let tier = body.get("tier").and_then(|v| v.as_str()).and_then(|t| {
        match t {
            "free" => Some(crate::middleware::auth::ClientTier::Free),
            "standard" => Some(crate::middleware::auth::ClientTier::Standard),
            "pro" => Some(crate::middleware::auth::ClientTier::Pro),
            "enterprise" => Some(crate::middleware::auth::ClientTier::Enterprise),
            _ => None,
        }
    });

    let providers = body.get("providers")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

    let label = body.get("label").and_then(|v| v.as_str()).map(String::from);
    let expires_in_days = body.get("expires_in_days").and_then(|v| v.as_u64()).map(|d| d as u32);

    let req = CreateApiKeyRequest {
        client_name,
        tier,
        providers,
        label,
        expires_in_days,
    };

    let response = state.api_keys.create_key(req);
    (StatusCode::CREATED, Json(serde_json::json!({
        "key": response.key,
        "key_prefix": response.key_prefix,
        "client_id": response.client_id,
        "created_at": response.created_at,
        "expires_at": response.expires_at,
        "warning": "Store this key securely — it will not be shown again."
    })))
}

/// GET /upp/v1/auth/keys — List all API keys (redacted).
pub async fn list_keys(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let keys = state.api_keys.list_keys();
    Json(serde_json::json!({
        "keys": keys,
        "total": keys.len(),
        "active": state.api_keys.active_count(),
    }))
}

/// POST /upp/v1/auth/keys/revoke — Revoke an API key by prefix.
pub async fn revoke_key(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let prefix = body.get("key_prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if prefix.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "key_prefix is required"
        }))).into_response();
    }

    let revoked = state.api_keys.revoke_by_prefix(prefix);

    if revoked {
        Json(serde_json::json!({
            "status": "revoked",
            "key_prefix": prefix,
        })).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "Key not found",
            "key_prefix": prefix,
        }))).into_response()
    }
}
