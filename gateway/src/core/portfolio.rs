// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Portfolio Analytics Engine — real-time P&L tracking, risk scoring,
// exposure heatmaps, and concentration analysis across all providers.

use crate::core::types::*;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;

// ─── Analytics Types ────────────────────────────────────────

/// Full portfolio analytics snapshot.
#[derive(Debug, Clone, Serialize)]
pub struct PortfolioAnalytics {
    pub total_value: f64,
    pub total_cost_basis: f64,
    pub total_unrealized_pnl: f64,
    pub total_realized_pnl: f64,
    pub total_pnl: f64,
    pub return_pct: f64,
    pub position_count: usize,
    pub open_position_count: usize,
    pub win_rate: f64,
    pub risk_score: RiskScore,
    pub exposure: ExposureAnalysis,
    pub provider_breakdown: Vec<ProviderBreakdown>,
    pub category_breakdown: Vec<CategoryBreakdown>,
    pub top_winners: Vec<PositionPnl>,
    pub top_losers: Vec<PositionPnl>,
    pub computed_at: DateTime<Utc>,
}

/// Risk score with component breakdown.
#[derive(Debug, Clone, Serialize)]
pub struct RiskScore {
    pub overall: f64,       // 0-100, higher = riskier
    pub concentration: f64, // single-position concentration risk
    pub provider: f64,      // single-provider concentration risk
    pub category: f64,      // single-category concentration risk
    pub liquidity: f64,     // illiquidity risk (positions in low-volume markets)
    pub correlation: f64,   // correlated positions risk
    pub label: String,      // "low", "moderate", "high", "critical"
}

