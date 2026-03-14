# Provider Adapters

Adapters are the bridge between UPP's unified protocol and provider-specific APIs. This page explains how they work and how to add support for new prediction market exchanges.

## Adapter Pattern

All adapters implement a common trait:

```rust
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Get markets, optionally filtered
    async fn get_markets(
        &self,
        filter: MarketFilter,
    ) -> Result<Vec<Market>, ProviderError>;

    /// Get a specific market by ID
    async fn get_market(
        &self,
        market_id: &str,
    ) -> Result<Market, ProviderError>;

    /// Search markets by title/description
    async fn search_markets(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Market>, ProviderError>;

    /// Get user's orders
    async fn get_orders(
        &self,
        user_id: &str,
    ) -> Result<Vec<Order>, ProviderError>;

    /// Get a specific order
    async fn get_order(
        &self,
        user_id: &str,
        order_id: &str,
    ) -> Result<Order, ProviderError>;

    /// Place a new order
    async fn place_order(
        &self,
        order: OrderRequest,
    ) -> Result<OrderResponse, ProviderError>;

    /// Cancel an order
    async fn cancel_order(
        &self,
        order_id: &str,
    ) -> Result<(), ProviderError>;

    /// Get user's portfolio/positions
    async fn get_portfolio(
        &self,
        user_id: &str,
    ) -> Result<Portfolio, ProviderError>;

    /// Subscribe to market updates
    async fn subscribe_markets(
        &self,
        markets: Vec<&str>,
    ) -> Result<Subscription, ProviderError>;

    /// Health check
    async fn health_check(&self) -> Result<(), ProviderError>;
}
```

The trait ensures all providers expose the same interface. Adapters handle provider-specific authentication, API translation, and error mapping.

## Adapter Structure

Each adapter is a module with consistent organization:

```
gateway/src/adapters/
├── mod.rs                  // Adapter registry
├── base.rs                 // Trait definition
├── polymarket/
│   ├── mod.rs             // Polymarket adapter impl
│   ├── auth.rs            // ECDSA signing
│   ├── models.rs          // API response types
│   └── errors.rs          // Polymarket-specific errors
├── kalshi/
│   ├── mod.rs             // Kalshi adapter impl
│   ├── auth.rs            // API key handling
│   ├── models.rs
│   └── errors.rs
└── opinion/
    ├── mod.rs
    ├── auth.rs
    ├── models.rs
    └── errors.rs
```

## Example: Polymarket Adapter

Here's a simplified Polymarket adapter implementation:

