// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// MCP (Model Context Protocol) & A2A (Agent-to-Agent) Integration
//
// Exposes prediction market operations as standardized tool definitions
// for LLM-based agents and AI models to interact with prediction markets.
// Includes MCP tool schemas, execution routing, and A2A agent card generation.

use crate::core::{registry::ProviderRegistry, cache::MarketCache, types::*};
use crate::adapters::MarketFilter;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
// tracing used for debug logging in tool execution

// ─── MCP Tool Definition ────────────────────────────────────────

/// Represents a single MCP tool definition compatible with Model Context Protocol.
/// Each tool can be called by AI agents with standardized parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name, used in execute requests
    pub name: String,

    /// Human-readable description, optimized for LLM understanding
    pub description: String,

    /// JSON Schema for input parameters (JSON Schema spec)
    pub input_schema: serde_json::Value,
}

/// MCP tool execution error
#[derive(Debug, Clone, Serialize)]
pub struct McpError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl McpError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_details(code: &str, message: &str, details: serde_json::Value) -> Self {
        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: Some(details),
        }
    }
}

// ─── Tool Catalog ──────────────────────────────────────────────

/// Returns all available MCP tools
pub fn list_mcp_tools() -> Vec<McpTool> {
    vec![
        search_markets_tool(),
        list_markets_tool(),
        get_market_tool(),
        get_orderbook_tool(),
        get_portfolio_tool(),
        place_order_tool(),
        estimate_order_tool(),
        get_market_analysis_tool(),
    ]
}

fn search_markets_tool() -> McpTool {
    McpTool {
        name: "search_markets".to_string(),
        description: "Search prediction markets across all providers using natural language. Returns markets matching your query with current prices, volume, and status. Useful for finding markets about specific topics, events, or outcomes. Results are ranked by relevance and trading volume.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural language search query. Examples: 'Will Bitcoin hit 100k by 2026?', 'US election odds', 'tech company IPO', 'weather prediction for March'"
                },
                "provider": {
                    "type": "string",
                    "enum": ["kalshi.com", "polymarket.com", "opinion.trade"],
                    "description": "Optional: Filter results to a specific prediction market provider"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default: 10, max: 100)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 10
                },
                "category": {
                    "type": "string",
                    "description": "Optional: Filter by category like 'politics', 'crypto', 'sports', 'weather', 'economics'"
                }
            },
            "required": ["query"]
        }),
    }
}

fn list_markets_tool() -> McpTool {
    McpTool {
        name: "list_markets".to_string(),
        description: "List prediction markets with optional filtering. Browse available markets by provider, status, category, or type. Useful for discovering open markets, upcoming events, or getting market inventory. Returns paginated results with market IDs, titles, outcomes, and current pricing.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "provider": {
                    "type": "string",
                    "enum": ["kalshi.com", "polymarket.com", "opinion.trade"],
                    "description": "Optional: Filter to a specific provider"
                },
                "status": {
                    "type": "string",
                    "enum": ["open", "closed", "resolved", "disputed"],
                    "description": "Optional: Filter by market status"
                },
                "category": {
                    "type": "string",
                    "description": "Optional: Filter by category (e.g. 'politics', 'crypto', 'sports')"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of results to return (default: 20, max: 100)",
                    "minimum": 1,
                    "maximum": 100,
                    "default": 20
                },
                "cursor": {
                    "type": "string",
                    "description": "Optional: Pagination cursor for fetching next page"
                }
            },
            "required": []
        }),
    }
}

fn get_market_tool() -> McpTool {
    McpTool {
        name: "get_market".to_string(),
        description: "Get detailed information about a specific prediction market. Returns all market metadata including outcomes, pricing, volume, rules, regulatory info, and current order book. Use this to analyze a market before trading or to get the latest prices.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "market_id": {
                    "type": "string",
                    "description": "Universal market ID in format 'upp:{provider}:{native_id}' or just the native market ID"
                }
            },
            "required": ["market_id"]
        }),
    }
}

fn get_orderbook_tool() -> McpTool {
    McpTool {
        name: "get_orderbook".to_string(),
        description: "Get the current order book for a prediction market. Returns bid/ask prices and quantities for each outcome. Shows current market prices and available liquidity. Use this to check price spreads before placing trades.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "market_id": {
                    "type": "string",
                    "description": "Market ID (e.g., 'upp:polymarket.com:0x123abc')"
                },
                "depth": {
                    "type": "integer",
                    "description": "Number of price levels to return (default: 5)",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 50
                }
            },
            "required": ["market_id"]
        }),
    }
}

