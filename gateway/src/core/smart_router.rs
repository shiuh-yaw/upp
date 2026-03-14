// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Smart Order Router — automatically finds the best price across all
// providers and routes orders to minimize cost / maximize fill probability.
// Extends the arbitrage scanner's cross-provider view into actionable routing.

use crate::adapters::{OrderBookSnapshot, OrderBookLevel, CreateOrderRequest};
use crate::core::types::*;
use crate::core::registry::ProviderRegistry;
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::debug;

// ─── Types ──────────────────────────────────────────────────

/// A routing plan describing how to fill an order optimally.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingPlan {
    pub market_native_id: String,
    pub outcome_id: String,
    pub side: String,
    pub total_quantity: i64,
    pub legs: Vec<RoutingLeg>,
    pub estimated_total_cost: f64,
    pub estimated_avg_price: f64,
    pub estimated_fees: f64,
    pub naive_cost: f64,         // cost if sent to first provider only
    pub savings: f64,            // naive_cost - estimated_total_cost
    pub savings_pct: f64,
    pub providers_considered: usize,
    pub computed_at: String,
}

/// One leg of the routing plan — an order to a specific provider.
#[derive(Debug, Clone, Serialize)]
pub struct RoutingLeg {
    pub provider: String,
    pub price: f64,
    pub quantity: i64,
    pub estimated_cost: f64,
    pub estimated_fee: f64,
    pub fill_probability: f64,   // 0-1, based on available liquidity
    pub priority: u32,           // 1 = best, execute first
}

/// Routing strategy selection.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum RoutingStrategy {
    /// Route to the single provider with the best price.
    BestPrice,
    /// Split across multiple providers for optimal fill.
    SplitOptimal,
    /// Route only to a specific provider (bypass smart routing).
    DirectRoute,
}

impl RoutingStrategy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "best_price" | "best" => Some(Self::BestPrice),
            "split" | "split_optimal" | "optimal" => Some(Self::SplitOptimal),
            "direct" => Some(Self::DirectRoute),
            _ => None,
        }
    }
}

/// Provider liquidity snapshot for one outcome.
#[derive(Debug, Clone)]
struct ProviderLiquidity {
    provider_id: String,
    best_price: f64,
    available_quantity: i64,
    levels: Vec<(f64, i64)>, // (price, qty) pairs
    estimated_fee_rate: f64,
}

// ─── Smart Router ───────────────────────────────────────────

pub struct SmartRouter {
    default_fee_rate: f64,
    routes_computed: AtomicU64,
    orders_routed: AtomicU64,
}

impl SmartRouter {
    pub fn new(default_fee_rate: f64) -> Self {
        Self {
            default_fee_rate,
            routes_computed: AtomicU64::new(0),
            orders_routed: AtomicU64::new(0),
        }
    }

