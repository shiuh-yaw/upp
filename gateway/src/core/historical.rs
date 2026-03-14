// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Historical Data Ingestion Pipeline — fetches historical price data from
// prediction market providers and feeds it into the PriceIndex for backtesting.
//
// Architecture:
//   - HistoricalDataSource trait: pluggable provider abstraction
//   - MockPriceGenerator: realistic price series for dev/testing
//   - IngestionPipeline: coordinates fetch → normalize → ingest flow
//   - Background task: periodic ingestion of recent data

use crate::core::price_index::{PriceIndex, PriceTick};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, info, warn};

// ─── Error Type ─────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum HistoricalError {
    #[error("Provider {0} not available")]
    ProviderNotAvailable(String),

    #[error("No data available for market {0} in requested period")]
    NoDataAvailable(String),

    #[error("API error from {provider}: {message}")]
    ApiError { provider: String, message: String },

    #[error("Data validation error: {0}")]
    ValidationError(String),

    #[error("Time range error: {0}")]
    TimeRangeError(String),
}

// ─── Data Types ─────────────────────────────────────────────

/// Raw price tick from a historical data provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalTick {
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
}

impl HistoricalTick {
    pub fn new(timestamp: DateTime<Utc>, price: f64, volume: f64) -> Self {
        Self { timestamp, price, volume, bid: None, ask: None }
    }

    pub fn with_spreads(timestamp: DateTime<Utc>, price: f64, volume: f64, bid: f64, ask: f64) -> Self {
        Self { timestamp, price, volume, bid: Some(bid), ask: Some(ask) }
    }

    fn validate(&self) -> Result<(), HistoricalError> {
        if !(0.0..=1.0).contains(&self.price) {
            return Err(HistoricalError::ValidationError(format!(
                "Price {} outside valid range [0, 1]", self.price
            )));
        }
        if self.volume < 0.0 {
            return Err(HistoricalError::ValidationError("Volume cannot be negative".into()));
        }
        if let (Some(bid), Some(ask)) = (self.bid, self.ask) {
            if bid > ask {
                return Err(HistoricalError::ValidationError("Bid price cannot exceed ask price".into()));
            }
        }
        Ok(())
    }
}

/// Metadata about available historical data for a market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalMarketInfo {
    pub native_market_id: String,
    pub description: String,
    pub earliest_data: DateTime<Utc>,
    pub latest_data: DateTime<Utc>,
    pub tick_count: u64,
}

impl HistoricalMarketInfo {
    pub fn new(id: String, description: String, earliest: DateTime<Utc>, latest: DateTime<Utc>, ticks: u64) -> Self {
        Self { native_market_id: id, description, earliest_data: earliest, latest_data: latest, tick_count: ticks }
    }
}

// ─── Data Source Trait ───────────────────────────────────────

/// Abstraction for fetching historical price data from a provider.
#[async_trait]
pub trait HistoricalDataSource: Send + Sync {
    fn provider_id(&self) -> &str;

    async fn fetch_market_history(
        &self,
        native_market_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<HistoricalTick>, HistoricalError>;

    async fn available_markets(&self) -> Result<Vec<HistoricalMarketInfo>, HistoricalError>;

    async fn data_range(&self, native_market_id: &str) -> Result<(DateTime<Utc>, DateTime<Utc>), HistoricalError> {
        let markets = self.available_markets().await?;
        markets
            .iter()
            .find(|m| m.native_market_id == native_market_id)
            .map(|m| (m.earliest_data, m.latest_data))
            .ok_or_else(|| HistoricalError::NoDataAvailable(native_market_id.to_string()))
    }
}

// ─── Mock Price Generator ───────────────────────────────────

/// Deterministic-seed PRNG for reproducible mock data without requiring `rand` crate.
/// Uses a simple xorshift64 generator seeded from timestamp + parameters.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn from_seed(seed: u64) -> Self {
        Self { state: if seed == 0 { 0xDEADBEEF } else { seed } }
    }

    /// Returns a f64 in [0, 1).
    fn next_f64(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as f64) / (u64::MAX as f64)
    }
}

/// Generates realistic prediction market price series for development and testing.
struct MockPriceGenerator {
    volatility: f64,
    mean_reversion_speed: f64,
    base_volume: f64,
}

impl MockPriceGenerator {
    fn new(volatility: f64, mean_reversion_speed: f64, base_volume: f64) -> Self {
        Self { volatility, mean_reversion_speed, base_volume }
    }