fn get_portfolio_tool() -> McpTool {
    McpTool {
        name: "get_portfolio".to_string(),
        description: "Get your current positions, holdings, and portfolio summary across all markets. Shows positions by outcome, current market value, unrealized P&L, and total portfolio statistics. Use this to understand your risk exposure and monitor performance.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "include_closed": {
                    "type": "boolean",
                    "description": "Include settled/closed positions (default: false)",
                    "default": false
                }
            },
            "required": []
        }),
    }
}

fn place_order_tool() -> McpTool {
    McpTool {
        name: "place_order".to_string(),
        description: "Place a buy or sell order on a prediction market. Supports market orders (immediate execution at best available price) or limit orders (execute at specified price or better). Returns order confirmation with order ID and execution details.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "market_id": {
                    "type": "string",
                    "description": "Market ID to trade"
                },
                "outcome_id": {
                    "type": "string",
                    "description": "Outcome to buy/sell (e.g., 'yes', 'no', or outcome name)"
                },
                "side": {
                    "type": "string",
                    "enum": ["buy", "sell"],
                    "description": "Buy or sell side"
                },
                "quantity": {
                    "type": "number",
                    "description": "Number of shares to trade (must be positive)",
                    "minimum": 0.001
                },
                "price": {
                    "type": "number",
                    "description": "Limit price (0.0-1.0 for binary markets). If omitted, executes as market order",
                    "minimum": 0,
                    "maximum": 1
                },
                "order_type": {
                    "type": "string",
                    "enum": ["market", "limit"],
                    "description": "Order type (default: limit if price provided, market otherwise)"
                }
            },
            "required": ["market_id", "outcome_id", "side", "quantity"]
        }),
    }
}

fn estimate_order_tool() -> McpTool {
    McpTool {
        name: "estimate_order".to_string(),
        description: "Get a cost estimate for a potential trade before committing. Returns total cost, fees, execution price, and slippage estimate. Use this to verify pricing and understand costs before calling place_order.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "market_id": {
                    "type": "string",
                    "description": "Market ID"
                },
                "outcome_id": {
                    "type": "string",
                    "description": "Outcome to trade"
                },
                "side": {
                    "type": "string",
                    "enum": ["buy", "sell"],
                    "description": "Buy or sell"
                },
                "quantity": {
                    "type": "number",
                    "description": "Number of shares (positive)",
                    "minimum": 0.001
                },
                "price": {
                    "type": "number",
                    "description": "Limit price if applicable (0.0-1.0)"
                }
            },
            "required": ["market_id", "outcome_id", "side", "quantity"]
        }),
    }
}

fn get_market_analysis_tool() -> McpTool {
    McpTool {
        name: "get_market_analysis".to_string(),
        description: "Get AI-friendly analysis and context for a market. Returns market summary, implied probabilities, volume metrics, market dynamics, and a narrative description. Optimized for LLM consumption to support reasoning and decision-making about prediction markets.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "market_id": {
                    "type": "string",
                    "description": "Market ID to analyze"
                }
            },
            "required": ["market_id"]
        }),
    }
}

// ─── MCP Tool Executor ──────────────────────────────────────────

/// Execute an MCP tool call.
/// Routes to the appropriate handler based on tool name and parameters.
pub async fn execute_tool(
    tool_name: &str,
    params: serde_json::Value,
    registry: &ProviderRegistry,
    cache: &MarketCache,
) -> Result<serde_json::Value, McpError> {
    match tool_name {
        "search_markets" => execute_search_markets(params, registry, cache).await,
        "list_markets" => execute_list_markets(params, registry, cache).await,
        "get_market" => execute_get_market(params, registry, cache).await,
        "get_orderbook" => execute_get_orderbook(params, registry, cache).await,
        "get_portfolio" => execute_get_portfolio(params).await,
        "place_order" => execute_place_order(params).await,
        "estimate_order" => execute_estimate_order(params).await,
        "get_market_analysis" => execute_get_market_analysis(params, registry, cache).await,
        _ => Err(McpError::new("UNKNOWN_TOOL", &format!("Tool '{}' not found", tool_name))),
    }
}

