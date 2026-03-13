// Persistent storage module — supports in-memory and Redis backends.
//
// Provides a `StorageBackend` trait for storing orders, trades, and market cache data.
// Default is in-memory (using DashMap), with optional Redis backing for multi-node setups.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ─── Data Types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredOrder {
    pub order_id: String,
    pub provider: String,
    pub market_id: String,
    pub outcome_id: String,
    pub side: String,       // "buy" or "sell"
    pub price: String,
    pub quantity: i64,
    pub status: String,     // "pending", "filled", "cancelled", "rejected"
    pub created_at: String, // ISO 8601
    pub updated_at: String,
    pub provider_order_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTrade {
    pub trade_id: String,
    pub order_id: String,
    pub provider: String,
    pub market_id: String,
    pub side: String,
    pub price: String,
    pub quantity: i64,
    pub fee: String,
    pub executed_at: String,
}

#[derive(Debug, Clone)]
pub struct OrderFilter {
    pub provider: Option<String>,
    pub market_id: Option<String>,
    pub status: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct TradeFilter {
    pub provider: Option<String>,
    pub market_id: Option<String>,
    pub order_id: Option<String>,
    pub limit: usize,
}

// ─── Storage Backend Trait ─────────────────────────────────────

#[async_trait]
pub trait StorageBackend: Send + Sync {
    // Orders
    async fn save_order(&self, order: &StoredOrder) -> Result<()>;
    async fn get_order(&self, order_id: &str) -> Result<Option<StoredOrder>>;
    async fn list_orders(&self, filter: &OrderFilter) -> Result<Vec<StoredOrder>>;
    async fn update_order_status(&self, order_id: &str, status: &str) -> Result<()>;

    // Trades
    async fn save_trade(&self, trade: &StoredTrade) -> Result<()>;
    async fn list_trades(&self, filter: &TradeFilter) -> Result<Vec<StoredTrade>>;

    // Market cache
    async fn cache_market(&self, market_id: &str, data: &str, ttl_secs: u64) -> Result<()>;
    async fn get_cached_market(&self, market_id: &str) -> Result<Option<String>>;

    // Stats
    async fn order_count(&self) -> Result<u64>;
    async fn trade_count(&self) -> Result<u64>;
}

// ─── In-Memory Storage (DashMap) ───────────────────────────────

pub struct InMemoryStorage {
    orders: DashMap<String, StoredOrder>,
    trades: DashMap<String, StoredTrade>,
    market_cache: DashMap<String, (String, Instant, Duration)>, // (data, cached_at, ttl)
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self {
            orders: DashMap::new(),
            trades: DashMap::new(),
            market_cache: DashMap::new(),
        }
    }
}

