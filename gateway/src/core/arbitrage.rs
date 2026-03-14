// Cross-provider arbitrage detection engine.
//
// Continuously scans orderbooks across all providers, detecting price
// divergences where one provider's best bid exceeds another's best ask.
// Surfaces opportunities via REST, WebSocket fan-out, and Prometheus metrics.

use crate::core::aggregation::{ArbitrageOpportunity, merged_orderbook};
use crate::core::registry::ProviderRegistry;
use crate::transport::websocket::WebSocketManager;
use chrono::Utc;
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug};

// ─── Arbitrage Alert ─────────────────────────────────────────

/// An enriched arbitrage alert with metadata for consumers.
#[derive(Debug, Clone, Serialize)]
pub struct ArbitrageAlert {
    /// Unique alert ID
    pub alert_id: String,
    /// The underlying market being compared
    pub market_id: String,
    /// Outcome (e.g., "yes")
    pub outcome_id: String,
    /// Provider with the best bid (sell here)
    pub bid_provider: String,
    pub bid_price: f64,
    /// Provider with the best ask (buy here)
    pub ask_provider: String,
    pub ask_price: f64,
    /// Spread: bid - ask (positive = profitable)
    pub spread: f64,
    /// Spread as a percentage of the ask price
    pub spread_pct: f64,
    /// Estimated profit per contract (before fees)
    pub gross_profit_per_contract: f64,
    /// Estimated total fees per contract (both sides)
    pub estimated_fees: f64,
    /// Net profit after estimated fees
    pub net_profit_per_contract: f64,
    /// Maximum executable quantity (limited by thinner side)
    pub max_quantity: i64,
    /// Estimated total net profit across max_quantity
    pub estimated_total_profit: f64,
    /// Confidence score 0.0-1.0 (based on liquidity depth and spread size)
    pub confidence: f64,
    /// When this opportunity was detected
    pub detected_at: String,
    /// Whether this is still considered active
    pub active: bool,
    /// How many consecutive scans this has been detected
    pub consecutive_detections: u32,
}

// ─── Scanner State ───────────────────────────────────────────

/// Shared state for the arbitrage scanner.
pub struct ArbitrageScanner {
    /// Currently active alerts keyed by "market_id:outcome_id"
    active_alerts: DashMap<String, ArbitrageAlert>,
    /// Historical alerts (ring buffer of last N)
    history: tokio::sync::Mutex<Vec<ArbitrageAlert>>,
    /// Maximum history entries
    max_history: usize,
    /// Counters
    pub scans_total: AtomicU64,
    pub opportunities_detected: AtomicU64,
    pub opportunities_active: AtomicU64,
    /// Minimum spread percentage to trigger an alert
    pub min_spread_pct: f64,
    /// Estimated fee per side (as fraction, e.g., 0.02 = 2%)
    pub estimated_fee_rate: f64,
}

impl ArbitrageScanner {
    pub fn new(min_spread_pct: f64, estimated_fee_rate: f64) -> Self {
        Self {
            active_alerts: DashMap::new(),
            history: tokio::sync::Mutex::new(Vec::new()),
            max_history: 1000,
            scans_total: AtomicU64::new(0),
            opportunities_detected: AtomicU64::new(0),
            opportunities_active: AtomicU64::new(0),
            min_spread_pct,
            estimated_fee_rate,
        }
    }

