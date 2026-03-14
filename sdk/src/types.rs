//! Type definitions for the UPP Gateway API

use serde::{Deserialize, Serialize};

// ============================================================================
// HEALTH & STATUS
// ============================================================================

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    /// Current health status (e.g. "healthy")
    pub status: String,
}

/// Readiness check response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadyResponse {
    /// Whether the gateway is ready to serve requests
    pub ready: bool,
}

/// Metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsResponse {
    /// Arbitrary metrics data as a JSON value
    pub data: serde_json::Value,
}

// ============================================================================
// MARKETS
// ============================================================================

/// Market listing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketsResponse {
    /// List of markets
    pub markets: Vec<Market>,
    /// Pagination info for the result set
    pub pagination: Option<Pagination>,
}

/// Single market response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketResponse {
    /// The requested market
    pub market: Market,
}

/// Market data type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    /// Unique market identifier (e.g. "kalshi.com:BTC-2026-Q1")
    pub id: String,
    /// Human-readable market title
    pub title: String,
    /// Optional detailed description
    pub description: Option<String>,
    /// Provider that hosts this market (e.g. "kalshi.com")
    pub provider: String,
    /// Current market status (e.g. "open", "closed")
    pub status: String,
    /// Market category (e.g. "crypto", "politics")
    pub category: Option<String>,
    /// Available outcomes for the market
    pub outcomes: Vec<MarketOutcome>,
    /// Total trading volume
    pub volume: Option<f64>,
    /// Trading volume in the last 24 hours
    pub volume_24h: Option<f64>,
    /// ISO 8601 timestamp when the market was created
    pub created_at: Option<String>,
    /// ISO 8601 timestamp when the market closes
    pub closes_at: Option<String>,
}

/// Market outcome definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOutcome {
    /// Unique outcome identifier (e.g. "yes", "no")
    pub id: String,
    /// Human-readable outcome title
    pub title: String,
    /// Current price of this outcome (0.0 to 1.0)
    pub price: Option<f64>,
}

/// Pagination info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Maximum number of results per page
    pub limit: u32,
    /// Cursor for the next page of results
    pub cursor: Option<String>,
}

// ============================================================================
// ORDERBOOK
// ============================================================================

/// Orderbook response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookResponse {
    /// Market identifier this orderbook belongs to
    pub market_id: String,
    /// Bid levels sorted by price descending
    pub bids: Vec<OrderbookLevel>,
    /// Ask levels sorted by price ascending
    pub asks: Vec<OrderbookLevel>,
    /// ISO 8601 timestamp of this orderbook snapshot
    pub timestamp: String,
}

/// Single level in orderbook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookLevel {
    /// Price at this level
    pub price: f64,
    /// Total size at this level
    pub size: f64,
    /// Number of orders at this level
    pub count: Option<u32>,
}

// ============================================================================
// SEARCH
// ============================================================================

/// Market search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// Matching markets
    pub results: Vec<Market>,
    /// Total number of matches
    pub total: u32,
}

// ============================================================================
// ARBITRAGE
// ============================================================================

/// Arbitrage opportunities list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageListResponse {
    /// Available arbitrage opportunities
    pub opportunities: Vec<ArbitrageOpportunity>,
}

/// Single arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Unique opportunity identifier
    pub id: String,
    /// Market this opportunity relates to
    pub market_id: String,
    /// First exchange in the arbitrage pair
    pub exchange_a: String,
    /// Second exchange in the arbitrage pair
    pub exchange_b: String,
    /// Profit as a percentage
    pub profit_percentage: f64,
    /// Absolute profit amount
    pub profit_amount: f64,
    /// Available volume for this opportunity
    pub volume_available: f64,
}

/// Arbitrage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageSummaryResponse {
    /// Total number of active arbitrage opportunities
    pub total_opportunities: u32,
    /// Total profit captured in the last 24 hours
    pub total_profit_24h: f64,
    /// Best available arbitrage opportunity, if any
    pub best_opportunity: Option<ArbitrageOpportunity>,
}