#[async_trait]
impl StorageBackend for InMemoryStorage {
    async fn save_order(&self, order: &StoredOrder) -> Result<()> {
        self.orders.insert(order.order_id.clone(), order.clone());
        Ok(())
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<StoredOrder>> {
        Ok(self.orders.get(order_id).map(|ref_multi| ref_multi.clone()))
    }

    async fn list_orders(&self, filter: &OrderFilter) -> Result<Vec<StoredOrder>> {
        let mut results: Vec<StoredOrder> = self
            .orders
            .iter()
            .map(|entry| entry.value().clone())
            .filter(|order| {
                if let Some(ref provider) = filter.provider {
                    if &order.provider != provider {
                        return false;
                    }
                }
                if let Some(ref market_id) = filter.market_id {
                    if &order.market_id != market_id {
                        return false;
                    }
                }
                if let Some(ref status) = filter.status {
                    if &order.status != status {
                        return false;
                    }
                }
                true
            })
            .collect();

        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        results.truncate(filter.limit);
        Ok(results)
    }

    async fn update_order_status(&self, order_id: &str, status: &str) -> Result<()> {
        if let Some(mut order) = self.orders.get_mut(order_id) {
            order.status = status.to_string();
            order.updated_at = chrono::Utc::now().to_rfc3339();
        }
        Ok(())
    }

    async fn save_trade(&self, trade: &StoredTrade) -> Result<()> {
        self.trades.insert(trade.trade_id.clone(), trade.clone());
        Ok(())
    }

    async fn list_trades(&self, filter: &TradeFilter) -> Result<Vec<StoredTrade>> {
        let mut results: Vec<StoredTrade> = self
            .trades
            .iter()
            .map(|entry| entry.value().clone())
            .filter(|trade| {
                if let Some(ref provider) = filter.provider {
                    if &trade.provider != provider {
                        return false;
                    }
                }
                if let Some(ref market_id) = filter.market_id {
                    if &trade.market_id != market_id {
                        return false;
                    }
                }
                if let Some(ref order_id) = filter.order_id {
                    if &trade.order_id != order_id {
                        return false;
                    }
                }
                true
            })
            .collect();

        results.sort_by(|a, b| b.executed_at.cmp(&a.executed_at));
        results.truncate(filter.limit);
        Ok(results)
    }

    async fn cache_market(&self, market_id: &str, data: &str, ttl_secs: u64) -> Result<()> {
        self.market_cache.insert(
            market_id.to_string(),
            (data.to_string(), Instant::now(), Duration::from_secs(ttl_secs)),
        );
        Ok(())
    }

    async fn get_cached_market(&self, market_id: &str) -> Result<Option<String>> {
        if let Some((data, cached_at, ttl)) = self.market_cache.get(market_id).map(|r| r.clone()) {
            if cached_at.elapsed() < ttl {
                return Ok(Some(data));
            } else {
                self.market_cache.remove(market_id);
            }
        }
        Ok(None)
    }

    async fn order_count(&self) -> Result<u64> {
        Ok(self.orders.len() as u64)
    }

    async fn trade_count(&self) -> Result<u64> {
        Ok(self.trades.len() as u64)
    }
}

// ─── Redis Storage ─────────────────────────────────────────────

pub struct RedisStorage {
    client: redis::Client,
    prefix: String,
}

impl RedisStorage {
    pub async fn new(url: &str) -> Result<Self> {
        let client = redis::Client::open(url)?;
        let mut conn = client.get_multiplexed_async_connection().await?;

        // Verify connectivity with PING
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;

        Ok(Self {
            client,
            prefix: "upp:".to_string(),
        })
    }

    async fn get_conn(&self) -> Result<redis::aio::MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow!("Redis connection failed: {}", e))
    }
}

