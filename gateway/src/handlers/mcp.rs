// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0

use axum::{extract::State, response::IntoResponse, Json};
use axum::http::StatusCode;
use serde::Deserialize;
use std::sync::atomic::Ordering;

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct McpExecuteRequest {
    pub tool: String,
    pub params: serde_json::Value,
}

/// GET /upp/v1/mcp/tools
pub async fn list_tools() -> impl IntoResponse {
    let tools = crate::core::mcp::list_mcp_tools();
    Json(serde_json::json!({
        "tools": tools,
        "total": tools.len(),
        "mcp_version": "2024-11-05",
    }))
}

/// GET /upp/v1/mcp/schema
pub async fn get_schema() -> impl IntoResponse {
    let tools = crate::core::mcp::list_mcp_tools();
    let mut definitions = serde_json::Map::new();

    for tool in &tools {
        definitions.insert(tool.name.clone(), tool.input_schema.clone());
    }

    Json(serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "UPP Gateway MCP API",
            "description": "Model Context Protocol tools for prediction market interactions",
            "version": "2026-03-11",
        },
        "servers": [{ "url": "/upp/v1/mcp", "description": "MCP endpoint" }],
        "x-mcp-tools": tools,
        "components": { "schemas": definitions }
    }))
}

/// POST /upp/v1/mcp/execute
pub async fn execute_tool(
    State(state): State<AppState>,
    Json(req): Json<McpExecuteRequest>,
) -> impl IntoResponse {
    match crate::core::mcp::execute_tool(
        &req.tool, req.params, &state.registry, &state.cache,
    ).await {
        Ok(result) => {
            state.metrics.requests_ok.fetch_add(1, Ordering::Relaxed);
            (StatusCode::OK, Json(serde_json::json!({
                "tool": req.tool,
                "result": result,
                "status": "ok",
            }))).into_response()
        }
        Err(e) => {
            state.metrics.requests_err.fetch_add(1, Ordering::Relaxed);
            (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": { "code": e.code, "message": e.message, "details": e.details }
            }))).into_response()
        }
    }
}

/// GET /.well-known/agent.json
pub async fn get_agent_card(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let gateway_url = format!(
        "http://{}:{}/upp/v1/mcp",
        state.config.host, state.config.port,
    );
    let card = crate::core::mcp::generate_agent_card(&gateway_url);
    Json(card)
}