/// Arbitrage history response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageHistoryResponse {
    /// Historical arbitrage entries
    pub entries: Vec<ArbitrageHistoryEntry>,
}

/// A single arbitrage history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageHistoryEntry {
    /// Unique entry identifier
    pub id: String,
    /// Market this entry relates to
    pub market_id: String,
    /// Profit captured from the arbitrage
    pub profit: f64,
    /// ISO 8601 timestamp of the arbitrage execution
    pub timestamp: String,
}

// ============================================================================
// CANDLES
// ============================================================================

/// Candle data response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlesResponse {
    /// Market identifier for these candles
    pub market_id: String,
    /// Outcome identifier for these candles
    pub outcome_id: String,
    /// Candle resolution (e.g. "1m", "5m", "1h", "1d")
    pub resolution: String,
    /// Candle data points
    pub candles: Vec<Candle>,
}

/// Single candle (OHLCV)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    /// ISO 8601 timestamp for this candle period
    pub timestamp: String,
    /// Opening price
    pub open: f64,
    /// Highest price during the period
    pub high: f64,
    /// Lowest price during the period
    pub low: f64,
    /// Closing price
    pub close: f64,
    /// Trading volume during the period
    pub volume: f64,
}

/// Latest candle response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestCandleResponse {
    /// Market identifier
    pub market_id: String,
    /// Outcome identifier
    pub outcome_id: String,
    /// Candle resolution
    pub resolution: String,
    /// The latest candle
    pub candle: Candle,
}

// ============================================================================
// PRICE INDEX
// ============================================================================

/// Price index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceIndexStatsResponse {
    /// Index identifier (e.g. "upp-global")
    pub index_id: String,
    /// Current index price
    pub price: f64,
    /// Absolute price change in the last 24 hours
    pub change_24h: f64,
    /// Percentage price change in the last 24 hours
    pub change_percent_24h: f64,
    /// Highest index price in the last 24 hours
    pub high_24h: f64,
    /// Lowest index price in the last 24 hours
    pub low_24h: f64,
    /// Total trading volume in the last 24 hours
    pub volume_24h: f64,
}

// ============================================================================
// BACKTEST
// ============================================================================

/// Backtest strategy list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategiesResponse {
    /// Available backtest strategies
    pub strategies: Vec<Strategy>,
}

/// A backtesting strategy definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    /// Unique strategy identifier
    pub id: String,
    /// Human-readable strategy name
    pub name: String,
    /// Optional strategy description
    pub description: Option<String>,
    /// Optional strategy parameters as a JSON value
    pub parameters: Option<serde_json::Value>,
}

/// Request to run backtest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunBacktestRequest {
    /// Strategy identifier to use
    pub strategy: String,
    /// Market to backtest against
    pub market_id: String,
    /// Outcome to backtest against
    pub outcome_id: String,
    /// Candle resolution for the backtest
    pub resolution: String,
    /// Optional strategy-specific parameters
    pub params: Option<serde_json::Value>,
    /// Starting capital in dollars
    pub initial_capital: f64,
    /// Fee per trade as a fraction (e.g. 0.02 = 2%)
    pub fee_rate: f64,
    /// Slippage per trade as a fraction (e.g. 0.005 = 0.5%)
    pub slippage_rate: f64,
    /// Maximum position size in contracts
    pub max_position: Option<f64>,
}

/// Backtest result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResponse {
    /// Strategy used for this backtest
    pub strategy_id: String,
    /// Market backtested against
    pub market_id: String,
    /// Total return as a percentage
    pub total_return: f64,
    /// Annualized Sharpe ratio
    pub sharpe_ratio: f64,
    /// Maximum drawdown as a percentage
    pub max_drawdown: f64,
    /// Win rate as a percentage
    pub win_rate: f64,
    /// Total number of trades executed
    pub trades: u32,
    /// Total profit and loss
    pub pnl: f64,
}