    /// Generate a price series with mean-reverting random walk.
    /// Prices stay in [0.01, 0.99] range and revert toward 0.5.
    fn generate_price_series(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        tick_interval_seconds: i64,
        starting_price: f64,
    ) -> Vec<HistoricalTick> {
        let mut ticks = Vec::new();
        let mut price = starting_price.clamp(0.01, 0.99);
        let target = 0.5;
        let mut t = start_time;
        let seed = start_time.timestamp() as u64 ^ (self.volatility * 1000.0) as u64;
        let mut rng = SimpleRng::from_seed(seed);

        while t <= end_time {
            let reversion = self.mean_reversion_speed * (target - price);
            let random_change = self.volatility
                * (2.0 * rng.next_f64() - 1.0)
                * (tick_interval_seconds as f64 / 3600.0).sqrt();

            price = (price + reversion + random_change).clamp(0.01, 0.99);

            // Volume varies by hour (higher at market open/close)
            let hour = t.hour();
            let vol_mult = match hour {
                0..=1 | 16..=17 | 23 => 1.8,
                8..=9 | 14..=15 => 1.5,
                _ => 1.0,
            };
            let volume = self.base_volume * vol_mult * (0.8 + rng.next_f64() * 0.4);

            // Bid-ask spread: 1-3 basis points
            let spread = (price * (0.0001 + rng.next_f64() * 0.0002)).max(0.0001);
            let bid = (price - spread / 2.0).max(0.0);
            let ask = (price + spread / 2.0).min(1.0);

            ticks.push(HistoricalTick::with_spreads(t, price, volume, bid, ask));
            t += Duration::seconds(tick_interval_seconds);
        }

        ticks
    }
}

// ─── Provider Implementations ───────────────────────────────

/// Kalshi historical data source (mock in dev mode).
pub struct KalshiHistorical {
    _api_endpoint: String,
    use_mock_data: bool,
}

impl KalshiHistorical {
    pub fn new(api_endpoint: String, use_mock_data: bool) -> Self {
        Self { _api_endpoint: api_endpoint, use_mock_data }
    }
    pub fn dev() -> Self {
        Self::new("https://api.kalshi.com/v2".into(), true)
    }
}

#[async_trait]
impl HistoricalDataSource for KalshiHistorical {
    fn provider_id(&self) -> &str { "kalshi.com" }

    async fn fetch_market_history(&self, _market_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<HistoricalTick>, HistoricalError> {
        if from >= to {
            return Err(HistoricalError::TimeRangeError("Start time must be before end time".into()));
        }
        if self.use_mock_data {
            let gen = MockPriceGenerator::new(0.15, 0.08, 100.0);
            let ticks = gen.generate_price_series(from, to, 60, 0.5);
            for t in &ticks { t.validate()?; }
            Ok(ticks)
        } else {
            Ok(vec![])
        }
    }

    async fn available_markets(&self) -> Result<Vec<HistoricalMarketInfo>, HistoricalError> {
        if self.use_mock_data {
            let now = Utc::now();
            Ok(vec![
                HistoricalMarketInfo::new("KALSHI-ELECTION".into(), "Election Outcome".into(), now - Duration::days(7), now, 1008),
                HistoricalMarketInfo::new("KALSHI-EARNINGS".into(), "Tech Earnings Beat".into(), now - Duration::days(7), now, 1008),
            ])
        } else {
            Ok(vec![])
        }
    }
}

/// Polymarket historical data source (mock in dev mode).
pub struct PolymarketHistorical {
    _subgraph_url: String,
    use_mock_data: bool,
}

impl PolymarketHistorical {
    pub fn new(subgraph_url: String, use_mock_data: bool) -> Self {
        Self { _subgraph_url: subgraph_url, use_mock_data }
    }
    pub fn dev() -> Self {
        Self::new("https://api.thegraph.com/subgraphs/name/polymarket/subgraph".into(), true)
    }
}

#[async_trait]
impl HistoricalDataSource for PolymarketHistorical {
    fn provider_id(&self) -> &str { "polymarket.com" }

