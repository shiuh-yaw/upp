// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Historical Price Indexer — time-series storage for market prices
// with candlestick aggregation (1m, 5m, 1h, 1d) and query API.
// Uses the WebSocket price feed as input, stores in memory with
// configurable retention per resolution.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{info, debug};

// ─── Types ──────────────────────────────────────────────────

/// A single price tick ingested from the WebSocket feed.
#[derive(Debug, Clone)]
pub struct PriceTick {
    pub market_id: String,
    pub outcome_id: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
}

/// OHLCV candlestick.
#[derive(Debug, Clone, Serialize)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,        // tick count in the period
    pub timestamp: i64,     // period start as unix epoch seconds
    pub period_seconds: u64,
}

impl Candle {
    fn new(price: f64, ts: i64, period: u64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 1,
            timestamp: ts,
            period_seconds: period,
        }
    }

    fn update(&mut self, price: f64) {
        if price > self.high { self.high = price; }
        if price < self.low { self.low = price; }
        self.close = price;
        self.volume += 1;
    }
}

/// Supported candlestick resolutions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Resolution {
    #[serde(rename = "1m")]
    OneMinute,
    #[serde(rename = "5m")]
    FiveMinute,
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "1d")]
    OneDay,
}

impl Resolution {
    pub fn seconds(&self) -> u64 {
        match self {
            Resolution::OneMinute => 60,
            Resolution::FiveMinute => 300,
            Resolution::OneHour => 3600,
            Resolution::OneDay => 86400,
        }
    }

    /// Align a timestamp to the start of its period.
    pub fn align(&self, ts: DateTime<Utc>) -> i64 {
        let epoch = ts.timestamp();
        match self {
            Resolution::OneMinute => epoch - (epoch % 60),
            Resolution::FiveMinute => epoch - (epoch % 300),
            Resolution::OneHour => epoch - (epoch % 3600),
            Resolution::OneDay => {
                // Align to midnight UTC
                ts.date_naive().and_hms_opt(0, 0, 0)
                    .map(|dt| dt.and_utc().timestamp())
                    .unwrap_or(epoch - (epoch % 86400))
            }
        }
    }

    pub fn max_candles(&self) -> usize {
        match self {
            Resolution::OneMinute => 1440,   // 24 hours
            Resolution::FiveMinute => 2016,  // 7 days
            Resolution::OneHour => 720,      // 30 days
            Resolution::OneDay => 365,       // 1 year
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "1m" | "1min" => Some(Resolution::OneMinute),
            "5m" | "5min" => Some(Resolution::FiveMinute),
            "1h" | "1hr" | "1hour" => Some(Resolution::OneHour),
            "1d" | "1day" => Some(Resolution::OneDay),
            _ => None,
        }
    }

    pub fn all() -> &'static [Resolution] {
        &[Resolution::OneMinute, Resolution::FiveMinute, Resolution::OneHour, Resolution::OneDay]
    }
}

/// Key for per-market, per-outcome, per-resolution candle series.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SeriesKey {
    market_id: String,
    outcome_id: String,
    resolution: Resolution,
}

/// Candle series — ring buffer of candles for one (market, outcome, resolution).
struct CandleSeries {
    candles: VecDeque<Candle>,
    max_len: usize,
}

impl CandleSeries {
    fn new(max_len: usize) -> Self {
        Self {
            candles: VecDeque::with_capacity(max_len),
            max_len,
        }
    }

    fn ingest(&mut self, price: f64, aligned_ts: i64, period: u64) {
        if let Some(last) = self.candles.back_mut() {
            if last.timestamp == aligned_ts {
                last.update(price);
                return;
            }
        }
        // New candle period
        if self.candles.len() >= self.max_len {
            self.candles.pop_front();
        }
        self.candles.push_back(Candle::new(price, aligned_ts, period));
    }

    fn query(&self, from: Option<i64>, to: Option<i64>, limit: usize) -> Vec<Candle> {
        self.candles.iter()
            .filter(|c| {
                from.map_or(true, |f| c.timestamp >= f)
                    && to.map_or(true, |t| c.timestamp <= t)
            })
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    fn latest(&self) -> Option<&Candle> {
        self.candles.back()
    }
}

// ─── Price Index ────────────────────────────────────────────

/// The main price indexer. Thread-safe, lock-free reads via DashMap.
pub struct PriceIndex {
    series: DashMap<SeriesKey, CandleSeries>,
    ticks_ingested: AtomicU64,
    markets_tracked: DashMap<String, ()>,
}

impl PriceIndex {
    pub fn new() -> Self {
        Self {
            series: DashMap::new(),
            ticks_ingested: AtomicU64::new(0),
            markets_tracked: DashMap::new(),
        }
    }