/// Request to compare strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStrategiesRequest {
    /// Market to compare strategies against
    pub market_id: String,
    /// Outcome to compare strategies against
    pub outcome_id: String,
    /// Candle resolution for comparison
    pub resolution: String,
    /// List of strategy identifiers to compare
    pub strategies: Vec<String>,
}

/// Strategy comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStrategiesResponse {
    /// Backtest results for each compared strategy
    pub results: Vec<BacktestResponse>,
}

// ============================================================================
// FEEDS
// ============================================================================

/// Feed status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedStatusResponse {
    /// Status of all registered feeds
    pub feeds: Vec<FeedInfo>,
}

/// Information about a single data feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedInfo {
    /// Unique feed identifier
    pub id: String,
    /// Human-readable feed name
    pub name: String,
    /// Current feed status (e.g. "connected", "disconnected")
    pub status: String,
    /// Current feed latency in milliseconds
    pub latency_ms: Option<f64>,
    /// ISO 8601 timestamp of the last received update
    pub last_update: Option<String>,
}

/// Feed statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedStatsResponse {
    /// Total number of registered feeds
    pub total_feeds: u32,
    /// Number of currently active feeds
    pub active_feeds: u32,
    /// Total messages received across all feeds
    pub total_messages: u64,
    /// Overall uptime percentage across all feeds
    pub uptime_percent: f64,
}

/// Feed subscription request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSubscriptionRequest {
    /// Feed identifiers to subscribe to
    pub feed_ids: Vec<String>,
    /// Optional market filter for the subscription
    pub market_ids: Option<Vec<String>>,
}

/// Feed subscription response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedSubscriptionResponse {
    /// Unique subscription identifier
    pub subscription_id: String,
    /// Feeds included in this subscription
    pub feeds: Vec<String>,
    /// Subscription status
    pub status: String,
}

// ============================================================================
// ORDERS
// ============================================================================

/// Request to create an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderRequest {
    /// Target market identifier
    pub market_id: String,
    /// Target outcome identifier
    pub outcome_id: String,
    /// Order side (buy or sell)
    pub side: OrderSide,
    /// Number of contracts to trade
    pub quantity: f64,
    /// Limit price per contract
    pub price: f64,
    /// Order type (limit or market)
    pub order_type: OrderType,
}

/// Order side enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderSide {
    /// Buy side
    Buy,
    /// Sell side
    Sell,
}

/// Order type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum OrderType {
    /// Limit order with specified price
    Limit,
    /// Market order at best available price
    Market,
}

/// Order data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Unique order identifier
    pub id: String,
    /// Market this order belongs to
    pub market_id: String,
    /// Outcome this order targets
    pub outcome_id: String,
    /// Order side
    pub side: OrderSide,
    /// Total quantity ordered
    pub quantity: f64,
    /// Quantity filled so far
    pub filled: f64,
    /// Order price
    pub price: f64,
    /// Order type
    pub order_type: OrderType,
    /// Current order status (e.g. "open", "filled", "cancelled")
    pub status: String,
    /// ISO 8601 timestamp when the order was created
    pub created_at: String,
    /// ISO 8601 timestamp of the last update
    pub updated_at: String,
}

/// Orders list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrdersResponse {
    /// List of orders
    pub orders: Vec<Order>,
    /// Pagination info
    pub pagination: Option<Pagination>,
}

/// Single order response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    /// The requested order
    pub order: Order,
}

/// Order estimation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateOrderRequest {
    /// Target market identifier
    pub market_id: String,
    /// Target outcome identifier
    pub outcome_id: String,
    /// Order side
    pub side: OrderSide,
    /// Desired quantity
    pub quantity: f64,
}

/// Order estimation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateOrderResponse {
    /// Estimated average fill price
    pub estimated_price: f64,
    /// Estimated total cost
    pub estimated_total: f64,
    /// Estimated fee
    pub fee: f64,
    /// Estimated slippage
    pub slippage: f64,
}

// ============================================================================
// TRADES
// ============================================================================