    async fn fetch_market_history(&self, _market_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<HistoricalTick>, HistoricalError> {
        if from >= to {
            return Err(HistoricalError::TimeRangeError("Start time must be before end time".into()));
        }
        if self.use_mock_data {
            let gen = MockPriceGenerator::new(0.18, 0.06, 150.0);
            let ticks = gen.generate_price_series(from, to, 120, 0.52);
            for t in &ticks { t.validate()?; }
            Ok(ticks)
        } else {
            Ok(vec![])
        }
    }

    async fn available_markets(&self) -> Result<Vec<HistoricalMarketInfo>, HistoricalError> {
        if self.use_mock_data {
            let now = Utc::now();
            Ok(vec![
                HistoricalMarketInfo::new("POLY-BTC-EOY".into(), "BTC Price End of Year".into(), now - Duration::days(14), now, 2016),
                HistoricalMarketInfo::new("POLY-SUPERBOWL".into(), "Super Bowl Winner".into(), now - Duration::days(14), now, 2016),
            ])
        } else {
            Ok(vec![])
        }
    }
}

/// Opinion.trade historical data source (mock in dev mode).
pub struct OpinionHistorical {
    _api_key: Option<String>,
    use_mock_data: bool,
}

impl OpinionHistorical {
    pub fn new(api_key: Option<String>, use_mock_data: bool) -> Self {
        Self { _api_key: api_key, use_mock_data }
    }
    pub fn dev() -> Self {
        Self::new(None, true)
    }
}

#[async_trait]
impl HistoricalDataSource for OpinionHistorical {
    fn provider_id(&self) -> &str { "opinion.trade" }

    async fn fetch_market_history(&self, _market_id: &str, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<HistoricalTick>, HistoricalError> {
        if from >= to {
            return Err(HistoricalError::TimeRangeError("Start time must be before end time".into()));
        }
        if self.use_mock_data {
            let gen = MockPriceGenerator::new(0.12, 0.10, 75.0);
            let ticks = gen.generate_price_series(from, to, 300, 0.55);
            for t in &ticks { t.validate()?; }
            Ok(ticks)
        } else {
            Ok(vec![])
        }
    }

    async fn available_markets(&self) -> Result<Vec<HistoricalMarketInfo>, HistoricalError> {
        if self.use_mock_data {
            let now = Utc::now();
            Ok(vec![
                HistoricalMarketInfo::new("OPINION-RATE".into(), "Federal Policy Rate".into(), now - Duration::days(30), now, 7200),
                HistoricalMarketInfo::new("OPINION-CPI".into(), "Next CPI Report".into(), now - Duration::days(30), now, 7200),
            ])
        } else {
            Ok(vec![])
        }
    }
}

// ─── Ingestion Pipeline ─────────────────────────────────────

/// Statistics for ingestion operations.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IngestionStats {
    pub ticks_ingested: u64,
    pub errors_encountered: u64,
    pub markets_processed: u64,
}

/// Orchestrates fetching historical data and feeding it into the PriceIndex.
pub struct IngestionPipeline {
    price_index: Arc<PriceIndex>,
    sources: Vec<Box<dyn HistoricalDataSource>>,
    ticks_ingested: AtomicU64,
    errors_encountered: AtomicU64,
    markets_processed: AtomicU64,
}

impl IngestionPipeline {
    pub fn new(price_index: Arc<PriceIndex>, sources: Vec<Box<dyn HistoricalDataSource>>) -> Self {
        Self {
            price_index,
            sources,
            ticks_ingested: AtomicU64::new(0),
            errors_encountered: AtomicU64::new(0),
            markets_processed: AtomicU64::new(0),
        }
    }