```rust
// gateway/src/adapters/polymarket/mod.rs

use async_trait::async_trait;
use reqwest::Client;
use sha3::{Digest, Keccak256};

pub struct PolymarketAdapter {
    client: Client,
    private_key: String,  // ECDSA private key (hex)
    base_url: String,     // https://clob.polymarket.com
}

#[async_trait]
impl ProviderAdapter for PolymarketAdapter {
    async fn get_markets(
        &self,
        filter: MarketFilter,
    ) -> Result<Vec<Market>, ProviderError> {
        // Build Polymarket API query
        let url = format!("{}markets", self.base_url);
        let params = self.build_market_params(&filter);

        // Add authentication
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let signature = self.sign_request(&format!("{}{:?}", url, params), timestamp)?;

        // Make request
        let response = self.client
            .get(&url)
            .query(&params)
            .header("POLY-SIGN", signature)
            .header("POLY-NONCE", timestamp)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::from_status(status));
        }

        // Parse response
        let poly_markets: Vec<PolymarketMarket> = response
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        // Translate to UPP format
        let markets = poly_markets
            .into_iter()
            .map(|m| self.translate_market(m))
            .collect();

        Ok(markets)
    }

    async fn place_order(
        &self,
        order: OrderRequest,
    ) -> Result<OrderResponse, ProviderError> {
        // Validate order
        if order.quantity <= 0.0 {
            return Err(ProviderError::InvalidInput("quantity must be > 0".into()));
        }

        // Translate from UPP to Polymarket format
        let poly_order = self.translate_order_request(&order)?;

        // Sign order (Polymarket uses EIP-712)
        let signature = self.sign_order(&poly_order)?;

        // Submit order
        let url = format!("{}orders", self.base_url);
        let response = self.client
            .post(&url)
            .header("POLY-SIGN", signature)
            .json(&poly_order)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let poly_response: PolymarketOrderResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        Ok(self.translate_order_response(poly_response))
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        let response = self.client
            .get(&format!("{}health", self.base_url))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::InternalServerError)
        }
    }
}

impl PolymarketAdapter {
    fn sign_request(
        &self,
        message: &str,
        timestamp: u64,
    ) -> Result<String, ProviderError> {
        // ECDSA sign using private key
        let hash = Keccak256::digest(message.as_bytes());
        let signature = sign_ecdsa(&self.private_key, &hash)?;
        Ok(hex::encode(signature))
    }

    fn translate_market(&self, poly: PolymarketMarket) -> Market {
        Market {
            id: poly.market_id,
            provider: "polymarket".to_string(),
            title: poly.question,
            description: poly.description,
            outcomes: poly.outcomes
                .into_iter()
                .map(|o| Outcome {
                    id: o.id,
                    name: o.title,
                    price: o.price,
                })
                .collect(),
            liquidity: poly.liquidity,
            volume_24h: poly.volume_24h,
            created_at: poly.created_at,
            expires_at: poly.expiry_date,
        }
    }

    fn translate_order_request(
        &self,
        req: &OrderRequest,
    ) -> Result<PolymarketOrderRequest, ProviderError> {
        Ok(PolymarketOrderRequest {
            market_id: req.market_id.clone(),
            side: req.side.as_polymarket_string().to_string(),
            price: req.price,
            size: req.quantity,
        })
    }

    fn translate_order_response(
        &self,
        poly: PolymarketOrderResponse,
    ) -> OrderResponse {
        OrderResponse {
            order_id: poly.order_id,
            status: "accepted".to_string(),
            filled: poly.amount_filled,
            remaining: poly.size - poly.amount_filled,
        }
    }
}
```

## Error Handling

Each adapter maps provider-specific errors to UPP's unified error type:

```rust
pub enum ProviderError {
    RateLimited(Duration),
    InvalidCredentials,
    InvalidInput(String),
    NotFound(String),
    NetworkError(String),
    ParseError(String),
    InternalServerError,
    Timeout,
}

impl PolymarketAdapter {
    fn map_http_error(&self, status: StatusCode) -> ProviderError {
        match status {
            StatusCode::TOO_MANY_REQUESTS => {
                ProviderError::RateLimited(Duration::from_secs(60))
            }
            StatusCode::UNAUTHORIZED => ProviderError::InvalidCredentials,
            StatusCode::NOT_FOUND => ProviderError::NotFound("Market not found".into()),
            StatusCode::BAD_REQUEST => {
                ProviderError::InvalidInput("Invalid request".into())
            }
            StatusCode::INTERNAL_SERVER_ERROR => ProviderError::InternalServerError,
            _ => ProviderError::NetworkError(format!("HTTP {}", status)),
        }
    }
}
```

## Adding a New Provider

To add support for a new exchange (e.g., Origin Protocol):

### Step 1: Create Adapter Module

```bash
mkdir -p gateway/src/adapters/origin
touch gateway/src/adapters/origin/{mod.rs,auth.rs,models.rs,errors.rs}
```

### Step 2: Define Models

In `origin/models.rs`, define API response types:

```rust
#[derive(Deserialize)]
pub struct OriginMarket {
    pub id: String,
    pub name: String,
    pub odds: HashMap<String, f64>,
    pub liquidity: f64,
}

#[derive(Serialize)]
pub struct OriginOrderRequest {
    pub market_id: String,
    pub outcome: String,
    pub amount: f64,
}
```

### Step 3: Implement ProviderAdapter Trait

In `origin/mod.rs`:

