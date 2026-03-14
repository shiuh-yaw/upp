/// Type definitions for the UPP Gateway API
use serde::{Deserialize, Serialize};

// ============================================================================
// HEALTH & STATUS
// ============================================================================

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
}

/// Readiness check response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadyResponse {
    pub ready: bool,
}

/// Metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    pub data: serde_json::Value,
}

// ============================================================================
// MARKETS
// ============================================================================

/// Market listing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketsResponse {
    pub markets: Vec<Market>,
    pub pagination: Option<Pagination>,
}

/// Single market response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketResponse {
    pub market: Market,
}

/// Market data type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub provider: String,
    pub status: String,
    pub category: Option<String>,
    pub outcomes: Vec<MarketOutcome>,
    pub volume: Option<f64>,
    pub volume_24h: Option<f64>,
    pub created_at: Option<String>,
    pub closes_at: Option<String>,
}

/// Market outcome definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOutcome {
    pub id: String,
    pub title: String,
    pub price: Option<f64>,
}

/// Pagination info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: u32,
    pub cursor: Option<String>,
}

// ============================================================================
// ORDERBOOK
// ============================================================================

/// Orderbook response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookResponse {
    pub market_id: String,
    pub bids: Vec<OrderbookLevel>,
    pub asks: Vec<OrderbookLevel>,
    pub timestamp: String,
}

/// Single level in orderbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookLevel {
    pub price: f64,
    pub size: f64,
    pub count: Option<u32>,
}

// ============================================================================
// SEARCH
// ============================================================================

/// Market search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<Market>,
    pub total: u32,
}

// ============================================================================
// ARBITRAGE
// ============================================================================

/// Arbitrage opportunities list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageListResponse {
    pub opportunities: Vec<ArbitrageOpportunity>,
}

/// Single arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub market_id: String,
    pub exchange_a: String,
    pub exchange_b: String,
    pub profit_percentage: f64,
    pub profit_amount: f64,
    pub volume_available: f64,
}

/// Arbitrage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageSummaryResponse {
    pub total_opportunities: u32,
    pub total_profit_24h: f64,
    pub best_opportunity: Option<ArbitrageOpportunity>,
}

/// Arbitrage history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageHistoryResponse {
    pub entries: Vec<ArbitrageHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageHistoryEntry {
    pub id: String,
    pub market_id: String,
    pub profit: f64,
    pub timestamp: String,
}

// ============================================================================
// CANDLES
// ============================================================================

/// Candle data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlesResponse {
    pub market_id: String,
    pub outcome_id: String,
    pub resolution: String,
    pub candles: Vec<Candle>,
}

/// Single candle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Latest candle response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestCandleResponse {
    pub market_id: String,
    pub outcome_id: String,
    pub resolution: String,
    pub candle: Candle,
}

// ============================================================================
// PRICE INDEX
// ============================================================================

/// Price index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceIndexStatsResponse {
    pub index_id: String,
    pub price: f64,
    pub change_24h: f64,
    pub change_percent_24h: f64,
    pub high_24h: f64,
    pub low_24h: f64,
    pub volume_24h: f64,
}

// ============================================================================
// BACKTEST
// ============================================================================

/// Backtest strategy list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategiesResponse {
    pub strategies: Vec<Strategy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

/// Request to run backtest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBacktestRequest {
    pub strategy: String,
    pub market_id: String,
    pub outcome_id: String,
    pub resolution: String,
    pub params: Option<serde_json::Value>,
    pub initial_capital: f64,
    pub fee_rate: f64,
    pub slippage_rate: f64,
    pub max_position: Option<f64>,
}

/// Backtest result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResponse {
    pub strategy_id: String,
    pub market_id: String,
    pub total_return: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub trades: u32,
    pub pnl: f64,
}

/// Request to compare strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStrategiesRequest {
    pub market_id: String,
    pub outcome_id: String,
    pub resolution: String,
    pub strategies: Vec<String>,
}

/// Strategy comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStrategiesResponse {
    pub results: Vec<BacktestResponse>,
}

// ============================================================================
// FEEDS
// ============================================================================

/// Feed status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedStatusResponse {
    pub feeds: Vec<FeedInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub latency_ms: Option<f64>,
    pub last_update: Option<String>,
}

/// Feed statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedStatsResponse {
    pub total_feeds: u32,
    pub active_feeds: u32,
    pub total_messages: u64,
    pub uptime_percent: f64,
}

/// Feed subscription request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSubscriptionRequest {
    pub feed_ids: Vec<String>,
    pub market_ids: Option<Vec<String>>,
}

/// Feed subscription response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSubscriptionResponse {
    pub subscription_id: String,
    pub feeds: Vec<String>,
    pub status: String,
}

// ============================================================================
// ORDERS
// ============================================================================

/// Request to create an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    pub market_id: String,
    pub outcome_id: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub order_type: OrderType,
}

/// Order side enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderType {
    Limit,
    Market,
}

/// Order data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub market_id: String,
    pub outcome_id: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub filled: f64,
    pub price: f64,
    pub order_type: OrderType,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Orders list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersResponse {
    pub orders: Vec<Order>,
    pub pagination: Option<Pagination>,
}

/// Single order response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub order: Order,
}

/// Order estimation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateOrderRequest {
    pub market_id: String,
    pub outcome_id: String,
    pub side: OrderSide,
    pub quantity: f64,
}

/// Order estimation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateOrderResponse {
    pub estimated_price: f64,
    pub estimated_total: f64,
    pub fee: f64,
    pub slippage: f64,
}

// ============================================================================
// TRADES
// ============================================================================

/// Trade data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub order_id: String,
    pub market_id: String,
    pub outcome_id: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub fee: f64,
    pub timestamp: String,
}

/// Trades list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradesResponse {
    pub trades: Vec<Trade>,
    pub pagination: Option<Pagination>,
}

// ============================================================================
// PORTFOLIO
// ============================================================================

/// Portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub market_id: String,
    pub outcome_id: String,
    pub quantity: f64,
    pub average_entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
}

/// Positions list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionsResponse {
    pub positions: Vec<Position>,
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummaryResponse {
    pub total_balance: f64,
    pub available_balance: f64,
    pub total_positions_value: f64,
    pub total_unrealized_pnl: f64,
    pub total_realized_pnl: f64,
}

/// Portfolio balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub amount: f64,
    pub available: f64,
    pub locked: f64,
}

/// Balances response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancesResponse {
    pub balances: Vec<Balance>,
}

/// Portfolio analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsResponse {
    pub total_return: f64,
    pub daily_return: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
}

// ============================================================================
// ROUTING
// ============================================================================

/// Route computation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRouteRequest {
    pub market_id: String,
    pub outcome_id: String,
    pub side: OrderSide,
    pub quantity: f64,
}

/// Route step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub exchange: String,
    pub price: f64,
    pub quantity: f64,
}

/// Computed route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRouteResponse {
    pub route_id: String,
    pub steps: Vec<RouteStep>,
    pub total_price: f64,
    pub total_fees: f64,
    pub estimated_slippage: f64,
}

/// Route execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRouteRequest {
    pub route_id: String,
}

/// Route execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRouteResponse {
    pub execution_id: String,
    pub route_id: String,
    pub status: String,
    pub orders_created: u32,
}

/// Route statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStatsResponse {
    pub total_routes: u32,
    pub successful_executions: u32,
    pub failed_executions: u32,
    pub avg_execution_time_ms: f64,
}

// ============================================================================
// COMMON
// ============================================================================

/// Generic empty response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyResponse {}

/// Generic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: Option<String>,
    pub details: Option<serde_json::Value>,
}