    /// Ingest historical data for a specific market into the PriceIndex.
    pub async fn ingest_market(
        &self,
        provider_id: &str,
        market_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<u64, HistoricalError> {
        let source = self.sources.iter()
            .find(|s| s.provider_id() == provider_id)
            .ok_or_else(|| HistoricalError::ProviderNotAvailable(provider_id.into()))?;

        debug!(provider = %provider_id, market = %market_id, "Fetching historical data");

        let ticks = source.fetch_market_history(market_id, from, to).await.map_err(|e| {
            self.errors_encountered.fetch_add(1, Ordering::Relaxed);
            warn!(provider = %provider_id, error = %e, "Error fetching history");
            e
        })?;

        let tick_count = ticks.len() as u64;
        let upp_market_id = format!("upp:{}:{}", provider_id, market_id);

        for tick in ticks {
            // Convert HistoricalTick → PriceTick (PriceIndex format)
            self.price_index.ingest(PriceTick {
                market_id: upp_market_id.clone(),
                outcome_id: "yes".to_string(),
                price: tick.price,
                timestamp: tick.timestamp,
            });
        }

        self.ticks_ingested.fetch_add(tick_count, Ordering::Relaxed);
        self.markets_processed.fetch_add(1, Ordering::Relaxed);
        info!(provider = %provider_id, market = %market_id, ticks = tick_count, "Historical data ingested");

        Ok(tick_count)
    }

    /// Bulk-ingest recent data for all known markets across all providers.
    pub async fn ingest_all_recent(&self, hours_back: u64) -> Result<u64, HistoricalError> {
        let to = Utc::now();
        let from = to - Duration::hours(hours_back as i64);
        let mut total = 0u64;

        for source in &self.sources {
            let pid = source.provider_id().to_string();
            match source.available_markets().await {
                Ok(markets) => {
                    for market in markets {
                        match self.ingest_market(&pid, &market.native_market_id, from, to).await {
                            Ok(n) => total += n,
                            Err(e) => warn!(provider = %pid, market = %market.native_market_id, error = %e, "Market ingestion failed"),
                        }
                    }
                }
                Err(e) => warn!(provider = %pid, error = %e, "Failed to list markets"),
            }
        }

        info!(total_ticks = total, hours_back, "Bulk ingestion complete");
        Ok(total)
    }

    /// Get current ingestion statistics.
    pub fn stats(&self) -> IngestionStats {
        IngestionStats {
            ticks_ingested: self.ticks_ingested.load(Ordering::Relaxed),
            errors_encountered: self.errors_encountered.load(Ordering::Relaxed),
            markets_processed: self.markets_processed.load(Ordering::Relaxed),
        }
    }
}

/// Start a background task that periodically ingests recent data.
pub fn start_ingestion_pipeline(
    pipeline: Arc<IngestionPipeline>,
    interval_minutes: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_minutes * 60));
        loop {
            interval.tick().await;
            debug!("Background ingestion starting");
            match pipeline.ingest_all_recent(1).await {
                Ok(n) => info!(ticks = n, "Background ingestion complete"),
                Err(e) => warn!(error = %e, "Background ingestion error"),
            }
        }
    })
}