    /// Ingest a price tick — updates all resolution candles atomically.
    pub fn ingest(&self, tick: PriceTick) {
        self.ticks_ingested.fetch_add(1, Ordering::Relaxed);
        self.markets_tracked.entry(tick.market_id.clone()).or_insert(());

        for &resolution in Resolution::all() {
            let key = SeriesKey {
                market_id: tick.market_id.clone(),
                outcome_id: tick.outcome_id.clone(),
                resolution,
            };

            let aligned = resolution.align(tick.timestamp);
            let period = resolution.seconds();

            self.series
                .entry(key)
                .or_insert_with(|| CandleSeries::new(resolution.max_candles()))
                .ingest(tick.price, aligned, period);
        }
    }

    /// Query candles for a market+outcome at a given resolution.
    pub fn query_candles(
        &self,
        market_id: &str,
        outcome_id: &str,
        resolution: Resolution,
        from: Option<i64>,
        to: Option<i64>,
        limit: usize,
    ) -> Vec<Candle> {
        let key = SeriesKey {
            market_id: market_id.to_string(),
            outcome_id: outcome_id.to_string(),
            resolution,
        };

        self.series.get(&key)
            .map(|s| s.query(from, to, limit))
            .unwrap_or_default()
    }

    /// Get the latest candle (current period) for a market+outcome.
    pub fn latest_candle(
        &self,
        market_id: &str,
        outcome_id: &str,
        resolution: Resolution,
    ) -> Option<Candle> {
        let key = SeriesKey {
            market_id: market_id.to_string(),
            outcome_id: outcome_id.to_string(),
            resolution,
        };

        self.series.get(&key)
            .and_then(|s| s.latest().cloned())
    }

    /// Get stats about the indexer.
    pub fn stats(&self) -> PriceIndexStats {
        PriceIndexStats {
            ticks_ingested: self.ticks_ingested.load(Ordering::Relaxed),
            markets_tracked: self.markets_tracked.len(),
            total_series: self.series.len(),
        }
    }

    /// List all tracked market IDs.
    pub fn tracked_markets(&self) -> Vec<String> {
        self.markets_tracked.iter().map(|e| e.key().clone()).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PriceIndexStats {
    pub ticks_ingested: u64,
    pub markets_tracked: usize,
    pub total_series: usize,
}

// ─── Background Ingestion ───────────────────────────────────

/// Start a background task that subscribes to the WebSocket price fan-out
/// and ingests ticks into the price index.
pub fn start_price_indexer(
    price_index: Arc<PriceIndex>,
    ws_manager: Arc<crate::transport::websocket::WebSocketManager>,
    poll_interval_ms: u64,
) {
    tokio::spawn(async move {
        info!("Price indexer started (poll interval: {}ms)", poll_interval_ms);
        let mut interval = tokio::time::interval(
            tokio::time::Duration::from_millis(poll_interval_ms),
        );

        loop {
            interval.tick().await;

            // Get latest prices from the WebSocket manager's cache
            let snapshot = ws_manager.get_price_snapshot().await;
            let now = Utc::now();

            for (market_id, prices) in snapshot {
                for (outcome_id, price_str) in prices {
                    if let Ok(price) = price_str.parse::<f64>() {
                        price_index.ingest(PriceTick {
                            market_id: market_id.clone(),
                            outcome_id,
                            price,
                            timestamp: now,
                        });
                    }
                }
            }

            let stats = price_index.stats();
            debug!(
                ticks = stats.ticks_ingested,
                markets = stats.markets_tracked,
                "Price indexer cycle"
            );
        }
    });
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn ts(epoch: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(epoch, 0).unwrap()
    }

    #[test]
    fn test_resolution_alignment() {
        // 2026-01-15 10:37:42 UTC
        let dt = ts(1768560000 + 37 * 60 + 42);

        let aligned_1m = Resolution::OneMinute.align(dt);
        assert_eq!(aligned_1m % 60, 0);

        let aligned_5m = Resolution::FiveMinute.align(dt);
        assert_eq!(aligned_5m % 300, 0);

        let aligned_1h = Resolution::OneHour.align(dt);
        assert_eq!(aligned_1h % 3600, 0);
    }

    #[test]
    fn test_candle_creation_and_update() {
        let mut candle = Candle::new(0.50, 1000, 60);
        assert_eq!(candle.open, 0.50);
        assert_eq!(candle.volume, 1);

        candle.update(0.55);
        assert_eq!(candle.high, 0.55);
        assert_eq!(candle.close, 0.55);
        assert_eq!(candle.volume, 2);

        candle.update(0.45);
        assert_eq!(candle.low, 0.45);
        assert_eq!(candle.close, 0.45);
        assert_eq!(candle.volume, 3);

        // open should remain unchanged
        assert_eq!(candle.open, 0.50);
    }

    #[test]
    fn test_ingest_same_period_aggregates() {
        let index = PriceIndex::new();
        let base = ts(1768560000); // aligned to 1m

        index.ingest(PriceTick {
            market_id: "m1".into(),
            outcome_id: "yes".into(),
            price: 0.50,
            timestamp: base,
        });
        index.ingest(PriceTick {
            market_id: "m1".into(),
            outcome_id: "yes".into(),
            price: 0.60,
            timestamp: base + Duration::seconds(10),
        });
        index.ingest(PriceTick {
            market_id: "m1".into(),
            outcome_id: "yes".into(),
            price: 0.45,
            timestamp: base + Duration::seconds(30),
        });

        let candles = index.query_candles("m1", "yes", Resolution::OneMinute, None, None, 100);
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].open, 0.50);
        assert_eq!(candles[0].high, 0.60);
        assert_eq!(candles[0].low, 0.45);
        assert_eq!(candles[0].close, 0.45);
        assert_eq!(candles[0].volume, 3);
    }