/// Exposure analysis across dimensions.
#[derive(Debug, Clone, Serialize)]
pub struct ExposureAnalysis {
    pub total_exposure: f64,
    pub net_exposure: f64,     // long - short
    pub long_exposure: f64,
    pub short_exposure: f64,
    pub max_single_position_pct: f64,
    pub provider_heatmap: Vec<HeatmapEntry>,
    pub category_heatmap: Vec<HeatmapEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeatmapEntry {
    pub label: String,
    pub value: f64,
    pub percentage: f64,
    pub position_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderBreakdown {
    pub provider: String,
    pub value: f64,
    pub pnl: f64,
    pub position_count: usize,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryBreakdown {
    pub category: String,
    pub value: f64,
    pub pnl: f64,
    pub position_count: usize,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionPnl {
    pub market_id: String,
    pub market_title: String,
    pub provider: String,
    pub pnl: f64,
    pub pnl_pct: f64,
    pub current_value: f64,
}

// ─── Analytics Engine ───────────────────────────────────────

/// Compute full portfolio analytics from positions across all providers.
pub fn compute_analytics(
    positions: &[Position],
    trades: &[Trade],
    markets: &HashMap<String, Market>,
) -> PortfolioAnalytics {
    let now = Utc::now();

    if positions.is_empty() {
        return PortfolioAnalytics {
            total_value: 0.0,
            total_cost_basis: 0.0,
            total_unrealized_pnl: 0.0,
            total_realized_pnl: 0.0,
            total_pnl: 0.0,
            return_pct: 0.0,
            position_count: 0,
            open_position_count: 0,
            win_rate: compute_win_rate(trades),
            risk_score: RiskScore {
                overall: 0.0,
                concentration: 0.0,
                provider: 0.0,
                category: 0.0,
                liquidity: 0.0,
                correlation: 0.0,
                label: "none".to_string(),
            },
            exposure: ExposureAnalysis {
                total_exposure: 0.0,
                net_exposure: 0.0,
                long_exposure: 0.0,
                short_exposure: 0.0,
                max_single_position_pct: 0.0,
                provider_heatmap: vec![],
                category_heatmap: vec![],
            },
            provider_breakdown: vec![],
            category_breakdown: vec![],
            top_winners: vec![],
            top_losers: vec![],
            computed_at: now,
        };
    }

    // ── Basic P&L aggregation ──
    let total_value: f64 = positions.iter()
        .map(|p| p.current_value.parse::<f64>().unwrap_or(0.0))
        .sum();
    let total_cost_basis: f64 = positions.iter()
        .map(|p| p.cost_basis.parse::<f64>().unwrap_or(0.0))
        .sum();
    let total_unrealized_pnl: f64 = positions.iter()
        .map(|p| p.unrealized_pnl.parse::<f64>().unwrap_or(0.0))
        .sum();
    let total_realized_pnl: f64 = positions.iter()
        .map(|p| p.realized_pnl.parse::<f64>().unwrap_or(0.0))
        .sum();
    let total_pnl = total_unrealized_pnl + total_realized_pnl;
    let return_pct = if total_cost_basis > 0.0 {
        (total_pnl / total_cost_basis) * 100.0
    } else {
        0.0
    };

    let open_positions: Vec<&Position> = positions.iter()
        .filter(|p| p.status == PositionStatus::Open)
        .collect();
    let open_position_count = open_positions.len();

    // ── Win rate from realized trades ──
    let win_rate = compute_win_rate(trades);

    // ── Exposure analysis ──
    let exposure = compute_exposure(positions, markets, total_value);

    // ── Provider breakdown ──
    let provider_breakdown = compute_provider_breakdown(positions, total_value);

    // ── Category breakdown ──
    let category_breakdown = compute_category_breakdown(positions, markets, total_value);

    // ── Risk scoring ──
    let risk_score = compute_risk_score(&exposure, &provider_breakdown, &category_breakdown);

    // ── Top winners / losers ──
    let (top_winners, top_losers) = compute_top_positions(positions, 5);

    PortfolioAnalytics {
        total_value,
        total_cost_basis,
        total_unrealized_pnl,
        total_realized_pnl,
        total_pnl,
        return_pct,
        position_count: positions.len(),
        open_position_count,
        win_rate,
        risk_score,
        exposure,
        provider_breakdown,
        category_breakdown,
        top_winners,
        top_losers,
        computed_at: now,
    }
}

fn compute_win_rate(trades: &[Trade]) -> f64 {
    if trades.is_empty() {
        return 0.0;
    }

    // Group trades by order — a "win" is when sell price > buy price for that market
    let mut market_pnl: HashMap<String, f64> = HashMap::new();
    for trade in trades {
        let key = format!("{}:{}", trade.market_id, trade.outcome_id);
        let price = trade.price.parse::<f64>().unwrap_or(0.0);
        let qty = trade.quantity as f64;
        let signed_notional = match trade.side {
            Side::Buy => -(price * qty),
            Side::Sell => price * qty,
        };
        *market_pnl.entry(key).or_insert(0.0) += signed_notional;
    }

    let total = market_pnl.len() as f64;
    let wins = market_pnl.values().filter(|&&v| v > 0.0).count() as f64;
    if total > 0.0 { (wins / total) * 100.0 } else { 0.0 }
}

fn compute_exposure(
    positions: &[Position],
    _markets: &HashMap<String, Market>,
    total_value: f64,
) -> ExposureAnalysis {
    let mut long_exposure = 0.0_f64;
    let mut short_exposure = 0.0_f64;
    let mut max_single = 0.0_f64;
    let mut provider_map: HashMap<String, (f64, usize)> = HashMap::new();
    let mut category_map: HashMap<String, (f64, usize)> = HashMap::new();

    for pos in positions {
        let value = pos.current_value.parse::<f64>().unwrap_or(0.0).abs();
        let qty = pos.quantity;

        if qty >= 0 {
            long_exposure += value;
        } else {
            short_exposure += value;
        }

        if total_value > 0.0 {
            let pct = (value / total_value) * 100.0;
            if pct > max_single {
                max_single = pct;
            }
        }

        let entry = provider_map.entry(pos.market_id.provider.clone()).or_insert((0.0, 0));
        entry.0 += value;
        entry.1 += 1;

        // Use market title as a proxy for category if market data not available
        let category = pos.market_title.split_whitespace()
            .next()
            .unwrap_or("Unknown")
            .to_string();
        let cat_entry = category_map.entry(category).or_insert((0.0, 0));
        cat_entry.0 += value;
        cat_entry.1 += 1;
    }

    let total_exposure = long_exposure + short_exposure;

    let provider_heatmap: Vec<HeatmapEntry> = provider_map.into_iter()
        .map(|(label, (value, count))| HeatmapEntry {
            percentage: if total_value > 0.0 { (value / total_value) * 100.0 } else { 0.0 },
            label,
            value,
            position_count: count,
        })
        .collect();

    let category_heatmap: Vec<HeatmapEntry> = category_map.into_iter()
        .map(|(label, (value, count))| HeatmapEntry {
            percentage: if total_value > 0.0 { (value / total_value) * 100.0 } else { 0.0 },
            label,
            value,
            position_count: count,
        })
        .collect();

    ExposureAnalysis {
        total_exposure,
        net_exposure: long_exposure - short_exposure,
        long_exposure,
        short_exposure,
        max_single_position_pct: max_single,
        provider_heatmap,
        category_heatmap,
    }
}

fn compute_provider_breakdown(positions: &[Position], total_value: f64) -> Vec<ProviderBreakdown> {
    let mut map: HashMap<String, (f64, f64, usize)> = HashMap::new();
    for pos in positions {
        let value = pos.current_value.parse::<f64>().unwrap_or(0.0);
        let pnl = pos.unrealized_pnl.parse::<f64>().unwrap_or(0.0)
            + pos.realized_pnl.parse::<f64>().unwrap_or(0.0);
        let entry = map.entry(pos.market_id.provider.clone()).or_insert((0.0, 0.0, 0));
        entry.0 += value;
        entry.1 += pnl;
        entry.2 += 1;
    }

    let mut result: Vec<ProviderBreakdown> = map.into_iter()
        .map(|(provider, (value, pnl, count))| ProviderBreakdown {
            percentage: if total_value > 0.0 { (value / total_value) * 100.0 } else { 0.0 },
            provider,
            value,
            pnl,
            position_count: count,
        })
        .collect();

    result.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    result
}

fn compute_category_breakdown(
    positions: &[Position],
    markets: &HashMap<String, Market>,
    total_value: f64,
) -> Vec<CategoryBreakdown> {
    let mut map: HashMap<String, (f64, f64, usize)> = HashMap::new();
    for pos in positions {
        let value = pos.current_value.parse::<f64>().unwrap_or(0.0);
        let pnl = pos.unrealized_pnl.parse::<f64>().unwrap_or(0.0)
            + pos.realized_pnl.parse::<f64>().unwrap_or(0.0);

        let category = markets.get(&pos.market_id.to_full_id())
            .map(|m| m.event.category.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let entry = map.entry(category).or_insert((0.0, 0.0, 0));
        entry.0 += value;
        entry.1 += pnl;
        entry.2 += 1;
    }

    let mut result: Vec<CategoryBreakdown> = map.into_iter()
        .map(|(category, (value, pnl, count))| CategoryBreakdown {
            percentage: if total_value > 0.0 { (value / total_value) * 100.0 } else { 0.0 },
            category,
            value,
            pnl,
            position_count: count,
        })
        .collect();

    result.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    result
}

fn compute_risk_score(
    exposure: &ExposureAnalysis,
    providers: &[ProviderBreakdown],
    categories: &[CategoryBreakdown],
) -> RiskScore {
    // Concentration risk — max single position as % of portfolio
    let concentration = (exposure.max_single_position_pct * 1.5).min(100.0);

    // Provider risk — max single provider as % of portfolio
    let provider_risk = providers.iter()
        .map(|p| p.percentage)
        .fold(0.0_f64, f64::max);
    let provider = (provider_risk * 1.2).min(100.0);

    // Category risk — max single category concentration
    let category_risk = categories.iter()
        .map(|c| c.percentage)
        .fold(0.0_f64, f64::max);
    let category = (category_risk * 1.0).min(100.0);

    // Liquidity risk placeholder (would need volume data in production)
    let liquidity = 20.0;

    // Correlation risk — if all positions in one provider + category
    let effective_providers = providers.iter().filter(|p| p.percentage > 5.0).count();
    let effective_categories = categories.iter().filter(|c| c.percentage > 5.0).count();
    let correlation = if effective_providers <= 1 && effective_categories <= 1 {
        80.0
    } else if effective_providers <= 2 || effective_categories <= 2 {
        50.0
    } else {
        20.0
    };

    let overall = (concentration * 0.25 + provider * 0.20 + category * 0.20
        + liquidity * 0.15 + correlation * 0.20).min(100.0);

    let label = match overall {
        x if x < 25.0 => "low",
        x if x < 50.0 => "moderate",
        x if x < 75.0 => "high",
        _ => "critical",
    }.to_string();

    RiskScore {
        overall,
        concentration,
        provider,
        category,
        liquidity,
        correlation,
        label,
    }
}

fn compute_top_positions(positions: &[Position], n: usize) -> (Vec<PositionPnl>, Vec<PositionPnl>) {
    let mut pnls: Vec<PositionPnl> = positions.iter()
        .map(|p| {
            let pnl = p.unrealized_pnl.parse::<f64>().unwrap_or(0.0)
                + p.realized_pnl.parse::<f64>().unwrap_or(0.0);
            let cost = p.cost_basis.parse::<f64>().unwrap_or(0.0);
            PositionPnl {
                market_id: p.market_id.to_full_id(),
                market_title: p.market_title.clone(),
                provider: p.market_id.provider.clone(),
                pnl,
                pnl_pct: if cost > 0.0 { (pnl / cost) * 100.0 } else { 0.0 },
                current_value: p.current_value.parse::<f64>().unwrap_or(0.0),
            }
        })
        .collect();

    pnls.sort_by(|a, b| b.pnl.partial_cmp(&a.pnl).unwrap_or(std::cmp::Ordering::Equal));

    let winners: Vec<PositionPnl> = pnls.iter()
        .filter(|p| p.pnl > 0.0)
        .take(n)
        .cloned()
        .collect();

    let losers: Vec<PositionPnl> = pnls.iter()
        .rev()
        .filter(|p| p.pnl < 0.0)
        .take(n)
        .cloned()
        .collect();

    (winners, losers)
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_position(provider: &str, value: f64, cost: f64, unrealized: f64, qty: i64) -> Position {
        Position {
            market_id: UniversalMarketId::new(provider, "test-market"),
            outcome_id: "yes".to_string(),
            quantity: qty,
            average_entry_price: format!("{:.2}", cost / qty.abs() as f64),
            current_price: format!("{:.2}", value / qty.abs() as f64),
            cost_basis: format!("{:.2}", cost),
            current_value: format!("{:.2}", value),
            unrealized_pnl: format!("{:.2}", unrealized),
            realized_pnl: "0.00".to_string(),
            status: PositionStatus::Open,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            market_title: "Test Market".to_string(),
            market_status: MarketStatus::Open,
        }
    }

    #[test]
    fn test_empty_portfolio() {
        let analytics = compute_analytics(&[], &[], &HashMap::new());
        assert_eq!(analytics.total_value, 0.0);
        assert_eq!(analytics.position_count, 0);
        assert_eq!(analytics.risk_score.label, "none");
    }

    #[test]
    fn test_basic_pnl() {
        let positions = vec![
            make_position("kalshi", 150.0, 100.0, 50.0, 10),
            make_position("polymarket", 80.0, 100.0, -20.0, 5),
        ];
        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert_eq!(analytics.total_value, 230.0);
        assert_eq!(analytics.total_cost_basis, 200.0);
        assert_eq!(analytics.total_unrealized_pnl, 30.0);
        assert_eq!(analytics.position_count, 2);
        assert!(analytics.return_pct > 14.0 && analytics.return_pct < 16.0);
    }

    #[test]
    fn test_provider_breakdown() {
        let positions = vec![
            make_position("kalshi", 200.0, 150.0, 50.0, 10),
            make_position("kalshi", 100.0, 80.0, 20.0, 5),
            make_position("polymarket", 50.0, 60.0, -10.0, 3),
        ];
        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert_eq!(analytics.provider_breakdown.len(), 2);
        // Kalshi should be first (higher value)
        assert_eq!(analytics.provider_breakdown[0].provider, "kalshi");
        assert_eq!(analytics.provider_breakdown[0].position_count, 2);
    }

    #[test]
    fn test_risk_score_concentrated() {
        // All in one provider = higher risk
        let positions = vec![
            make_position("kalshi", 900.0, 800.0, 100.0, 10),
            make_position("kalshi", 100.0, 90.0, 10.0, 5),
        ];
        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert!(analytics.risk_score.provider > 50.0, "Single-provider should be high risk");
        assert!(analytics.risk_score.overall > 30.0, "Overall risk should be elevated");
    }

    #[test]
    fn test_exposure_long_short() {
        let positions = vec![
            make_position("kalshi", 100.0, 80.0, 20.0, 10),   // long
            make_position("polymarket", 50.0, 60.0, -10.0, -5), // short
        ];
        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert_eq!(analytics.exposure.long_exposure, 100.0);
        assert_eq!(analytics.exposure.short_exposure, 50.0);
        assert_eq!(analytics.exposure.net_exposure, 50.0);
    }

    #[test]
    fn test_top_winners_losers() {
        let positions = vec![
            make_position("kalshi", 150.0, 100.0, 50.0, 10),
            make_position("polymarket", 60.0, 100.0, -40.0, 5),
            make_position("opinion", 120.0, 100.0, 20.0, 8),
        ];
        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert_eq!(analytics.top_winners.len(), 2);
        assert!(analytics.top_winners[0].pnl > analytics.top_winners[1].pnl);
        assert_eq!(analytics.top_losers.len(), 1);
        assert!(analytics.top_losers[0].pnl < 0.0);
    }

    #[test]
    fn test_win_rate() {
        let trades = vec![
            Trade {
                id: "t1".into(),
                order_id: "o1".into(),
                market_id: UniversalMarketId::new("kalshi", "m1"),
                outcome_id: "yes".into(),
                side: Side::Buy,
                price: "0.40".into(),
                quantity: 10,
                notional: "4.00".into(),
                role: TradeRole::Taker,
                fees: OrderFees::default(),
                executed_at: Utc::now(),
            },
            Trade {
                id: "t2".into(),
                order_id: "o2".into(),
                market_id: UniversalMarketId::new("kalshi", "m1"),
                outcome_id: "yes".into(),
                side: Side::Sell,
                price: "0.60".into(),
                quantity: 10,
                notional: "6.00".into(),
                role: TradeRole::Taker,
                fees: OrderFees::default(),
                executed_at: Utc::now(),
            },
            Trade {
                id: "t3".into(),
                order_id: "o3".into(),
                market_id: UniversalMarketId::new("poly", "m2"),
                outcome_id: "yes".into(),
                side: Side::Buy,
                price: "0.70".into(),
                quantity: 5,
                notional: "3.50".into(),
                role: TradeRole::Taker,
                fees: OrderFees::default(),
                executed_at: Utc::now(),
            },
        ];

        let rate = compute_win_rate(&trades);
        // m1: -4.0 + 6.0 = +2.0 (win), m2: -3.5 (loss) → 50%
        assert!(rate > 49.0 && rate < 51.0);
    }
}
