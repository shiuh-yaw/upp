// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Multi-provider aggregation — parallel fan-out queries, merged orderbooks,
// and cross-provider arbitrage detection.
//
// Key features:
//   - Parallel provider queries with configurable timeout
//   - Merged orderbooks combining liquidity across providers
//   - Arbitrage detection when prices diverge cross-provider
//   - Error isolation: one provider failure doesn't block others

use crate::adapters::{MarketFilter, OrderBookLevel, OrderBookSnapshot};
use crate::core::registry::ProviderRegistry;
use crate::core::types::*;
use futures::future::join_all;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tracing::warn;

/// Timeout for individual provider calls during fan-out.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(5);

// ─── Parallel Market Queries ─────────────────────────────────

/// Result from a single provider query.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderResult<T> {
    pub provider: String,
    pub data: Option<T>,
    pub error: Option<String>,
    pub latency_ms: u64,
}

/// Aggregated result from multiple providers.
#[derive(Debug, Clone, Serialize)]
pub struct AggregatedMarkets {
    pub markets: Vec<Market>,
    pub provider_results: Vec<ProviderResult<usize>>,  // count per provider
    pub total: usize,
    pub errors: Vec<ProviderError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderError {
    pub provider: String,
    pub error: String,
}

/// Query markets from all providers in parallel.
/// Each provider runs independently — failures are isolated.
pub async fn parallel_list_markets(
    registry: &ProviderRegistry,
    filter: MarketFilter,
    provider_ids: Option<Vec<String>>,
) -> AggregatedMarkets {
    let pids = provider_ids.unwrap_or_else(|| registry.provider_ids());

    // Spawn parallel queries
    let futures: Vec<_> = pids.iter().map(|pid| {
        let pid = pid.clone();
        let f = filter.clone();
        let adapter = registry.get(&pid);
        async move {
            let start = std::time::Instant::now();
            match adapter {
                Some(adapter) => {
                    match tokio::time::timeout(PROVIDER_TIMEOUT, adapter.list_markets(f)).await {
                        Ok(Ok(page)) => {
                            ProviderResult {
                                provider: pid,
                                data: Some(page.markets),
                                error: None,
                                latency_ms: start.elapsed().as_millis() as u64,
                            }
                        }
                        Ok(Err(e)) => {
                            warn!(provider = %pid, "list_markets error: {}", e);
                            ProviderResult {
                                provider: pid,
                                data: None,
                                error: Some(e.to_string()),
                                latency_ms: start.elapsed().as_millis() as u64,
                            }
                        }
                        Err(_) => {
                            warn!(provider = %pid, "list_markets timeout");
                            ProviderResult {
                                provider: pid,
                                data: None,
                                error: Some("Timeout".to_string()),
                                latency_ms: PROVIDER_TIMEOUT.as_millis() as u64,
                            }
                        }
                    }
                }
                None => ProviderResult {
                    provider: pid,
                    data: None,
                    error: Some("Provider not found".to_string()),
                    latency_ms: 0,
                },
            }
        }
    }).collect();

    let results: Vec<ProviderResult<Vec<Market>>> = join_all(futures).await;

    // Merge results
    let mut all_markets = Vec::new();
    let mut provider_results = Vec::new();
    let mut errors = Vec::new();

    for result in results {
        let pid = result.provider.clone();
        match result.data {
            Some(markets) => {
                let count = markets.len();
                all_markets.extend(markets);
                provider_results.push(ProviderResult {
                    provider: pid,
                    data: Some(count),
                    error: None,
                    latency_ms: result.latency_ms,
                });
            }
            None => {
                if let Some(err) = &result.error {
                    errors.push(ProviderError {
                        provider: pid.clone(),
                        error: err.clone(),
                    });
                }
                provider_results.push(ProviderResult {
                    provider: pid,
                    data: Some(0),
                    error: result.error,
                    latency_ms: result.latency_ms,
                });
            }
        }
    }

    // Sort by volume (highest first)
    all_markets.sort_by(|a, b| {
        let va: f64 = a.volume.volume_24h.parse().unwrap_or(0.0);
        let vb: f64 = b.volume.volume_24h.parse().unwrap_or(0.0);
        vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = all_markets.len();
    AggregatedMarkets { markets: all_markets, provider_results, total, errors }
}

/// Search markets across all providers in parallel.
pub async fn parallel_search_markets(
    registry: &ProviderRegistry,
    query: &str,
    filter: MarketFilter,
) -> AggregatedMarkets {
    let pids = registry.provider_ids();
    let query_owned = query.to_string();

    let futures: Vec<_> = pids.iter().map(|pid| {
        let pid = pid.clone();
        let f = filter.clone();
        let q = query_owned.clone();
        let adapter = registry.get(&pid);
        async move {
            let start = std::time::Instant::now();
            match adapter {
                Some(adapter) => {
                    match tokio::time::timeout(PROVIDER_TIMEOUT, adapter.search_markets(&q, f)).await {
                        Ok(Ok(page)) => ProviderResult {
                            provider: pid,
                            data: Some(page.markets),
                            error: None,
                            latency_ms: start.elapsed().as_millis() as u64,
                        },
                        Ok(Err(e)) => ProviderResult {
                            provider: pid,
                            data: None,
                            error: Some(e.to_string()),
                            latency_ms: start.elapsed().as_millis() as u64,
                        },
                        Err(_) => ProviderResult {
                            provider: pid,
                            data: None,
                            error: Some("Timeout".to_string()),
                            latency_ms: PROVIDER_TIMEOUT.as_millis() as u64,
                        },
                    }
                }
                None => ProviderResult {
                    provider: pid,
                    data: None,
                    error: Some("Provider not found".to_string()),
                    latency_ms: 0,
                },
            }
        }
    }).collect();

    let results: Vec<ProviderResult<Vec<Market>>> = join_all(futures).await;

    let mut all_markets = Vec::new();
    let mut provider_results = Vec::new();
    let mut errors = Vec::new();

    for result in results {
        let pid = result.provider.clone();
        match result.data {
            Some(markets) => {
                let count = markets.len();
                all_markets.extend(markets);
                provider_results.push(ProviderResult {
                    provider: pid, data: Some(count), error: None, latency_ms: result.latency_ms,
                });
            }
            None => {
                if let Some(err) = &result.error {
                    errors.push(ProviderError { provider: pid.clone(), error: err.clone() });
                }
                provider_results.push(ProviderResult {
                    provider: pid, data: Some(0), error: result.error, latency_ms: result.latency_ms,
                });
            }
        }
    }

    all_markets.sort_by(|a, b| {
        let va: f64 = a.volume.volume_24h.parse().unwrap_or(0.0);
        let vb: f64 = b.volume.volume_24h.parse().unwrap_or(0.0);
        vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
    });

    let total = all_markets.len();
    AggregatedMarkets { markets: all_markets, provider_results, total, errors }
}

// ─── Merged Orderbooks ───────────────────────────────────────

/// A merged orderbook combining liquidity from multiple providers.
#[derive(Debug, Clone, Serialize)]
pub struct MergedOrderBook {
    /// Market ID (or search key)
    pub market_id: String,
    pub outcome_id: String,
    /// Bids sorted by price descending (best bid first)
    pub bids: Vec<MergedLevel>,
    /// Asks sorted by price ascending (best ask first)
    pub asks: Vec<MergedLevel>,
    /// Per-provider snapshots
    pub provider_books: Vec<ProviderOrderBook>,
    /// Detected arbitrage opportunities
    pub arbitrage: Option<ArbitrageOpportunity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MergedLevel {
    pub price: String,
    pub total_quantity: i64,
    pub providers: Vec<ProviderLevelContribution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderLevelContribution {
    pub provider: String,
    pub quantity: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderOrderBook {
    pub provider: String,
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
    pub latency_ms: u64,
}

/// Cross-provider arbitrage opportunity.
#[derive(Debug, Clone, Serialize)]
pub struct ArbitrageOpportunity {
    /// Description of the opportunity
    pub description: String,
    /// Provider with the best bid (sell here)
    pub bid_provider: String,
    /// Best bid price
    pub bid_price: String,
    /// Provider with the best ask (buy here)
    pub ask_provider: String,
    /// Best ask price
    pub ask_price: String,
    /// Spread as a percentage
    pub spread_pct: f64,
    /// Estimated profit per contract
    pub profit_per_contract: String,
}

/// Fetch orderbooks from multiple providers in parallel and merge them.
pub async fn merged_orderbook(
    registry: &ProviderRegistry,
    native_ids: &HashMap<String, String>,  // provider -> native_market_id
    outcome_id: Option<&str>,
    depth: i32,
) -> MergedOrderBook {
    let futures: Vec<_> = native_ids.iter().map(|(pid, native_id)| {
        let pid = pid.clone();
        let nid = native_id.clone();
        let outcome = outcome_id.map(String::from);
        let adapter = registry.get(&pid);
        async move {
            let start = std::time::Instant::now();
            match adapter {
                Some(adapter) => {
                    match tokio::time::timeout(
                        PROVIDER_TIMEOUT,
                        adapter.get_orderbook(&nid, outcome.as_deref(), depth),
                    ).await {
                        Ok(Ok(snapshots)) => {
                            let latency = start.elapsed().as_millis() as u64;
                            Some((pid, snapshots, latency))
                        }
                        Ok(Err(e)) => {
                            warn!(provider = %pid, "orderbook error: {}", e);
                            None
                        }
                        Err(_) => {
                            warn!(provider = %pid, "orderbook timeout");
                            None
                        }
                    }
                }
                None => None,
            }
        }
    }).collect();

    let results: Vec<Option<(String, Vec<OrderBookSnapshot>, u64)>> = join_all(futures).await;

    // Collect per-provider books
    let mut provider_books = Vec::new();
    let mut all_bids: HashMap<String, Vec<ProviderLevelContribution>> = HashMap::new();
    let mut all_asks: HashMap<String, Vec<ProviderLevelContribution>> = HashMap::new();

    // Track best bid/ask per provider for arbitrage detection
    let mut best_bids: Vec<(String, f64)> = Vec::new();
    let mut best_asks: Vec<(String, f64)> = Vec::new();

    for result in results.into_iter().flatten() {
        let (pid, snapshots, latency) = result;
        for snap in &snapshots {
            // Merge bids
            for level in &snap.bids {
                all_bids.entry(level.price.clone())
                    .or_default()
                    .push(ProviderLevelContribution {
                        provider: pid.clone(),
                        quantity: level.quantity,
                    });
            }

            // Merge asks
            for level in &snap.asks {
                all_asks.entry(level.price.clone())
                    .or_default()
                    .push(ProviderLevelContribution {
                        provider: pid.clone(),
                        quantity: level.quantity,
                    });
            }

            // Track best bid/ask
            if let Some(best_bid) = snap.bids.first() {
                if let Ok(price) = best_bid.price.parse::<f64>() {
                    best_bids.push((pid.clone(), price));
                }
            }
            if let Some(best_ask) = snap.asks.first() {
                if let Ok(price) = best_ask.price.parse::<f64>() {
                    best_asks.push((pid.clone(), price));
                }
            }

            provider_books.push(ProviderOrderBook {
                provider: pid.clone(),
                bids: snap.bids.clone(),
                asks: snap.asks.clone(),
                latency_ms: latency,
            });
        }
    }

    // Build merged levels — bids descending, asks ascending
    let mut merged_bids: Vec<MergedLevel> = all_bids.into_iter().map(|(price, contribs)| {
        let total = contribs.iter().map(|c| c.quantity).sum();
        MergedLevel { price, total_quantity: total, providers: contribs }
    }).collect();
    merged_bids.sort_by(|a, b| {
        let pa: f64 = b.price.parse().unwrap_or(0.0);
        let pb_val: f64 = a.price.parse().unwrap_or(0.0);
        pa.partial_cmp(&pb_val).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut merged_asks: Vec<MergedLevel> = all_asks.into_iter().map(|(price, contribs)| {
        let total = contribs.iter().map(|c| c.quantity).sum();
        MergedLevel { price, total_quantity: total, providers: contribs }
    }).collect();
    merged_asks.sort_by(|a, b| {
        let pa: f64 = a.price.parse().unwrap_or(0.0);
        let pb_val: f64 = b.price.parse().unwrap_or(0.0);
        pa.partial_cmp(&pb_val).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Detect cross-provider arbitrage
    let arbitrage = detect_arbitrage(&best_bids, &best_asks);

    MergedOrderBook {
        market_id: String::new(),
        outcome_id: outcome_id.unwrap_or("yes").to_string(),
        bids: merged_bids,
        asks: merged_asks,
        provider_books,
        arbitrage,
    }
}

/// Detect cross-provider arbitrage: provider A's best bid > provider B's best ask.
fn detect_arbitrage(
    best_bids: &[(String, f64)],
    best_asks: &[(String, f64)],
) -> Option<ArbitrageOpportunity> {
    if best_bids.is_empty() || best_asks.is_empty() {
        return None;
    }

    // Find highest bid and lowest ask across different providers
    let (bid_provider, bid_price) = best_bids.iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;

    let (ask_provider, ask_price) = best_asks.iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;

    // Only an arb if bid > ask AND they're from different providers
    if bid_price > ask_price && bid_provider != ask_provider {
        let spread = bid_price - ask_price;
        let spread_pct = (spread / ask_price) * 100.0;
        Some(ArbitrageOpportunity {
            description: format!(
                "Buy on {} at {:.4}, sell on {} at {:.4} for {:.4} profit ({:.2}%)",
                ask_provider, ask_price, bid_provider, bid_price, spread, spread_pct
            ),
            bid_provider: bid_provider.clone(),
            bid_price: format!("{:.4}", bid_price),
            ask_provider: ask_provider.clone(),
            ask_price: format!("{:.4}", ask_price),
            spread_pct,
            profit_per_contract: format!("{:.4}", spread),
        })
    } else {
        None
    }
}