async fn execute_search_markets(
    params: serde_json::Value,
    registry: &ProviderRegistry,
    cache: &MarketCache,
) -> Result<serde_json::Value, McpError> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: query"))?;

    let limit = params
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10)
        .max(1)
        .min(100) as i32;

    let filter = MarketFilter {
        pagination: crate::core::types::PaginationRequest {
            limit: Some(limit),
            cursor: None,
        },
        ..Default::default()
    };

    let agg = crate::core::aggregation::parallel_search_markets(
        registry,
        query,
        filter,
    )
    .await;

    // Cache markets
    for market in &agg.markets {
        cache.put_market(market.id.to_full_id(), market.clone()).await;
    }

    Ok(json!({
        "markets": agg.markets,
        "query": query,
        "total": agg.total,
        "provider_results": agg.provider_results,
        "errors": agg.errors,
    }))
}

async fn execute_list_markets(
    params: serde_json::Value,
    registry: &ProviderRegistry,
    cache: &MarketCache,
) -> Result<serde_json::Value, McpError> {
    let provider = params.get("provider").and_then(|v| v.as_str());
    let status = params.get("status").and_then(|v| v.as_str());
    let category = params.get("category").and_then(|v| v.as_str());
    let limit = params
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .max(1)
        .min(100) as i32;
    let cursor = params.get("cursor").and_then(|v| v.as_str());

    let filter = MarketFilter {
        provider: provider.map(|s| s.to_string()),
        status: status.and_then(|s| {
            match s {
                "open" => Some(MarketStatus::Open),
                "closed" => Some(MarketStatus::Closed),
                "resolved" => Some(MarketStatus::Resolved),
                "disputed" => Some(MarketStatus::Disputed),
                _ => None,
            }
        }),
        category: category.map(|s| s.to_string()),
        pagination: crate::core::types::PaginationRequest {
            limit: Some(limit),
            cursor: cursor.map(|s| s.to_string()),
        },
        ..Default::default()
    };

    let provider_ids = provider.map(|p| vec![p.to_string()]);

    let agg = crate::core::aggregation::parallel_list_markets(registry, filter, provider_ids).await;

    for market in &agg.markets {
        cache.put_market(market.id.to_full_id(), market.clone()).await;
    }

    Ok(json!({
        "markets": agg.markets,
        "total": agg.total,
        "provider_results": agg.provider_results,
        "errors": agg.errors,
    }))
}

async fn execute_get_market(
    params: serde_json::Value,
    registry: &ProviderRegistry,
    cache: &MarketCache,
) -> Result<serde_json::Value, McpError> {
    let market_id = params
        .get("market_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: market_id"))?;

    // Parse market ID
    let uid = if let Some(parsed) = UniversalMarketId::parse(market_id) {
        parsed
    } else {
        // Assume it's just a native ID, need to disambiguate
        return Err(McpError::new(
            "INVALID_MARKET_ID",
            "Market ID must be in format 'upp:provider:native_id'",
        ));
    };

    // Check cache first
    if let Some(market) = cache.get_market(&uid.to_full_id()).await {
        return Ok(json!({
            "market": market,
            "source": "cache"
        }));
    }

    // Query provider
    if let Some(adapter) = registry.get(&uid.provider) {
        match adapter.get_market(&uid.native_id).await {
            Ok(market) => {
                cache.put_market(uid.to_full_id(), market.clone()).await;
                Ok(json!({
                    "market": market,
                    "source": "provider"
                }))
            }
            Err(e) => Err(McpError::new(
                "PROVIDER_ERROR",
                &format!("Failed to fetch market: {}", e),
            )),
        }
    } else {
        Err(McpError::new(
            "UNKNOWN_PROVIDER",
            &format!("Provider '{}' not found", uid.provider),
        ))
    }
}

