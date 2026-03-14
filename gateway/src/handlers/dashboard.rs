// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::response::{Html, IntoResponse};

/// GET /dashboard — Serve the monitoring dashboard.
pub async fn serve_dashboard() -> impl IntoResponse {
    Html(include_str!("../../static/dashboard.html"))
}