    #[test]
    fn test_ingest_different_periods_creates_candles() {
        let index = PriceIndex::new();

        for i in 0..5 {
            let t = ts(1768560000 + i * 60); // 5 separate minute candles
            index.ingest(PriceTick {
                market_id: "m1".into(),
                outcome_id: "yes".into(),
                price: 0.50 + (i as f64 * 0.01),
                timestamp: t,
            });
        }

        let candles = index.query_candles("m1", "yes", Resolution::OneMinute, None, None, 100);
        assert_eq!(candles.len(), 5);

        // But only 1 candle at 5m resolution (all within same 5-min window)
        let candles_5m = index.query_candles("m1", "yes", Resolution::FiveMinute, None, None, 100);
        assert_eq!(candles_5m.len(), 1);
        assert_eq!(candles_5m[0].volume, 5);
    }

    #[test]
    fn test_query_with_time_range() {
        let index = PriceIndex::new();

        for i in 0..10 {
            let t = ts(1768560000 + i * 60);
            index.ingest(PriceTick {
                market_id: "m1".into(),
                outcome_id: "yes".into(),
                price: 0.50,
                timestamp: t,
            });
        }

        // Query only candles 3-7
        let from_ts = 1768560000 + 3 * 60;
        let to_ts = 1768560000 + 7 * 60;
        let candles = index.query_candles("m1", "yes", Resolution::OneMinute, Some(from_ts), Some(to_ts), 100);
        assert_eq!(candles.len(), 5);
    }

    #[test]
    fn test_query_with_limit() {
        let index = PriceIndex::new();

        for i in 0..20 {
            let t = ts(1768560000 + i * 60);
            index.ingest(PriceTick {
                market_id: "m1".into(),
                outcome_id: "yes".into(),
                price: 0.50,
                timestamp: t,
            });
        }

        let candles = index.query_candles("m1", "yes", Resolution::OneMinute, None, None, 5);
        assert_eq!(candles.len(), 5);
        // Should be the latest 5
        assert!(candles[4].timestamp > candles[0].timestamp);
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let mut series = CandleSeries::new(3);
        series.ingest(0.50, 1000, 60);
        series.ingest(0.51, 1060, 60);
        series.ingest(0.52, 1120, 60);
        series.ingest(0.53, 1180, 60); // should evict first

        assert_eq!(series.candles.len(), 3);
        assert_eq!(series.candles[0].timestamp, 1060);
        assert_eq!(series.candles[2].timestamp, 1180);
    }

    #[test]
    fn test_stats() {
        let index = PriceIndex::new();
        index.ingest(PriceTick {
            market_id: "m1".into(),
            outcome_id: "yes".into(),
            price: 0.50,
            timestamp: Utc::now(),
        });
        index.ingest(PriceTick {
            market_id: "m2".into(),
            outcome_id: "no".into(),
            price: 0.30,
            timestamp: Utc::now(),
        });

        let stats = index.stats();
        assert_eq!(stats.ticks_ingested, 2);
        assert_eq!(stats.markets_tracked, 2);
        // 2 ticks × 4 resolutions = 8 series
        assert_eq!(stats.total_series, 8);
    }

    #[test]
    fn test_resolution_parse() {
        assert_eq!(Resolution::parse("1m"), Some(Resolution::OneMinute));
        assert_eq!(Resolution::parse("5m"), Some(Resolution::FiveMinute));
        assert_eq!(Resolution::parse("1h"), Some(Resolution::OneHour));
        assert_eq!(Resolution::parse("1d"), Some(Resolution::OneDay));
        assert_eq!(Resolution::parse("invalid"), None);
    }

    #[test]
    fn test_latest_candle() {
        let index = PriceIndex::new();
        assert!(index.latest_candle("m1", "yes", Resolution::OneMinute).is_none());

        index.ingest(PriceTick {
            market_id: "m1".into(),
            outcome_id: "yes".into(),
            price: 0.65,
            timestamp: Utc::now(),
        });

        let latest = index.latest_candle("m1", "yes", Resolution::OneMinute);
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().close, 0.65);
    }
}