/// Trade data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Unique trade identifier
    pub id: String,
    /// Order that generated this trade
    pub order_id: String,
    /// Market where the trade occurred
    pub market_id: String,
    /// Outcome traded
    pub outcome_id: String,
    /// Trade side
    pub side: OrderSide,
    /// Trade quantity
    pub quantity: f64,
    /// Execution price
    pub price: f64,
    /// Fee charged for this trade
    pub fee: f64,
    /// ISO 8601 timestamp of execution
    pub timestamp: String,
}

/// Trades list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradesResponse {
    /// List of trades
    pub trades: Vec<Trade>,
    /// Pagination info
    pub pagination: Option<Pagination>,
}

// ============================================================================
// PORTFOLIO
// ============================================================================

/// Portfolio position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Market identifier for this position
    pub market_id: String,
    /// Outcome identifier for this position
    pub outcome_id: String,
    /// Number of contracts held
    pub quantity: f64,
    /// Average price paid per contract
    pub average_entry_price: f64,
    /// Current market price per contract
    pub current_price: f64,
    /// Unrealized profit and loss
    pub unrealized_pnl: f64,
}

/// Positions list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionsResponse {
    /// List of open positions
    pub positions: Vec<Position>,
}

/// Portfolio summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummaryResponse {
    /// Total portfolio balance (cash + positions)
    pub total_balance: f64,
    /// Available cash balance
    pub available_balance: f64,
    /// Total value of open positions
    pub total_positions_value: f64,
    /// Total unrealized profit and loss
    pub total_unrealized_pnl: f64,
    /// Total realized profit and loss
    pub total_realized_pnl: f64,
}

/// Portfolio balance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    /// Asset symbol (e.g. "USD", "USDC")
    pub asset: String,
    /// Total amount held
    pub amount: f64,
    /// Amount available for trading
    pub available: f64,
    /// Amount locked in open orders
    pub locked: f64,
}

/// Balances response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalancesResponse {
    /// List of asset balances
    pub balances: Vec<Balance>,
}

/// Portfolio analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsResponse {
    /// Total return as a percentage
    pub total_return: f64,
    /// Daily return as a percentage
    pub daily_return: f64,
    /// Annualized Sharpe ratio
    pub sharpe_ratio: f64,
    /// Maximum drawdown as a percentage
    pub max_drawdown: f64,
    /// Win rate as a percentage
    pub win_rate: f64,
}

// ============================================================================
// ROUTING
// ============================================================================

/// Route computation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRouteRequest {
    /// Target market identifier
    pub market_id: String,
    /// Target outcome identifier
    pub outcome_id: String,
    /// Order side
    pub side: OrderSide,
    /// Desired quantity
    pub quantity: f64,
}

/// Route step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    /// Exchange where this step executes
    pub exchange: String,
    /// Price at this step
    pub price: f64,
    /// Quantity to fill at this step
    pub quantity: f64,
}

/// Computed route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeRouteResponse {
    /// Unique route identifier
    pub route_id: String,
    /// Ordered steps to execute
    pub steps: Vec<RouteStep>,
    /// Total price across all steps
    pub total_price: f64,
    /// Total fees across all steps
    pub total_fees: f64,
    /// Estimated slippage as a fraction
    pub estimated_slippage: f64,
}

/// Route execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRouteRequest {
    /// Route identifier to execute
    pub route_id: String,
}

/// Route execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRouteResponse {
    /// Unique execution identifier
    pub execution_id: String,
    /// Route that was executed
    pub route_id: String,
    /// Execution status (e.g. "submitted", "completed")
    pub status: String,
    /// Number of orders created across exchanges
    pub orders_created: u32,
}

/// Route statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStatsResponse {
    /// Total routes computed
    pub total_routes: u32,
    /// Successfully executed routes
    pub successful_executions: u32,
    /// Failed route executions
    pub failed_executions: u32,
    /// Average execution time in milliseconds
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
    /// Error message
    pub error: String,
    /// Optional error code
    pub code: Option<String>,
    /// Optional additional error details
    pub details: Option<serde_json::Value>,
}
