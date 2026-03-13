// Two-tier cache: in-process (Moka) for hot data, Redis for shared state.

use super::config::GatewayConfig;
use crate::core::types::Market;
use moka::future::Cache;
use std::time::Duration;

pub struct MarketCache {
    /// Hot cache for market metadata (changes rarely)
    market_cache: Cache<String, Market>,
    /// Config
    _config: GatewayConfig,
}

impl MarketCache {
    pub fn new(config: &GatewayConfig) -> Self {
        let market_cache = Cache::builder()
            .max_capacity(100_000) // 100K markets
            .time_to_live(Duration::from_secs(config.market_cache_ttl_seconds))
            .build();

        Self {
            market_cache,
            _config: config.clone(),
        }
    }

    pub async fn get_market(&self, id: &str) -> Option<Market> {
        self.market_cache.get(id).await
    }

    pub async fn put_market(&self, id: String, market: Market) {
        self.market_cache.insert(id, market).await;
    }

    pub async fn invalidate_market(&self, id: &str) {
        self.market_cache.invalidate(id).await;
    }
}