/// Create a default pipeline with mock data sources for development.
pub fn create_dev_pipeline(price_index: Arc<PriceIndex>) -> Arc<IngestionPipeline> {
    let sources: Vec<Box<dyn HistoricalDataSource>> = vec![
        Box::new(KalshiHistorical::dev()),
        Box::new(PolymarketHistorical::dev()),
        Box::new(OpinionHistorical::dev()),
    ];
    Arc::new(IngestionPipeline::new(price_index, sources))
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_validation_valid() {
        let tick = HistoricalTick::new(Utc::now(), 0.5, 100.0);
        assert!(tick.validate().is_ok());
    }

    #[test]
    fn test_tick_validation_price_out_of_range() {
        assert!(HistoricalTick::new(Utc::now(), 1.5, 100.0).validate().is_err());
        assert!(HistoricalTick::new(Utc::now(), -0.1, 100.0).validate().is_err());
    }

    #[test]
    fn test_tick_validation_negative_volume() {
        assert!(HistoricalTick::new(Utc::now(), 0.5, -10.0).validate().is_err());
    }

    #[test]
    fn test_tick_validation_inverted_spread() {
        let tick = HistoricalTick::with_spreads(Utc::now(), 0.5, 100.0, 0.6, 0.4);
        assert!(tick.validate().is_err());
    }

    #[test]
    fn test_tick_validation_valid_spread() {
        let tick = HistoricalTick::with_spreads(Utc::now(), 0.5, 100.0, 0.49, 0.51);
        assert!(tick.validate().is_ok());
        assert_eq!(tick.bid, Some(0.49));
        assert_eq!(tick.ask, Some(0.51));
    }

    #[test]
    fn test_mock_price_bounds() {
        let gen = MockPriceGenerator::new(0.15, 0.08, 100.0);
        let start = Utc::now();
        let ticks = gen.generate_price_series(start, start + Duration::hours(24), 60, 0.5);

        assert!(!ticks.is_empty());
        for tick in &ticks {
            assert!((0.0..=1.0).contains(&tick.price), "Price {} out of bounds", tick.price);
            assert!(tick.volume >= 0.0);
        }
    }

    #[test]
    fn test_mock_price_mean_reversion() {
        let gen = MockPriceGenerator::new(0.05, 0.15, 100.0);
        let start = Utc::now();
        let ticks = gen.generate_price_series(start, start + Duration::hours(48), 60, 0.9);

        let avg: f64 = ticks.iter().map(|t| t.price).sum::<f64>() / ticks.len() as f64;
        assert!(avg > 0.3 && avg < 0.75, "Mean should revert toward 0.5, got {}", avg);
    }

    #[test]
    fn test_mock_monotonic_timestamps() {
        let gen = MockPriceGenerator::new(0.15, 0.08, 100.0);
        let start = Utc::now();
        let ticks = gen.generate_price_series(start, start + Duration::hours(12), 300, 0.5);

        for i in 1..ticks.len() {
            assert!(ticks[i].timestamp > ticks[i - 1].timestamp);
        }
    }

    #[test]
    fn test_mock_tick_interval() {
        let gen = MockPriceGenerator::new(0.15, 0.08, 100.0);
        let start = Utc::now();
        let ticks = gen.generate_price_series(start, start + Duration::hours(1), 120, 0.5);

        // ~30 ticks for 1 hour at 2-minute intervals
        assert!(ticks.len() > 25 && ticks.len() < 35);
        for i in 1..ticks.len() {
            let diff = ticks[i].timestamp.signed_duration_since(ticks[i - 1].timestamp);
            assert_eq!(diff.num_seconds(), 120);
        }
    }

    #[test]
    fn test_market_info_creation() {
        let now = Utc::now();
        let info = HistoricalMarketInfo::new("M1".into(), "Test".into(), now - Duration::days(7), now, 1000);
        assert_eq!(info.native_market_id, "M1");
        assert_eq!(info.tick_count, 1000);
    }

    #[tokio::test]
    async fn test_kalshi_dev_mode() {
        let source = KalshiHistorical::dev();
        assert_eq!(source.provider_id(), "kalshi.com");

        let markets = source.available_markets().await.unwrap();
        assert!(!markets.is_empty());

        let now = Utc::now();
        let ticks = source.fetch_market_history(&markets[0].native_market_id, now - Duration::hours(1), now).await.unwrap();
        assert!(!ticks.is_empty());
        for t in &ticks { assert!(t.validate().is_ok()); }
    }

    #[tokio::test]
    async fn test_polymarket_dev_mode() {
        let source = PolymarketHistorical::dev();
        assert_eq!(source.provider_id(), "polymarket.com");

        let markets = source.available_markets().await.unwrap();
        assert!(!markets.is_empty());
    }

    #[tokio::test]
    async fn test_opinion_dev_mode() {
        let source = OpinionHistorical::dev();
        assert_eq!(source.provider_id(), "opinion.trade");

        let markets = source.available_markets().await.unwrap();
        assert!(!markets.is_empty());
    }

    #[tokio::test]
    async fn test_time_range_validation() {
        let source = KalshiHistorical::dev();
        let now = Utc::now();
        let result = source.fetch_market_history("TEST", now, now - Duration::hours(1)).await;
        assert!(matches!(result, Err(HistoricalError::TimeRangeError(_))));
    }

    #[tokio::test]
    async fn test_pipeline_ingest_market() {
        let index = Arc::new(PriceIndex::new());
        let sources: Vec<Box<dyn HistoricalDataSource>> = vec![Box::new(KalshiHistorical::dev())];
        let pipeline = IngestionPipeline::new(index.clone(), sources);

        let now = Utc::now();
        let count = pipeline.ingest_market("kalshi.com", "KALSHI-ELECTION", now - Duration::hours(1), now).await.unwrap();
        assert!(count > 0);

        let stats = pipeline.stats();
        assert!(stats.ticks_ingested > 0);
        assert_eq!(stats.markets_processed, 1);
        assert_eq!(stats.errors_encountered, 0);

        // Verify data landed in price index
        let idx_stats = index.stats();
        assert!(idx_stats.ticks_ingested > 0);
    }

    #[tokio::test]
    async fn test_pipeline_ingest_all_recent() {
        let index = Arc::new(PriceIndex::new());
        let pipeline = create_dev_pipeline(index.clone());

        let count = pipeline.ingest_all_recent(1).await.unwrap();
        assert!(count > 0, "Should ingest ticks from all providers");

        let stats = pipeline.stats();
        assert!(stats.markets_processed >= 6, "Should process markets from all 3 providers");
    }

    #[tokio::test]
    async fn test_pipeline_unknown_provider() {
        let index = Arc::new(PriceIndex::new());
        let pipeline = IngestionPipeline::new(index, vec![]);
        let now = Utc::now();
        let result = pipeline.ingest_market("unknown.com", "M1", now - Duration::hours(1), now).await;
        assert!(matches!(result, Err(HistoricalError::ProviderNotAvailable(_))));
    }

    #[test]
    fn test_ingestion_stats_default() {
        let stats = IngestionStats::default();
        assert_eq!(stats.ticks_ingested, 0);
        assert_eq!(stats.errors_encountered, 0);
        assert_eq!(stats.markets_processed, 0);
    }
}