```rust
use async_trait::async_trait;
use crate::adapters::base::ProviderAdapter;

pub struct OriginAdapter {
    client: Client,
    api_key: String,
    base_url: String,
}

#[async_trait]
impl ProviderAdapter for OriginAdapter {
    async fn get_markets(
        &self,
        filter: MarketFilter,
    ) -> Result<Vec<Market>, ProviderError> {
        let response = self.client
            .get(&format!("{}markets", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .query(&[("limit", filter.limit.to_string())])
            .send()
            .await?;

        let origin_markets: Vec<OriginMarket> = response.json().await?;

        Ok(origin_markets
            .into_iter()
            .map(|m| self.translate_market(m))
            .collect())
    }

    // ... implement other trait methods
}
```

### Step 4: Register Adapter

In `gateway/src/adapters/mod.rs`:

```rust
pub fn create_adapters(config: &Config) -> AdapterRegistry {
    let mut adapters = HashMap::new();

    // Register Polymarket
    adapters.insert(
        "polymarket".to_string(),
        Arc::new(PolymarketAdapter::new(config)) as Arc<dyn ProviderAdapter>,
    );

    // Register Kalshi
    adapters.insert(
        "kalshi".to_string(),
        Arc::new(KalshiAdapter::new(config)),
    );

    // Register Opinion
    adapters.insert(
        "opinion_trade".to_string(),
        Arc::new(OpinionAdapter::new(config)),
    );

    // NEW: Register Origin
    adapters.insert(
        "origin".to_string(),
        Arc::new(OriginAdapter::new(config)),
    );

    AdapterRegistry { adapters }
}
```

### Step 5: Add Configuration

In `config/default.toml`:

```toml
[origin]
base_url = "https://api.origin.example.com/"
api_key = "${ORIGIN_API_KEY}"  # Read from env
timeout_seconds = 30
```

### Step 6: Write Tests

Create `gateway/tests/adapters/origin_adapter_test.rs`:

```rust
#[tokio::test]
async fn test_origin_get_markets() {
    let adapter = OriginAdapter::new_mock();
    let markets = adapter.get_markets(MarketFilter::default()).await.unwrap();
    assert!(!markets.is_empty());
}

#[tokio::test]
async fn test_origin_place_order() {
    let adapter = OriginAdapter::new_mock();
    let order = OrderRequest {
        market_id: "test".into(),
        side: Side::Buy,
        price: 0.5,
        quantity: 10.0,
    };
    let response = adapter.place_order(order).await.unwrap();
    assert!(!response.order_id.is_empty());
}
```

## Provider-Specific Features

Some providers have unique features worth documenting:

### Polymarket
- Uses ECDSA signing (EIP-712)
- WebSocket feed at `wss://ws-clob.polymarket.com`
- Requires private key (not API key)
- Supports order cancellation

### Kalshi
- RESTful API, no WebSocket
- Basic auth with API key + secret
- US-regulated, binary outcomes only
- Higher rate limit (100 req/sec)

### Opinion.trade
- Simple REST API
- Bearer token auth
- Lower volume but growing
- Longer polling intervals (30sec vs 5sec)

## Testing Adapters

Use mocks for testing without external API calls:

```rust
#[cfg(test)]
mod tests {
    use mockito::mock;

    #[tokio::test]
    async fn test_get_markets_with_mock() {
        let _m = mock("GET", mockito::Matcher::Any)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":"1","name":"Test Market"}]"#)
            .create();

        let adapter = PolymarketAdapter::new(Config::test());
        let markets = adapter.get_markets(MarketFilter::default()).await.unwrap();
        assert_eq!(markets.len(), 1);
    }
}
```

## Performance Considerations

- **Connection pooling**: Reuse HTTP client across requests
- **Caching**: Adapter-local cache for rapid repeated queries
- **Batch operations**: Where possible, combine multiple queries
- **Pagination**: Handle large market lists incrementally
- **Rate limiting**: Respect provider limits; use exponential backoff for retries

## Monitoring Adapters

Each adapter exports metrics:

```
upp_adapter_requests_total{provider="polymarket",method="get_markets"} 1250
upp_adapter_latency_ms{provider="polymarket",method="get_markets"} 45
upp_adapter_errors_total{provider="polymarket",error="rate_limited"} 2
upp_adapter_cache_hit_ratio{provider="polymarket"} 0.82
```

Monitor these to detect issues early and optimize performance.
