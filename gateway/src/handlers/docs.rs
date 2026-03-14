// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::response::{Html, IntoResponse};

/// GET /docs — Serve Swagger UI.
pub async fn swagger_ui() -> impl IntoResponse {
    Html(include_str!("../../static/swagger.html"))
}

/// GET /openapi.json — Serve the OpenAPI 3.1 specification.
pub async fn openapi_spec() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        include_str!("../../static/openapi.json"),
    )
}