    /// Process a detected arbitrage opportunity from a merged orderbook scan.
    pub async fn process_opportunity(
        &self,
        market_id: &str,
        outcome_id: &str,
        arb: &ArbitrageOpportunity,
        max_quantity: i64,
    ) -> Option<ArbitrageAlert> {
        let bid_price: f64 = arb.bid_price.parse().unwrap_or(0.0);
        let ask_price: f64 = arb.ask_price.parse().unwrap_or(0.0);
        let spread = bid_price - ask_price;
        let spread_pct = if ask_price > 0.0 { (spread / ask_price) * 100.0 } else { 0.0 };

        // Filter by minimum spread
        if spread_pct < self.min_spread_pct {
            return None;
        }

        // Calculate fees and net profit
        let fee_per_side = self.estimated_fee_rate;
        let buy_fee = ask_price * fee_per_side;
        let sell_fee = bid_price * fee_per_side;
        let total_fees = buy_fee + sell_fee;
        let net_profit = spread - total_fees;

        // Only alert if profitable after fees
        if net_profit <= 0.0 {
            return None;
        }

        // Confidence based on spread size and liquidity
        let spread_confidence = (spread_pct / 10.0).min(1.0); // maxes at 10% spread
        let liquidity_confidence = (max_quantity as f64 / 100.0).min(1.0); // maxes at 100 contracts
        let confidence = (spread_confidence * 0.6 + liquidity_confidence * 0.4).min(1.0);

        let key = format!("{}:{}", market_id, outcome_id);

        // Check if this is a recurring detection
        let consecutive = self.active_alerts
            .get(&key)
            .map(|a| a.consecutive_detections + 1)
            .unwrap_or(1);

        let alert = ArbitrageAlert {
            alert_id: format!("arb-{}", uuid::Uuid::new_v4()),
            market_id: market_id.to_string(),
            outcome_id: outcome_id.to_string(),
            bid_provider: arb.bid_provider.clone(),
            bid_price,
            ask_provider: arb.ask_provider.clone(),
            ask_price,
            spread,
            spread_pct,
            gross_profit_per_contract: spread,
            estimated_fees: total_fees,
            net_profit_per_contract: net_profit,
            max_quantity,
            estimated_total_profit: net_profit * max_quantity as f64,
            confidence,
            detected_at: Utc::now().to_rfc3339(),
            active: true,
            consecutive_detections: consecutive,
        };

        // Update active alerts
        self.active_alerts.insert(key, alert.clone());
        self.opportunities_detected.fetch_add(1, Ordering::Relaxed);
        self.opportunities_active.store(
            self.active_alerts.len() as u64,
            Ordering::Relaxed,
        );

        // Add to history
        let mut history = self.history.lock().await;
        if history.len() >= self.max_history {
            history.remove(0);
        }
        history.push(alert.clone());

        Some(alert)
    }

    /// Mark opportunities as inactive if they weren't detected in the latest scan.
    pub async fn expire_stale(&self, scanned_keys: &[String]) {
        let all_keys: Vec<String> = self.active_alerts.iter().map(|r| r.key().clone()).collect();
        for key in all_keys {
            if !scanned_keys.contains(&key) {
                if let Some(mut alert) = self.active_alerts.get_mut(&key) {
                    alert.active = false;
                }
                self.active_alerts.remove(&key);
            }
        }
        self.opportunities_active.store(
            self.active_alerts.len() as u64,
            Ordering::Relaxed,
        );
    }

    /// Get all currently active arbitrage alerts.
    pub fn get_active_alerts(&self) -> Vec<ArbitrageAlert> {
        self.active_alerts
            .iter()
            .map(|r| r.value().clone())
            .collect()
    }

    /// Get recent historical alerts.
    pub async fn get_history(&self, limit: usize) -> Vec<ArbitrageAlert> {
        let history = self.history.lock().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Get a summary of arbitrage activity.
    pub async fn get_summary(&self) -> ArbitrageSummary {
        let active = self.get_active_alerts();
        let total_profit: f64 = active.iter().map(|a| a.estimated_total_profit).sum();
        let avg_spread: f64 = if active.is_empty() {
            0.0
        } else {
            active.iter().map(|a| a.spread_pct).sum::<f64>() / active.len() as f64
        };
        let best = active.iter()
            .max_by(|a, b| a.net_profit_per_contract.partial_cmp(&b.net_profit_per_contract)
                .unwrap_or(std::cmp::Ordering::Equal))
            .cloned();

        ArbitrageSummary {
            active_opportunities: active.len(),
            total_scans: self.scans_total.load(Ordering::Relaxed),
            total_detected: self.opportunities_detected.load(Ordering::Relaxed),
            total_estimated_profit: format!("{:.4}", total_profit),
            average_spread_pct: format!("{:.2}", avg_spread),
            best_opportunity: best,
            min_spread_threshold: self.min_spread_pct,
            estimated_fee_rate: self.estimated_fee_rate,
        }
    }
}

/// Summary statistics for the arbitrage scanner.
#[derive(Debug, Clone, Serialize)]
pub struct ArbitrageSummary {
    pub active_opportunities: usize,
    pub total_scans: u64,
    pub total_detected: u64,
    pub total_estimated_profit: String,
    pub average_spread_pct: String,
    pub best_opportunity: Option<ArbitrageAlert>,
    pub min_spread_threshold: f64,
    pub estimated_fee_rate: f64,
}

// ─── Background Scanner ─────────────────────────────────────

/// Start the background arbitrage scanner that polls markets on an interval.
///
/// Scans all known markets, fetches merged orderbooks, and publishes
/// alerts to both the scanner state and the WebSocket fan-out system.
pub fn start_arbitrage_scanner(
    scanner: Arc<ArbitrageScanner>,
    registry: Arc<ProviderRegistry>,
    ws_manager: Arc<WebSocketManager>,
    interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));
        info!(
            interval_ms = interval_ms,
            min_spread = scanner.min_spread_pct,
            "Arbitrage scanner started"
        );

        loop {
            interval.tick().await;

            let scan_result = run_scan(&scanner, &registry, &ws_manager).await;
            scanner.scans_total.fetch_add(1, Ordering::Relaxed);

            if scan_result > 0 {
                info!(
                    opportunities = scan_result,
                    "Arbitrage scan complete — {} active opportunities",
                    scan_result
                );
            } else {
                debug!("Arbitrage scan complete — no opportunities");
            }
        }
    })
}