#[async_trait]
impl StorageBackend for RedisStorage {
    async fn save_order(&self, order: &StoredOrder) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let key = format!("{}orders:{}", self.prefix, order.order_id);
        let json = serde_json::to_string(order)?;
        conn.set::<_, _, ()>(&key, json).await?;
        Ok(())
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<StoredOrder>> {
        let mut conn = self.get_conn().await?;
        let key = format!("{}orders:{}", self.prefix, order_id);
        match conn.get::<_, Option<String>>(&key).await {
            Ok(Some(json)) => Ok(Some(serde_json::from_str(&json)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow!("Redis get failed: {}", e)),
        }
    }

    async fn list_orders(&self, filter: &OrderFilter) -> Result<Vec<StoredOrder>> {
        let mut conn = self.get_conn().await?;
        let pattern = format!("{}orders:*", self.prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();

        let mut orders = Vec::new();
        for key in keys {
            if let Ok(Some(json)) = conn.get::<_, Option<String>>(&key).await {
                if let Ok(order) = serde_json::from_str::<StoredOrder>(&json) {
                    if (filter.provider.is_none() || filter.provider.as_ref() == Some(&order.provider))
                        && (filter.market_id.is_none() || filter.market_id.as_ref() == Some(&order.market_id))
                        && (filter.status.is_none() || filter.status.as_ref() == Some(&order.status))
                    {
                        orders.push(order);
                    }
                }
            }
        }

        orders.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        orders.truncate(filter.limit);
        Ok(orders)
    }

    async fn update_order_status(&self, order_id: &str, status: &str) -> Result<()> {
        if let Some(mut order) = self.get_order(order_id).await? {
            order.status = status.to_string();
            order.updated_at = chrono::Utc::now().to_rfc3339();
            self.save_order(&order).await?;
        }
        Ok(())
    }

    async fn save_trade(&self, trade: &StoredTrade) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let key = format!("{}trades:{}", self.prefix, trade.trade_id);
        let json = serde_json::to_string(trade)?;
        conn.set::<_, _, ()>(&key, json).await?;
        Ok(())
    }

    async fn list_trades(&self, filter: &TradeFilter) -> Result<Vec<StoredTrade>> {
        let mut conn = self.get_conn().await?;
        let pattern = format!("{}trades:*", self.prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();

        let mut trades = Vec::new();
        for key in keys {
            if let Ok(Some(json)) = conn.get::<_, Option<String>>(&key).await {
                if let Ok(trade) = serde_json::from_str::<StoredTrade>(&json) {
                    if (filter.provider.is_none() || filter.provider.as_ref() == Some(&trade.provider))
                        && (filter.market_id.is_none() || filter.market_id.as_ref() == Some(&trade.market_id))
                        && (filter.order_id.is_none() || filter.order_id.as_ref() == Some(&trade.order_id))
                    {
                        trades.push(trade);
                    }
                }
            }
        }

        trades.sort_by(|a, b| b.executed_at.cmp(&a.executed_at));
        trades.truncate(filter.limit);
        Ok(trades)
    }

    async fn cache_market(&self, market_id: &str, data: &str, ttl_secs: u64) -> Result<()> {
        let mut conn = self.get_conn().await?;
        let key = format!("{}market_cache:{}", self.prefix, market_id);
        conn.set_ex::<_, _, ()>(&key, data, ttl_secs).await?;
        Ok(())
    }

    async fn get_cached_market(&self, market_id: &str) -> Result<Option<String>> {
        let mut conn = self.get_conn().await?;
        let key = format!("{}market_cache:{}", self.prefix, market_id);
        match conn.get::<_, Option<String>>(&key).await {
            Ok(value) => Ok(value),
            Err(e) => Err(anyhow!("Redis get failed: {}", e)),
        }
    }

    async fn order_count(&self) -> Result<u64> {
        let mut conn = self.get_conn().await?;
        let pattern = format!("{}orders:*", self.prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();
        Ok(keys.len() as u64)
    }

    async fn trade_count(&self) -> Result<u64> {
        let mut conn = self.get_conn().await?;
        let pattern = format!("{}trades:*", self.prefix);
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&pattern)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();
        Ok(keys.len() as u64)
    }
}

// ─── Factory Function ──────────────────────────────────────────

pub async fn create_storage(redis_url: Option<&str>) -> Result<Arc<dyn StorageBackend>> {
    if let Some(url) = redis_url {
        match RedisStorage::new(url).await {
            Ok(redis) => {
                tracing::info!("Using Redis storage at {}", url);
                return Ok(Arc::new(redis));
            }
            Err(e) => {
                tracing::warn!("Redis connection failed ({}), falling back to in-memory", e);
            }
        }
    }
    tracing::info!("Using in-memory storage");
    Ok(Arc::new(InMemoryStorage::default()))
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_order(id: &str, provider: &str, market: &str, status: &str) -> StoredOrder {
        StoredOrder {
            order_id: id.to_string(),
            provider: provider.to_string(),
            market_id: market.to_string(),
            outcome_id: "yes".to_string(),
            side: "buy".to_string(),
            price: "0.65".to_string(),
            quantity: 100,
            status: status.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            provider_order_id: None,
        }
    }

    fn make_trade(id: &str, order_id: &str, provider: &str, market: &str) -> StoredTrade {
        StoredTrade {
            trade_id: id.to_string(),
            order_id: order_id.to_string(),
            provider: provider.to_string(),
            market_id: market.to_string(),
            side: "buy".to_string(),
            price: "0.65".to_string(),
            quantity: 50,
            fee: "0.01".to_string(),
            executed_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_inmemory_save_and_get_order() {
        let storage = InMemoryStorage::default();
        let order = make_order("ord-1", "kalshi", "BTC-100K", "pending");

        storage.save_order(&order).await.unwrap();
        let retrieved = storage.get_order("ord-1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().order_id, "ord-1");
    }

    #[tokio::test]
    async fn test_inmemory_get_nonexistent_order() {
        let storage = InMemoryStorage::default();
        let result = storage.get_order("nope").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_inmemory_list_orders_with_filter() {
        let storage = InMemoryStorage::default();
        storage.save_order(&make_order("ord-1", "kalshi", "BTC-100K", "pending")).await.unwrap();
        storage.save_order(&make_order("ord-2", "polymarket", "ELECTION", "filled")).await.unwrap();
        storage.save_order(&make_order("ord-3", "kalshi", "BTC-100K", "filled")).await.unwrap();

        // Filter by provider
        let filter = OrderFilter {
            provider: Some("kalshi".to_string()),
            market_id: None,
            status: None,
            limit: 100,
        };
        let results = storage.list_orders(&filter).await.unwrap();
        assert_eq!(results.len(), 2);

        // Filter by status
        let filter = OrderFilter {
            provider: None,
            market_id: None,
            status: Some("filled".to_string()),
            limit: 100,
        };
        let results = storage.list_orders(&filter).await.unwrap();
        assert_eq!(results.len(), 2);

        // Filter by market
        let filter = OrderFilter {
            provider: None,
            market_id: Some("ELECTION".to_string()),
            status: None,
            limit: 100,
        };
        let results = storage.list_orders(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_inmemory_list_orders_limit() {
        let storage = InMemoryStorage::default();
        for i in 0..10 {
            storage.save_order(&make_order(&format!("ord-{}", i), "kalshi", "BTC", "pending")).await.unwrap();
        }

        let filter = OrderFilter {
            provider: None,
            market_id: None,
            status: None,
            limit: 3,
        };
        let results = storage.list_orders(&filter).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_inmemory_update_order_status() {
        let storage = InMemoryStorage::default();
        storage.save_order(&make_order("ord-1", "kalshi", "BTC", "pending")).await.unwrap();

        storage.update_order_status("ord-1", "filled").await.unwrap();

        let order = storage.get_order("ord-1").await.unwrap().unwrap();
        assert_eq!(order.status, "filled");
    }

    #[tokio::test]
    async fn test_inmemory_save_and_list_trades() {
        let storage = InMemoryStorage::default();
        storage.save_trade(&make_trade("trd-1", "ord-1", "kalshi", "BTC")).await.unwrap();
        storage.save_trade(&make_trade("trd-2", "ord-1", "kalshi", "BTC")).await.unwrap();
        storage.save_trade(&make_trade("trd-3", "ord-2", "poly", "ELEC")).await.unwrap();

        // Filter by order
        let filter = TradeFilter {
            provider: None,
            market_id: None,
            order_id: Some("ord-1".to_string()),
            limit: 100,
        };
        let results = storage.list_trades(&filter).await.unwrap();
        assert_eq!(results.len(), 2);

        // Filter by provider
        let filter = TradeFilter {
            provider: Some("poly".to_string()),
            market_id: None,
            order_id: None,
            limit: 100,
        };
        let results = storage.list_trades(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_inmemory_market_cache() {
        let storage = InMemoryStorage::default();

        // Cache a market
        storage.cache_market("BTC-100K", r#"{"price":"0.65"}"#, 60).await.unwrap();

        // Retrieve it
        let cached = storage.get_cached_market("BTC-100K").await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), r#"{"price":"0.65"}"#);

        // Non-existent market
        let none = storage.get_cached_market("NOPE").await.unwrap();
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn test_inmemory_market_cache_ttl_expired() {
        let storage = InMemoryStorage::default();

        // Cache with 0-second TTL (expires immediately)
        storage.market_cache.insert(
            "BTC-EXPIRED".to_string(),
            (
                "data".to_string(),
                Instant::now() - Duration::from_secs(10), // 10 seconds in the past
                Duration::from_secs(1),                     // 1-second TTL
            ),
        );

        let cached = storage.get_cached_market("BTC-EXPIRED").await.unwrap();
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_inmemory_order_and_trade_counts() {
        let storage = InMemoryStorage::default();
        assert_eq!(storage.order_count().await.unwrap(), 0);
        assert_eq!(storage.trade_count().await.unwrap(), 0);

        storage.save_order(&make_order("o1", "k", "m", "pending")).await.unwrap();
        storage.save_order(&make_order("o2", "k", "m", "filled")).await.unwrap();
        storage.save_trade(&make_trade("t1", "o1", "k", "m")).await.unwrap();

        assert_eq!(storage.order_count().await.unwrap(), 2);
        assert_eq!(storage.trade_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_create_storage_defaults_to_inmemory() {
        let storage = create_storage(None).await.unwrap();
        // Should succeed and return in-memory backend
        assert_eq!(storage.order_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_create_storage_invalid_redis_falls_back() {
        // Invalid Redis URL should fall back to in-memory
        let storage = create_storage(Some("redis://invalid-host:9999")).await.unwrap();
        assert_eq!(storage.order_count().await.unwrap(), 0);
    }
}