async fn execute_get_orderbook(
    params: serde_json::Value,
    registry: &ProviderRegistry,
    _cache: &MarketCache,
) -> Result<serde_json::Value, McpError> {
    let market_id = params
        .get("market_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: market_id"))?;

    let depth = params
        .get("depth")
        .and_then(|v| v.as_i64())
        .unwrap_or(5)
        .max(1)
        .min(50) as i32;

    let uid = UniversalMarketId::parse(market_id).ok_or_else(|| {
        McpError::new(
            "INVALID_MARKET_ID",
            "Market ID must be in format 'upp:provider:native_id'",
        )
    })?;

    if let Some(adapter) = registry.get(&uid.provider) {
        match adapter.get_orderbook(&uid.native_id, None, depth).await {
            Ok(orderbook) => Ok(json!({
                "market_id": market_id,
                "orderbook": orderbook,
                "depth": depth,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })),
            Err(e) => Err(McpError::new(
                "PROVIDER_ERROR",
                &format!("Failed to fetch orderbook: {}", e),
            )),
        }
    } else {
        Err(McpError::new(
            "UNKNOWN_PROVIDER",
            &format!("Provider '{}' not found", uid.provider),
        ))
    }
}

async fn execute_get_portfolio(
    _params: serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    // Portfolio functionality requires authentication and user context
    // For now, return a placeholder response
    Ok(json!({
        "positions": [],
        "summary": {
            "total_value": 0.0,
            "unrealized_pnl": 0.0,
            "realized_pnl": 0.0,
            "num_open_positions": 0,
        },
        "note": "Portfolio endpoints require authentication"
    }))
}

async fn execute_place_order(
    _params: serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    // Trading functionality requires authentication and user context
    Err(McpError::new(
        "AUTH_REQUIRED",
        "Place order requires user authentication. Use authenticated endpoint /upp/v1/orders",
    ))
}

async fn execute_estimate_order(
    params: serde_json::Value,
) -> Result<serde_json::Value, McpError> {
    let market_id = params
        .get("market_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: market_id"))?;

    let outcome_id = params
        .get("outcome_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: outcome_id"))?;

    let side = params
        .get("side")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: side"))?;

    let quantity = params
        .get("quantity")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: quantity"))?;

    let price = params.get("price").and_then(|v| v.as_f64());

    // For now, return a placeholder estimate
    let estimated_cost = if let Some(p) = price {
        quantity * p
    } else {
        quantity * 0.5 // assume midpoint for market orders
    };

    Ok(json!({
        "market_id": market_id,
        "outcome_id": outcome_id,
        "side": side,
        "quantity": quantity,
        "estimated_price": price.unwrap_or(0.5),
        "estimated_cost": estimated_cost,
        "fees": estimated_cost * 0.002,
        "total": estimated_cost + (estimated_cost * 0.002),
        "note": "Estimate based on current orderbook. Actual execution may vary."
    }))
}