    /// Compute the optimal routing plan for an order.
    pub async fn compute_route(
        &self,
        registry: &ProviderRegistry,
        market_native_id: &str,
        outcome_id: &str,
        side: Side,
        quantity: i64,
        strategy: RoutingStrategy,
        preferred_provider: Option<&str>,
    ) -> anyhow::Result<RoutingPlan> {
        self.routes_computed.fetch_add(1, Ordering::Relaxed);

        // If DirectRoute, skip comparison
        if strategy == RoutingStrategy::DirectRoute {
            if let Some(pid) = preferred_provider {
                return self.direct_route(registry, pid, market_native_id, outcome_id, side, quantity).await;
            }
            anyhow::bail!("DirectRoute requires a preferred_provider");
        }

        // Gather liquidity from all providers
        let mut provider_liquidity = Vec::new();
        for pid in registry.provider_ids() {
            if let Some(adapter) = registry.get(&pid) {
                match adapter.get_orderbook(market_native_id, Some(outcome_id), 10).await {
                    Ok(snapshots) => {
                        for snap in snapshots {
                            if snap.outcome_id == outcome_id {
                                let liquidity = self.extract_liquidity(&pid, &snap, side);
                                if liquidity.available_quantity > 0 {
                                    provider_liquidity.push(liquidity);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!(provider = %pid, "Skipping provider for routing: {}", e);
                    }
                }
            }
        }

        if provider_liquidity.is_empty() {
            anyhow::bail!("No liquidity available across any provider");
        }

        let plan = match strategy {
            RoutingStrategy::BestPrice => self.route_best_price(
                &provider_liquidity, market_native_id, outcome_id, side, quantity,
            ),
            RoutingStrategy::SplitOptimal => self.route_split_optimal(
                &provider_liquidity, market_native_id, outcome_id, side, quantity,
            ),
            RoutingStrategy::DirectRoute => unreachable!(),
        };

        Ok(plan)
    }

    /// Execute a routing plan — place orders to each provider in priority order.
    pub async fn execute_plan(
        &self,
        registry: &ProviderRegistry,
        plan: &RoutingPlan,
        side: Side,
        order_type: OrderType,
        tif: TimeInForce,
    ) -> Vec<ExecutionResult> {
        let mut results = Vec::new();

        let mut sorted_legs = plan.legs.clone();
        sorted_legs.sort_by_key(|l| l.priority);

        for leg in &sorted_legs {
            if let Some(adapter) = registry.get(&leg.provider) {
                let req = CreateOrderRequest {
                    market_native_id: plan.market_native_id.clone(),
                    outcome_id: plan.outcome_id.clone(),
                    side,
                    order_type,
                    tif,
                    price: Some(format!("{:.2}", leg.price)),
                    quantity: leg.quantity,
                    client_order_id: Some(format!("smart-{}", uuid::Uuid::new_v4())),
                };

                match adapter.create_order(req).await {
                    Ok(order) => {
                        self.orders_routed.fetch_add(1, Ordering::Relaxed);
                        results.push(ExecutionResult {
                            provider: leg.provider.clone(),
                            quantity: leg.quantity,
                            price: leg.price,
                            status: "placed".to_string(),
                            order_id: Some(order.id),
                            error: None,
                        });
                    }
                    Err(e) => {
                        results.push(ExecutionResult {
                            provider: leg.provider.clone(),
                            quantity: leg.quantity,
                            price: leg.price,
                            status: "failed".to_string(),
                            order_id: None,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
        }

        results
    }

    pub fn stats(&self) -> RouterStats {
        RouterStats {
            routes_computed: self.routes_computed.load(Ordering::Relaxed),
            orders_routed: self.orders_routed.load(Ordering::Relaxed),
        }
    }

    // ── Private ─────────────────────────────────────────────

    fn extract_liquidity(
        &self,
        provider_id: &str,
        snapshot: &OrderBookSnapshot,
        side: Side,
    ) -> ProviderLiquidity {
        // For a Buy order, we look at the asks (we're buying from sellers)
        // For a Sell order, we look at the bids (we're selling to buyers)
        let levels: &Vec<OrderBookLevel> = match side {
            Side::Buy => &snapshot.asks,
            Side::Sell => &snapshot.bids,
        };

        let parsed: Vec<(f64, i64)> = levels.iter()
            .filter_map(|l| {
                l.price.parse::<f64>().ok().map(|p| (p, l.quantity))
            })
            .collect();

        let best_price = match side {
            Side::Buy => parsed.iter().map(|(p, _)| *p).fold(f64::MAX, f64::min),
            Side::Sell => parsed.iter().map(|(p, _)| *p).fold(0.0_f64, f64::max),
        };

        let total_qty: i64 = parsed.iter().map(|(_, q)| *q).sum();

        ProviderLiquidity {
            provider_id: provider_id.to_string(),
            best_price: if best_price == f64::MAX { 0.0 } else { best_price },
            available_quantity: total_qty,
            levels: parsed,
            estimated_fee_rate: self.default_fee_rate,
        }
    }

    fn route_best_price(
        &self,
        liquidity: &[ProviderLiquidity],
        market_native_id: &str,
        outcome_id: &str,
        side: Side,
        quantity: i64,
    ) -> RoutingPlan {
        // Sort providers by best price
        let mut sorted = liquidity.to_vec();
        match side {
            Side::Buy => sorted.sort_by(|a, b| a.best_price.partial_cmp(&b.best_price).unwrap_or(std::cmp::Ordering::Equal)),
            Side::Sell => sorted.sort_by(|a, b| b.best_price.partial_cmp(&a.best_price).unwrap_or(std::cmp::Ordering::Equal)),
        }

        let best = &sorted[0];
        let fill_qty = quantity.min(best.available_quantity);
        let cost = best.best_price * fill_qty as f64;
        let fee = cost * best.estimated_fee_rate;
        let fill_prob = if best.available_quantity >= quantity { 1.0 }
            else { best.available_quantity as f64 / quantity as f64 };

        let naive_cost = cost; // For best price, naive = actual since it's single provider

        let legs = vec![RoutingLeg {
            provider: best.provider_id.clone(),
            price: best.best_price,
            quantity: fill_qty,
            estimated_cost: cost,
            estimated_fee: fee,
            fill_probability: fill_prob,
            priority: 1,
        }];

        RoutingPlan {
            market_native_id: market_native_id.to_string(),
            outcome_id: outcome_id.to_string(),
            side: format!("{:?}", side).to_lowercase(),
            total_quantity: quantity,
            estimated_total_cost: cost + fee,
            estimated_avg_price: best.best_price,
            estimated_fees: fee,
            naive_cost: naive_cost + fee,
            savings: 0.0,
            savings_pct: 0.0,
            providers_considered: liquidity.len(),
            legs,
            computed_at: Utc::now().to_rfc3339(),
        }
    }

    fn route_split_optimal(
        &self,
        liquidity: &[ProviderLiquidity],
        market_native_id: &str,
        outcome_id: &str,
        side: Side,
        quantity: i64,
    ) -> RoutingPlan {
        // Build a global sorted level book across all providers
        let mut global_levels: Vec<(f64, i64, String)> = Vec::new();
        for prov in liquidity {
            for &(price, qty) in &prov.levels {
                global_levels.push((price, qty, prov.provider_id.clone()));
            }
        }

        // Sort by price: ascending for buy (cheapest first), descending for sell
        match side {
            Side::Buy => global_levels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)),
            Side::Sell => global_levels.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)),
        }

        // Walk the book and fill optimally
        let mut remaining = quantity;
        let mut legs_map: HashMap<String, RoutingLeg> = HashMap::new();
        let mut total_cost = 0.0_f64;
        let mut priority = 1_u32;

        for (price, qty, provider) in &global_levels {
            if remaining <= 0 { break; }
            let fill = remaining.min(*qty);
            let cost = *price * fill as f64;
            let fee_rate = liquidity.iter()
                .find(|l| l.provider_id == *provider)
                .map(|l| l.estimated_fee_rate)
                .unwrap_or(self.default_fee_rate);

            let leg = legs_map.entry(provider.clone()).or_insert_with(|| RoutingLeg {
                provider: provider.clone(),
                price: *price,
                quantity: 0,
                estimated_cost: 0.0,
                estimated_fee: 0.0,
                fill_probability: 1.0,
                priority: {
                    let p = priority;
                    priority += 1;
                    p
                },
            });

            leg.quantity += fill;
            leg.estimated_cost += cost;
            leg.estimated_fee += cost * fee_rate;
            leg.price = *price; // Update to worst price in this leg
            total_cost += cost;
            remaining -= fill;
        }

        let legs: Vec<RoutingLeg> = legs_map.into_values().collect();
        let total_fees: f64 = legs.iter().map(|l| l.estimated_fee).sum();
        let filled_qty = quantity - remaining;
        let avg_price = if filled_qty > 0 { total_cost / filled_qty as f64 } else { 0.0 };

        // Calculate naive cost (single provider, first in list)
        let naive_cost = if let Some(first) = liquidity.first() {
            first.best_price * quantity as f64 * (1.0 + first.estimated_fee_rate)
        } else {
            total_cost + total_fees
        };

        let actual_total = total_cost + total_fees;
        let savings = (naive_cost - actual_total).max(0.0);
        let savings_pct = if naive_cost > 0.0 { (savings / naive_cost) * 100.0 } else { 0.0 };

        RoutingPlan {
            market_native_id: market_native_id.to_string(),
            outcome_id: outcome_id.to_string(),
            side: format!("{:?}", side).to_lowercase(),
            total_quantity: quantity,
            estimated_total_cost: actual_total,
            estimated_avg_price: avg_price,
            estimated_fees: total_fees,
            naive_cost,
            savings,
            savings_pct,
            providers_considered: liquidity.len(),
            legs,
            computed_at: Utc::now().to_rfc3339(),
        }
    }

    async fn direct_route(
        &self,
        registry: &ProviderRegistry,
        provider_id: &str,
        market_native_id: &str,
        outcome_id: &str,
        side: Side,
        quantity: i64,
    ) -> anyhow::Result<RoutingPlan> {
        let adapter = registry.get(provider_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_id))?;

        let snapshots = adapter.get_orderbook(market_native_id, Some(outcome_id), 10).await?;
        let liquidity = snapshots.iter()
            .find(|s| s.outcome_id == outcome_id)
            .map(|s| self.extract_liquidity(provider_id, s, side))
            .ok_or_else(|| anyhow::anyhow!("No orderbook data for outcome"))?;

        let cost = liquidity.best_price * quantity as f64;
        let fee = cost * liquidity.estimated_fee_rate;

        Ok(RoutingPlan {
            market_native_id: market_native_id.to_string(),
            outcome_id: outcome_id.to_string(),
            side: format!("{:?}", side).to_lowercase(),
            total_quantity: quantity,
            estimated_total_cost: cost + fee,
            estimated_avg_price: liquidity.best_price,
            estimated_fees: fee,
            naive_cost: cost + fee,
            savings: 0.0,
            savings_pct: 0.0,
            providers_considered: 1,
            legs: vec![RoutingLeg {
                provider: provider_id.to_string(),
                price: liquidity.best_price,
                quantity,
                estimated_cost: cost,
                estimated_fee: fee,
                fill_probability: if liquidity.available_quantity >= quantity { 1.0 }
                    else { liquidity.available_quantity as f64 / quantity as f64 },
                priority: 1,
            }],
            computed_at: Utc::now().to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    pub provider: String,
    pub quantity: i64,
    pub price: f64,
    pub status: String,
    pub order_id: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouterStats {
    pub routes_computed: u64,
    pub orders_routed: u64,
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_liquidity(provider: &str, levels: Vec<(f64, i64)>) -> ProviderLiquidity {
        let best = levels.iter().map(|(p, _)| *p).fold(f64::MAX, f64::min);
        let total: i64 = levels.iter().map(|(_, q)| *q).sum();
        ProviderLiquidity {
            provider_id: provider.to_string(),
            best_price: if best == f64::MAX { 0.0 } else { best },
            available_quantity: total,
            levels,
            estimated_fee_rate: 0.02,
        }
    }

    #[test]
    fn test_best_price_routing() {
        let router = SmartRouter::new(0.02);
        let liquidity = vec![
            make_liquidity("kalshi", vec![(0.55, 100)]),
            make_liquidity("polymarket", vec![(0.52, 80)]),
            make_liquidity("opinion", vec![(0.58, 50)]),
        ];

        let plan = router.route_best_price(&liquidity, "test-market", "yes", Side::Buy, 50);

        assert_eq!(plan.legs.len(), 1);
        assert_eq!(plan.legs[0].provider, "polymarket"); // cheapest
        assert_eq!(plan.legs[0].price, 0.52);
        assert_eq!(plan.legs[0].quantity, 50);
    }

    #[test]
    fn test_split_optimal_routing() {
        let router = SmartRouter::new(0.02);
        let liquidity = vec![
            make_liquidity("kalshi", vec![(0.55, 30), (0.56, 30)]),
            make_liquidity("polymarket", vec![(0.52, 20), (0.54, 40)]),
        ];

        let plan = router.route_split_optimal(&liquidity, "test-market", "yes", Side::Buy, 50);

        // Should fill 20 @ 0.52 from poly, 30 @ 0.54 from poly or 0.55 from kalshi
        let total_filled: i64 = plan.legs.iter().map(|l| l.quantity).sum();
        assert_eq!(total_filled, 50);
        assert!(plan.providers_considered == 2);
    }

    #[test]
    fn test_split_routes_to_cheapest_first() {
        let router = SmartRouter::new(0.02);
        let liquidity = vec![
            make_liquidity("expensive", vec![(0.70, 100)]),
            make_liquidity("cheap", vec![(0.40, 100)]),
        ];

        let plan = router.route_split_optimal(&liquidity, "m1", "yes", Side::Buy, 50);

        // All 50 should route to "cheap" since it has enough liquidity at best price
        assert_eq!(plan.legs.len(), 1);
        assert_eq!(plan.legs[0].provider, "cheap");
        assert_eq!(plan.legs[0].quantity, 50);
    }

    #[test]
    fn test_split_across_providers_when_insufficient_single() {
        let router = SmartRouter::new(0.02);
        let liquidity = vec![
            make_liquidity("a", vec![(0.50, 30)]),
            make_liquidity("b", vec![(0.51, 30)]),
        ];

        let plan = router.route_split_optimal(&liquidity, "m1", "yes", Side::Buy, 50);

        let total_filled: i64 = plan.legs.iter().map(|l| l.quantity).sum();
        assert_eq!(total_filled, 50);
        assert_eq!(plan.legs.len(), 2);
    }

    #[test]
    fn test_sell_side_routing() {
        let router = SmartRouter::new(0.02);
        // For selling, we want highest bid prices
        let liquidity = vec![
            ProviderLiquidity {
                provider_id: "high_bidder".into(),
                best_price: 0.65,
                available_quantity: 100,
                levels: vec![(0.65, 100)],
                estimated_fee_rate: 0.02,
            },
            ProviderLiquidity {
                provider_id: "low_bidder".into(),
                best_price: 0.55,
                available_quantity: 100,
                levels: vec![(0.55, 100)],
                estimated_fee_rate: 0.02,
            },
        ];

        let plan = router.route_best_price(&liquidity, "m1", "yes", Side::Sell, 50);
        assert_eq!(plan.legs[0].provider, "high_bidder");
    }

    #[test]
    fn test_savings_calculation() {
        let router = SmartRouter::new(0.02);
        let liquidity = vec![
            make_liquidity("expensive", vec![(0.60, 100)]),
            make_liquidity("cheap", vec![(0.50, 100)]),
        ];

        let plan = router.route_split_optimal(&liquidity, "m1", "yes", Side::Buy, 50);

        // Naive would be first provider (expensive): 50 * 0.60 * 1.02 = 30.6
        // Optimal routes to cheap: 50 * 0.50 * 1.02 = 25.5
        assert!(plan.savings > 0.0);
        assert!(plan.savings_pct > 0.0);
    }

    #[test]
    fn test_routing_strategy_parse() {
        assert_eq!(RoutingStrategy::parse("best_price"), Some(RoutingStrategy::BestPrice));
        assert_eq!(RoutingStrategy::parse("split"), Some(RoutingStrategy::SplitOptimal));
        assert_eq!(RoutingStrategy::parse("direct"), Some(RoutingStrategy::DirectRoute));
        assert_eq!(RoutingStrategy::parse("unknown"), None);
    }

    #[test]
    fn test_router_stats() {
        let router = SmartRouter::new(0.02);
        let stats = router.stats();
        assert_eq!(stats.routes_computed, 0);
        assert_eq!(stats.orders_routed, 0);
    }
}
