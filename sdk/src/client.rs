//! HTTP client implementation for the UPP Gateway API

use crate::error::{Result, UppSdkError};
use crate::types::*;
use reqwest::{Client as HttpClient, StatusCode};
use serde::de::DeserializeOwned;
use std::time::Duration;
use url::Url;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_BASE_URL: &str = "http://localhost:9090";

/// Main client for interacting with the UPP Gateway API
///
/// # Example
///
/// ```no_run
/// use upp_sdk::UppClient;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = UppClient::builder()
///         .base_url("http://localhost:9090")
///         .build()?;
///
///     let health = client.health().await?;
///     println!("Gateway health: {:?}", health);
///
///     Ok(())
/// }
/// ```
#[derive(Clone, Debug)]
pub struct UppClient {
    /// Underlying HTTP client
    http_client: HttpClient,
    /// Base URL for the UPP Gateway API
    base_url: Url,
    /// Optional API key for authenticated requests
    api_key: Option<String>,
}

/// Builder for constructing a UppClient with custom configuration
///
/// # Example
///
/// ```no_run
/// use upp_sdk::UppClient;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = UppClient::builder()
///         .base_url("http://localhost:9090")
///         .api_key("my-api-key")
///         .timeout(Duration::from_secs(60))
///         .build()?;
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct UppClientBuilder {
    /// Base URL for the UPP Gateway API
    base_url: String,
    /// Optional API key for authenticated requests
    api_key: Option<String>,
    /// Request timeout duration
    timeout: Duration,
}

impl Default for UppClientBuilder {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl UppClientBuilder {
    /// Create a new builder with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base URL for the API
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the API key for authenticated requests
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the request timeout
    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    /// Build the UppClient
    pub fn build(self) -> Result<UppClient> {
        let base_url = Url::parse(&self.base_url)?;

        let http_client = HttpClient::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| UppSdkError::ConfigError(e.to_string()))?;

        Ok(UppClient {
            http_client,
            base_url,
            api_key: self.api_key,
        })
    }
}