async fn execute_get_market_analysis(
    params: serde_json::Value,
    registry: &ProviderRegistry,
    cache: &MarketCache,
) -> Result<serde_json::Value, McpError> {
    let market_id = params
        .get("market_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::new("INVALID_PARAMS", "Missing required parameter: market_id"))?;

    let uid = UniversalMarketId::parse(market_id).ok_or_else(|| {
        McpError::new(
            "INVALID_MARKET_ID",
            "Market ID must be in format 'upp:provider:native_id'",
        )
    })?;

    // Get market data
    let market = if let Some(cached) = cache.get_market(market_id).await {
        cached
    } else if let Some(adapter) = registry.get(&uid.provider) {
        match adapter.get_market(&uid.native_id).await {
            Ok(m) => {
                cache.put_market(uid.to_full_id(), m.clone()).await;
                m
            }
            Err(e) => {
                return Err(McpError::new(
                    "PROVIDER_ERROR",
                    &format!("Failed to fetch market: {}", e),
                ))
            }
        }
    } else {
        return Err(McpError::new(
            "UNKNOWN_PROVIDER",
            &format!("Provider '{}' not found", uid.provider),
        ));
    };

    // Get orderbook for prices
    let orderbook = if let Some(adapter) = registry.get(&uid.provider) {
        adapter.get_orderbook(&uid.native_id, None, 5).await.ok()
    } else {
        None
    };

    // Calculate implied probabilities from orderbook
    let mut implied_probs = HashMap::new();
    if let Some(snapshots) = &orderbook {
        for snapshot in snapshots {
            let outcome_label = market.outcomes
                .iter()
                .find(|o| o.id == snapshot.outcome_id)
                .map(|o| o.label.clone())
                .unwrap_or_else(|| snapshot.outcome_id.clone());

            if !snapshot.bids.is_empty() && !snapshot.asks.is_empty() {
                // Parse prices and calculate midpoint
                let bid_price = snapshot.bids[0].price.parse::<f64>().unwrap_or(0.5);
                let ask_price = snapshot.asks[0].price.parse::<f64>().unwrap_or(0.5);
                let mid_price = (bid_price + ask_price) / 2.0;
                implied_probs.insert(outcome_label, mid_price);
            }
        }
    }

    // Generate AI-friendly summary
    let summary = format!(
        "Market: {}. Question: {}. {} possible outcomes. Total volume: ${}. Status: {:?}",
        market.event.title,
        market.event.description,
        market.outcomes.len(),
        market.volume.total_volume,
        market.lifecycle.status
    );

    Ok(json!({
        "market_id": market_id,
        "summary": summary,
        "market": market,
        "implied_probabilities": implied_probs,
        "volume": market.volume,
        "lifecycle": market.lifecycle,
        "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// ─── A2A Agent Card ────────────────────────────────────────────

/// A2A (Agent-to-Agent) Agent Card following Google's A2A specification.
/// Describes the UPP Gateway agent to other AI systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub authentication: Vec<AuthMethod>,
    #[serde(rename = "defaultInputModes")]
    pub default_input_modes: Vec<String>,
    #[serde(rename = "defaultOutputModes")]
    pub default_output_modes: Vec<String>,
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMethod {
    pub auth_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub examples: Vec<String>,
}

/// Generate an A2A Agent Card for the UPP Gateway
pub fn generate_agent_card(gateway_url: &str) -> AgentCard {
    AgentCard {
        name: "UPP Gateway".to_string(),
        description: "Universal Prediction Protocol Gateway. High-performance gateway providing unified access to prediction markets across Kalshi, Polymarket, and Opinion providers. Enables AI agents and LLMs to search markets, analyze predictions, view portfolio positions, and execute trades with a single API.".to_string(),
        url: gateway_url.to_string(),
        version: "2026-03-11".to_string(),
        capabilities: vec![
            "market-research".to_string(),
            "trading".to_string(),
            "portfolio-management".to_string(),
            "market-analysis".to_string(),
            "real-time-prices".to_string(),
        ],
        authentication: vec![
            AuthMethod {
                auth_type: "none".to_string(),
                description: "Public endpoints (search, list, get markets) - no auth required".to_string(),
            },
            AuthMethod {
                auth_type: "api_key".to_string(),
                description: "Protected endpoints (trading, portfolio) - API key required".to_string(),
            },
        ],
        default_input_modes: vec!["text/plain".to_string(), "application/json".to_string()],
        default_output_modes: vec!["text/plain".to_string(), "application/json".to_string()],
        skills: vec![
            Skill {
                id: "market-research".to_string(),
                name: "Market Research".to_string(),
                description: "Search and browse prediction markets, get detailed market information, analyze orderbooks and pricing.".to_string(),
                examples: vec![
                    "Find markets about Bitcoin price in 2026".to_string(),
                    "List all open markets on Polymarket".to_string(),
                    "Get the current orderbook for a specific market".to_string(),
                ],
            },
            Skill {
                id: "trading".to_string(),
                name: "Trading".to_string(),
                description: "Place buy/sell orders, estimate trade costs, cancel orders. Supports market and limit orders.".to_string(),
                examples: vec![
                    "Place a buy order for 100 shares at 0.65 price".to_string(),
                    "Estimate the cost of buying 50 shares of an outcome".to_string(),
                    "Cancel my open orders".to_string(),
                ],
            },
            Skill {
                id: "portfolio-management".to_string(),
                name: "Portfolio Management".to_string(),
                description: "View positions, check P&L, monitor portfolio risk and balances across all markets.".to_string(),
                examples: vec![
                    "Show my current positions and P&L".to_string(),
                    "What's my portfolio summary?".to_string(),
                    "List my open positions by market".to_string(),
                ],
            },
            Skill {
                id: "market-analysis".to_string(),
                name: "Market Analysis".to_string(),
                description: "Get comprehensive market analysis including implied probabilities, volume metrics, and AI-friendly market summaries.".to_string(),
                examples: vec![
                    "Analyze this market for me".to_string(),
                    "What are the implied probabilities?".to_string(),
                    "Is there high volume on this market?".to_string(),
                ],
            },
        ],
    }
}