/// Run a single arbitrage scan across all providers.
async fn run_scan(
    scanner: &ArbitrageScanner,
    registry: &ProviderRegistry,
    ws_manager: &WebSocketManager,
) -> usize {
    let provider_ids = registry.provider_ids();

    // Need at least 2 providers for cross-provider arbitrage
    if provider_ids.len() < 2 {
        return 0;
    }

    // Get markets from all providers to find overlapping ones
    // For now, scan all known market pairs across providers
    // In production, this would use a market-matching index
    let mut native_ids_per_market: HashMap<String, HashMap<String, String>> = HashMap::new();

    // Build a map of canonical market -> { provider -> native_id }
    // Using the mock adapters, each provider generates its own markets
    // We scan each provider's markets and group by similar titles/slugs
    for pid in &provider_ids {
        if let Some(adapter) = registry.get(pid) {
            let filter = crate::adapters::MarketFilter {
                pagination: crate::core::types::PaginationRequest {
                    limit: Some(20),
                    cursor: None,
                },
                ..Default::default()
            };
            if let Ok(page) = adapter.list_markets(filter).await {
                for market in &page.markets {
                    // Use native_id as the grouping key for cross-provider matching
                    // In production, this would use NLP/similarity matching
                    let canonical = market.id.native_id.clone();
                    native_ids_per_market
                        .entry(canonical.clone())
                        .or_default()
                        .insert(pid.clone(), market.id.native_id.clone());
                }
            }
        }
    }

    let mut scanned_keys = Vec::new();
    let mut found = 0;

    // For each market that exists on 2+ providers, fetch merged orderbooks
    for (canonical_id, provider_map) in &native_ids_per_market {
        if provider_map.len() < 2 {
            continue;
        }

        let merged = merged_orderbook(registry, provider_map, Some("yes"), 10).await;

        let market_id = format!("upp:{}", canonical_id);
        let outcome_id = "yes";
        let key = format!("{}:{}", market_id, outcome_id);
        scanned_keys.push(key);

        if let Some(arb) = &merged.arbitrage {
            // Determine max executable quantity from merged book
            let max_qty = merged.bids.first()
                .map(|b| b.total_quantity)
                .unwrap_or(0)
                .min(merged.asks.first().map(|a| a.total_quantity).unwrap_or(0));

            if let Some(alert) = scanner.process_opportunity(
                &market_id, outcome_id, arb, max_qty,
            ).await {
                // Broadcast via WebSocket fan-out
                let alert_data = serde_json::to_value(&alert).unwrap_or_default();
                ws_manager.publish("arbitrage", &market_id, alert_data).await;
                found += 1;
            }
        }
    }

    // Expire stale alerts
    scanner.expire_stale(&scanned_keys).await;

    found
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::aggregation::ArbitrageOpportunity;

    fn make_arb(bid_provider: &str, bid: f64, ask_provider: &str, ask: f64) -> ArbitrageOpportunity {
        ArbitrageOpportunity {
            description: format!("Buy {} at {}, sell {} at {}", ask_provider, ask, bid_provider, bid),
            bid_provider: bid_provider.to_string(),
            bid_price: format!("{:.4}", bid),
            ask_provider: ask_provider.to_string(),
            ask_price: format!("{:.4}", ask),
            spread_pct: ((bid - ask) / ask) * 100.0,
            profit_per_contract: format!("{:.4}", bid - ask),
        }
    }

    #[tokio::test]
    async fn test_process_opportunity_profitable() {
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        let arb = make_arb("kalshi.com", 0.65, "polymarket.com", 0.58);

        let alert = scanner.process_opportunity("market-1", "yes", &arb, 100).await;
        assert!(alert.is_some());

        let alert = alert.unwrap();
        assert!(alert.spread > 0.0);
        assert!(alert.net_profit_per_contract > 0.0);
        assert_eq!(alert.max_quantity, 100);
        assert!(alert.active);
        assert_eq!(alert.consecutive_detections, 1);
    }

    #[tokio::test]
    async fn test_process_opportunity_below_min_spread() {
        // 0.1% spread is below the 0.5% minimum
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        let arb = make_arb("kalshi.com", 0.601, "polymarket.com", 0.600);

        let alert = scanner.process_opportunity("market-2", "yes", &arb, 50).await;
        assert!(alert.is_none());
    }

    #[tokio::test]
    async fn test_process_opportunity_unprofitable_after_fees() {
        // Small spread that doesn't cover the 2% per-side fee
        let scanner = ArbitrageScanner::new(0.1, 0.05); // 5% fee per side
        let arb = make_arb("kalshi.com", 0.52, "polymarket.com", 0.50);

        let alert = scanner.process_opportunity("market-3", "yes", &arb, 50).await;
        assert!(alert.is_none());
    }

    #[tokio::test]
    async fn test_consecutive_detections() {
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        let arb = make_arb("kalshi.com", 0.70, "polymarket.com", 0.55);

        // First detection
        let alert1 = scanner.process_opportunity("market-4", "yes", &arb, 100).await.unwrap();
        assert_eq!(alert1.consecutive_detections, 1);

        // Second detection of same opportunity
        let alert2 = scanner.process_opportunity("market-4", "yes", &arb, 100).await.unwrap();
        assert_eq!(alert2.consecutive_detections, 2);

        // Third
        let alert3 = scanner.process_opportunity("market-4", "yes", &arb, 100).await.unwrap();
        assert_eq!(alert3.consecutive_detections, 3);
    }

    #[tokio::test]
    async fn test_active_alerts_and_expiry() {
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        let arb = make_arb("kalshi.com", 0.70, "polymarket.com", 0.55);

        scanner.process_opportunity("market-5", "yes", &arb, 100).await;
        scanner.process_opportunity("market-6", "yes", &arb, 50).await;

        assert_eq!(scanner.get_active_alerts().len(), 2);

        // Expire market-5 (only market-6 was scanned)
        scanner.expire_stale(&["market-6:yes".to_string()]).await;

        assert_eq!(scanner.get_active_alerts().len(), 1);
        assert_eq!(scanner.get_active_alerts()[0].market_id, "market-6");
    }

    #[tokio::test]
    async fn test_history() {
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        let arb = make_arb("kalshi.com", 0.70, "polymarket.com", 0.55);

        for i in 0..5 {
            scanner.process_opportunity(&format!("market-{}", i), "yes", &arb, 100).await;
        }

        let history = scanner.get_history(3).await;
        assert_eq!(history.len(), 3);
        // Most recent first
        assert_eq!(history[0].market_id, "market-4");
    }

    #[tokio::test]
    async fn test_summary() {
        let scanner = ArbitrageScanner::new(0.5, 0.02);
        let arb = make_arb("kalshi.com", 0.70, "polymarket.com", 0.55);

        scanner.process_opportunity("market-7", "yes", &arb, 100).await;
        scanner.scans_total.store(10, Ordering::Relaxed);

        let summary = scanner.get_summary().await;
        assert_eq!(summary.active_opportunities, 1);
        assert_eq!(summary.total_scans, 10);
        assert!(summary.best_opportunity.is_some());
    }

    #[tokio::test]
    async fn test_confidence_score() {
        let scanner = ArbitrageScanner::new(0.5, 0.01);

        // Large spread + high liquidity = high confidence
        let arb = make_arb("kalshi.com", 0.80, "polymarket.com", 0.50);
        let alert = scanner.process_opportunity("conf-high", "yes", &arb, 200).await.unwrap();
        assert!(alert.confidence > 0.8);

        // Small spread + low liquidity = low confidence
        let arb2 = make_arb("kalshi.com", 0.52, "polymarket.com", 0.50);
        let alert2 = scanner.process_opportunity("conf-low", "yes", &arb2, 5).await.unwrap();
        assert!(alert2.confidence < alert.confidence);
    }
}