impl UppClient {
    /// Create a new client with the default base URL
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        UppClientBuilder::new().base_url(base_url).build()
    }

    /// Create a builder for configuring a client
    pub fn builder() -> UppClientBuilder {
        UppClientBuilder::new()
    }

    /// Get the base URL
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    // ========================================================================
    // HEALTH & STATUS ENDPOINTS
    // ========================================================================

    /// Check gateway health
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use upp_sdk::UppClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = UppClient::new("http://localhost:9090")?;
    /// let health = client.health().await?;
    /// assert_eq!(health.status, "healthy");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health(&self) -> Result<HealthResponse> {
        self.get("/health").await
    }

    /// Check gateway readiness
    pub async fn ready(&self) -> Result<ReadyResponse> {
        self.get("/ready").await
    }

    /// Get gateway metrics
    pub async fn metrics(&self) -> Result<MetricsResponse> {
        self.get("/metrics").await
    }

    // ========================================================================
    // MARKETS ENDPOINTS
    // ========================================================================

    /// List all markets
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use upp_sdk::UppClient;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = UppClient::new("http://localhost:9090")?;
    /// let markets = client.list_markets(None, None, None, None, None).await?;
    /// println!("Markets: {:?}", markets.markets);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_markets(
        &self,
        provider: Option<&str>,
        status: Option<&str>,
        category: Option<&str>,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<MarketsResponse> {
        let mut url = self.build_url("/upp/v1/markets")?;
        if let Some(p) = provider {
            let _ = url.query_pairs_mut().append_pair("provider", p);
        }
        if let Some(s) = status {
            let _ = url.query_pairs_mut().append_pair("status", s);
        }
        if let Some(c) = category {
            let _ = url.query_pairs_mut().append_pair("category", c);
        }
        if let Some(l) = limit {
            let _ = url.query_pairs_mut().append_pair("limit", &l.to_string());
        }
        if let Some(cur) = cursor {
            let _ = url.query_pairs_mut().append_pair("cursor", cur);
        }
        self.get_url(&url).await
    }

    /// Get a specific market by ID
    pub async fn get_market(&self, market_id: &str) -> Result<MarketResponse> {
        self.get(&format!("/upp/v1/markets/{}", market_id)).await
    }

    /// Get orderbook for a market
    pub async fn get_orderbook(&self, market_id: &str) -> Result<OrderbookResponse> {
        self.get(&format!("/upp/v1/markets/{}/orderbook", market_id))
            .await
    }

    /// Search markets by query
    pub async fn search_markets(
        &self,
        q: Option<&str>,
        provider: Option<&str>,
        category: Option<&str>,
        limit: Option<u32>,
    ) -> Result<SearchResponse> {
        let mut url = self.build_url("/upp/v1/markets/search")?;
        if let Some(query) = q {
            let _ = url.query_pairs_mut().append_pair("q", query);
        }
        if let Some(p) = provider {
            let _ = url.query_pairs_mut().append_pair("provider", p);
        }
        if let Some(c) = category {
            let _ = url.query_pairs_mut().append_pair("category", c);
        }
        if let Some(l) = limit {
            let _ = url.query_pairs_mut().append_pair("limit", &l.to_string());
        }
        self.get_url(&url).await
    }

    // ========================================================================
    // ARBITRAGE ENDPOINTS
    // ========================================================================

    /// List arbitrage opportunities
    pub async fn list_arbitrage(&self) -> Result<ArbitrageListResponse> {
        self.get("/upp/v1/arbitrage").await
    }

    /// Get arbitrage summary
    pub async fn arbitrage_summary(&self) -> Result<ArbitrageSummaryResponse> {
        self.get("/upp/v1/arbitrage/summary").await
    }

    /// Get arbitrage history
    pub async fn arbitrage_history(&self, limit: Option<u32>) -> Result<ArbitrageHistoryResponse> {
        let mut url = self.build_url("/upp/v1/arbitrage/history")?;
        if let Some(l) = limit {
            let _ = url.query_pairs_mut().append_pair("limit", &l.to_string());
        }
        self.get_url(&url).await
    }

    // ========================================================================
    // CANDLES ENDPOINTS
    // ========================================================================

    /// Get candle data for a market outcome
    pub async fn get_candles(
        &self,
        market_id: &str,
        outcome_id: Option<&str>,
        resolution: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        limit: Option<u32>,
    ) -> Result<CandlesResponse> {
        let mut url = self.build_url(&format!("/upp/v1/markets/{}/candles", market_id))?;
        if let Some(o) = outcome_id {
            let _ = url.query_pairs_mut().append_pair("outcome_id", o);
        }
        if let Some(r) = resolution {
            let _ = url.query_pairs_mut().append_pair("resolution", r);
        }
        if let Some(f) = from {
            let _ = url.query_pairs_mut().append_pair("from", f);
        }
        if let Some(t) = to {
            let _ = url.query_pairs_mut().append_pair("to", t);
        }
        if let Some(l) = limit {
            let _ = url.query_pairs_mut().append_pair("limit", &l.to_string());
        }
        self.get_url(&url).await
    }

    /// Get latest candle for a market outcome
    pub async fn get_latest_candle(
        &self,
        market_id: &str,
        outcome_id: Option<&str>,
        resolution: Option<&str>,
    ) -> Result<LatestCandleResponse> {
        let mut url = self.build_url(&format!("/upp/v1/markets/{}/candles/latest", market_id))?;
        if let Some(o) = outcome_id {
            let _ = url.query_pairs_mut().append_pair("outcome_id", o);
        }
        if let Some(r) = resolution {
            let _ = url.query_pairs_mut().append_pair("resolution", r);
        }
        self.get_url(&url).await
    }

    // ========================================================================
    // PRICE INDEX ENDPOINTS
    // ========================================================================

    /// Get price index statistics
    pub async fn price_index_stats(&self) -> Result<PriceIndexStatsResponse> {
        self.get("/upp/v1/price-index/stats").await
    }

    // ========================================================================
    // BACKTEST ENDPOINTS
    // ========================================================================

    /// List available backtest strategies
    pub async fn list_strategies(&self) -> Result<StrategiesResponse> {
        self.get("/upp/v1/backtest/strategies").await
    }

    /// Run a backtest
    pub async fn run_backtest(&self, request: RunBacktestRequest) -> Result<BacktestResponse> {
        self.post("/upp/v1/backtest/run", &request).await
    }

    /// Compare multiple strategies
    pub async fn compare_strategies(
        &self,
        request: CompareStrategiesRequest,
    ) -> Result<CompareStrategiesResponse> {
        self.post("/upp/v1/backtest/compare", &request).await
    }

    // ========================================================================
    // FEEDS ENDPOINTS
    // ========================================================================

    /// Get feed status
    pub async fn feed_status(&self) -> Result<FeedStatusResponse> {
        self.get("/upp/v1/feeds/status").await
    }

    /// Get feed statistics
    pub async fn feed_stats(&self) -> Result<FeedStatsResponse> {
        self.get("/upp/v1/feeds/stats").await
    }

    /// Subscribe to feeds (requires authentication)
    pub async fn subscribe_feeds(
        &self,
        request: FeedSubscriptionRequest,
    ) -> Result<FeedSubscriptionResponse> {
        self.post_authenticated("/upp/v1/feeds/subscribe", &request)
            .await
    }

    // ========================================================================
    // ORDERS ENDPOINTS (AUTHENTICATED)
    // ========================================================================

    /// Create a new order
    pub async fn create_order(&self, request: CreateOrderRequest) -> Result<OrderResponse> {
        self.post_authenticated("/upp/v1/orders", &request).await
    }

    /// List all orders
    pub async fn list_orders(&self) -> Result<OrdersResponse> {
        self.get_authenticated("/upp/v1/orders").await
    }

    /// Get a specific order
    pub async fn get_order(&self, order_id: &str) -> Result<OrderResponse> {
        self.get_authenticated(&format!("/upp/v1/orders/{}", order_id))
            .await
    }

    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<OrderResponse> {
        self.delete_authenticated(&format!("/upp/v1/orders/{}", order_id))
            .await
    }

    /// Cancel all orders
    pub async fn cancel_all_orders(&self) -> Result<OrdersResponse> {
        self.post_authenticated("/upp/v1/orders/cancel-all", &EmptyResponse {})
            .await
    }

    /// Estimate an order
    pub async fn estimate_order(
        &self,
        request: EstimateOrderRequest,
    ) -> Result<EstimateOrderResponse> {
        self.post_authenticated("/upp/v1/orders/estimate", &request)
            .await
    }

    // ========================================================================
    // TRADES ENDPOINTS (AUTHENTICATED)
    // ========================================================================

    /// List all trades
    pub async fn list_trades(&self) -> Result<TradesResponse> {
        self.get_authenticated("/upp/v1/trades").await
    }

    // ========================================================================
    // PORTFOLIO ENDPOINTS (AUTHENTICATED)
    // ========================================================================

    /// Get portfolio positions
    pub async fn get_positions(&self) -> Result<PositionsResponse> {
        self.get_authenticated("/upp/v1/portfolio/positions").await
    }

    /// Get portfolio summary
    pub async fn portfolio_summary(&self) -> Result<PortfolioSummaryResponse> {
        self.get_authenticated("/upp/v1/portfolio/summary").await
    }

    /// Get portfolio balances
    pub async fn get_balances(&self) -> Result<BalancesResponse> {
        self.get_authenticated("/upp/v1/portfolio/balances").await
    }

    /// Get portfolio analytics
    pub async fn portfolio_analytics(&self) -> Result<AnalyticsResponse> {
        self.get_authenticated("/upp/v1/portfolio/analytics").await
    }

    // ========================================================================
    // ROUTING ENDPOINTS (AUTHENTICATED)
    // ========================================================================

    /// Compute route for an order
    pub async fn compute_route(&self, request: ComputeRouteRequest) -> Result<ComputeRouteResponse> {
        self.post_authenticated("/upp/v1/orders/route", &request)
            .await
    }

    /// Execute a computed route
    pub async fn execute_route(&self, request: ExecuteRouteRequest) -> Result<ExecuteRouteResponse> {
        self.post_authenticated("/upp/v1/orders/route/execute", &request)
            .await
    }

    /// Get routing statistics
    pub async fn route_stats(&self) -> Result<RouteStatsResponse> {
        self.get_authenticated("/upp/v1/orders/route/stats").await
    }

    // ========================================================================
    // INTERNAL HTTP METHODS
    // ========================================================================

    fn build_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(path)
            .map_err(UppSdkError::InvalidUrl)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path)?;
        self.get_url(&url).await
    }

    async fn get_url<T: DeserializeOwned>(&self, url: &Url) -> Result<T> {
        let response = self.http_client.get(url.clone()).send().await?;
        self.handle_response(response).await
    }

    async fn get_authenticated<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path)?;
        let response = self
            .http_client
            .get(url)
            .header("Authorization", self.auth_header()?)
            .send()
            .await?;
        self.handle_response(response).await
    }

    async fn post<T: serde::Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        let url = self.build_url(path)?;
        let response = self.http_client.post(url).json(body).send().await?;
        self.handle_response(response).await
    }

    async fn post_authenticated<T: serde::Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        let url = self.build_url(path)?;
        let response = self
            .http_client
            .post(url)
            .header("Authorization", self.auth_header()?)
            .json(body)
            .send()
            .await?;
        self.handle_response(response).await
    }

    async fn delete_authenticated<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path)?;
        let response = self
            .http_client
            .delete(url)
            .header("Authorization", self.auth_header()?)
            .send()
            .await?;
        self.handle_response(response).await
    }

    fn auth_header(&self) -> Result<String> {
        let key = self
            .api_key
            .as_ref()
            .ok_or_else(|| UppSdkError::MissingParameter("api_key".to_string()))?;
        Ok(format!("Bearer {}", key))
    }

    async fn handle_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        match status {
            StatusCode::OK | StatusCode::CREATED => {
                let body = response.text().await?;
                serde_json::from_str(&body).map_err(|e| {
                    UppSdkError::UnexpectedResponse(format!(
                        "Failed to parse response: {}, body: {}",
                        e, body
                    ))
                })
            }
            _ => {
                let body = response.text().await.unwrap_or_default();
                Err(UppSdkError::ApiError {
                    status: status.as_u16(),
                    body,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_builder_default() {
        let builder = UppClientBuilder::new();
        assert_eq!(builder.base_url, DEFAULT_BASE_URL);
        assert_eq!(builder.api_key, None);
        assert_eq!(builder.timeout, DEFAULT_TIMEOUT);
    }

    #[test]
    fn test_client_builder_with_settings() {
        let builder = UppClientBuilder::new()
            .base_url("http://example.com")
            .api_key("test-key")
            .timeout(Duration::from_secs(60));

        assert_eq!(builder.base_url, "http://example.com");
        assert_eq!(builder.api_key, Some("test-key".to_string()));
        assert_eq!(builder.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_client_builder_build() {
        let client = UppClientBuilder::new()
            .base_url("http://localhost:9090")
            .build();

        assert!(client.is_ok());
    }

    #[test]
    fn test_client_new() {
        let client = UppClient::new("http://localhost:9090");
        assert!(client.is_ok());
    }

    #[test]
    fn test_build_url() {
        let client = UppClient::new("http://localhost:9090").unwrap();
        let url = client.build_url("/test").unwrap();
        assert_eq!(url.as_str(), "http://localhost:9090/test");
    }

    #[test]
    fn test_build_url_with_path() {
        let client = UppClient::new("http://localhost:9090").unwrap();
        let url = client.build_url("/upp/v1/markets").unwrap();
        assert_eq!(url.as_str(), "http://localhost:9090/upp/v1/markets");
    }
}
