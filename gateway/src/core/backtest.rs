// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Backtesting Engine — simulate trading strategies against historical price data.
//
// Architecture:
//   - Feeds historical OHLCV candles from PriceIndex into a strategy evaluator
//   - Strategies implement the `Strategy` trait (signal generation + position sizing)
//   - Execution engine simulates fills with configurable slippage and fees
//   - Output: equity curve, trade log, performance metrics (Sharpe, max drawdown, etc.)
//
// Built-in strategies:
//   - MeanReversion: buy when price drops below moving average, sell above
//   - Momentum: follow price trends with breakout detection
//   - ArbitrageReplay: replay historical cross-provider spread opportunities
//   - ThresholdBand: buy below lower band, sell above upper band

use crate::core::price_index::{Candle, PriceIndex, Resolution};
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;

// ─── Strategy Interface ──────────────────────────────────────

/// Signal emitted by a strategy on each candle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Signal {
    /// Buy `quantity` contracts at market.
    Buy(i64),
    /// Sell `quantity` contracts at market.
    Sell(i64),
    /// Do nothing this period.
    Hold,
    /// Close any open position.
    ClosePosition,
}

/// A backtesting strategy. Receives candle history and emits trading signals.
pub trait Strategy: Send + Sync {
    /// Strategy display name.
    fn name(&self) -> &str;

    /// Generate a signal given the current candle index and full history up to that point.
    /// `candles[..=idx]` are available (look-back is bounded by `idx`).
    fn evaluate(&mut self, candles: &[Candle], idx: usize) -> Signal;

    /// Reset internal state for a new run.
    fn reset(&mut self);
}

// ─── Backtest Configuration ──────────────────────────────────

/// Configuration for a single backtest run.
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Starting capital in dollars.
    pub initial_capital: f64,
    /// Fee per trade as a fraction (e.g. 0.02 = 2%).
    pub fee_rate: f64,
    /// Slippage per trade as a fraction (e.g. 0.005 = 0.5%).
    pub slippage_rate: f64,
    /// Maximum position size in contracts.
    pub max_position: i64,
    /// Risk-free rate for Sharpe ratio calculation (annualized).
    pub risk_free_rate: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 10_000.0,
            fee_rate: 0.02,
            slippage_rate: 0.005,
            max_position: 1000,
            risk_free_rate: 0.05,
        }
    }
}

// ─── Backtest Engine ─────────────────────────────────────────

/// A single simulated trade in the backtest.
#[derive(Debug, Clone, Serialize)]
pub struct SimulatedTrade {
    pub idx: usize,
    pub timestamp: i64,
    pub side: String,
    pub price: f64,
    pub quantity: i64,
    pub gross_cost: f64,
    pub fee: f64,
    pub slippage_cost: f64,
    pub net_cost: f64,
    pub position_after: i64,
    pub equity_after: f64,
}

/// A single point on the equity curve.
#[derive(Debug, Clone, Serialize)]
pub struct EquityPoint {
    pub idx: usize,
    pub timestamp: i64,
    pub equity: f64,
    pub position: i64,
    pub unrealized_pnl: f64,
    pub drawdown_pct: f64,
}

/// Performance metrics from a completed backtest.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestMetrics {
    pub strategy_name: String,
    pub market_id: String,
    pub outcome_id: String,
    pub resolution: String,
    pub candles_evaluated: usize,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub initial_capital: f64,
    pub final_equity: f64,
    pub total_return: f64,
    pub total_return_pct: f64,
    pub total_fees_paid: f64,
    pub total_slippage_cost: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
    pub profit_factor: f64,
    pub avg_trade_pnl: f64,
    pub best_trade_pnl: f64,
    pub worst_trade_pnl: f64,
    pub max_position_held: i64,
    pub time_in_market_pct: f64,
}

/// Full backtest result including metrics, trade log, and equity curve.
#[derive(Debug, Clone, Serialize)]
pub struct BacktestResult {
    pub metrics: BacktestMetrics,
    pub trades: Vec<SimulatedTrade>,
    pub equity_curve: Vec<EquityPoint>,
    pub computed_at: String,
}

/// Run a backtest against historical candle data.
pub fn run_backtest(
    strategy: &mut dyn Strategy,
    candles: &[Candle],
    config: &BacktestConfig,
    market_id: &str,
    outcome_id: &str,
) -> BacktestResult {
    strategy.reset();

    let mut cash = config.initial_capital;
    let mut position: i64 = 0;
    let mut trades: Vec<SimulatedTrade> = Vec::new();
    let mut equity_curve: Vec<EquityPoint> = Vec::new();
    let mut peak_equity = config.initial_capital;
    let mut max_drawdown_pct = 0.0_f64;
    let mut periods_in_market: usize = 0;
    let mut max_position_held: i64 = 0;

    for idx in 0..candles.len() {
        let candle = &candles[idx];
        let signal = strategy.evaluate(candles, idx);

        // Execute signal
        match signal {
            Signal::Buy(qty) => {
                let qty = qty.min(config.max_position - position).max(0);
                if qty > 0 {
                    let exec_price = candle.close * (1.0 + config.slippage_rate);
                    let gross = exec_price * qty as f64;
                    let fee = gross * config.fee_rate;
                    let slippage = candle.close * config.slippage_rate * qty as f64;

                    cash -= gross + fee;
                    position += qty;

                    let equity = cash + position as f64 * candle.close;
                    trades.push(SimulatedTrade {
                        idx,
                        timestamp: candle.timestamp,
                        side: "buy".to_string(),
                        price: exec_price,
                        quantity: qty,
                        gross_cost: gross,
                        fee,
                        slippage_cost: slippage,
                        net_cost: gross + fee,
                        position_after: position,
                        equity_after: equity,
                    });
                }
            }
            Signal::Sell(qty) => {
                let qty = qty.min(position).max(0);
                if qty > 0 {
                    let exec_price = candle.close * (1.0 - config.slippage_rate);
                    let gross = exec_price * qty as f64;
                    let fee = gross * config.fee_rate;
                    let slippage = candle.close * config.slippage_rate * qty as f64;

                    cash += gross - fee;
                    position -= qty;

                    let equity = cash + position as f64 * candle.close;
                    trades.push(SimulatedTrade {
                        idx,
                        timestamp: candle.timestamp,
                        side: "sell".to_string(),
                        price: exec_price,
                        quantity: qty,
                        gross_cost: gross,
                        fee,
                        slippage_cost: slippage,
                        net_cost: gross - fee,
                        position_after: position,
                        equity_after: equity,
                    });
                }
            }
            Signal::ClosePosition => {
                if position > 0 {
                    let exec_price = candle.close * (1.0 - config.slippage_rate);
                    let gross = exec_price * position as f64;
                    let fee = gross * config.fee_rate;
                    let slippage = candle.close * config.slippage_rate * position as f64;
                    let qty = position;

                    cash += gross - fee;
                    position = 0;

                    trades.push(SimulatedTrade {
                        idx,
                        timestamp: candle.timestamp,
                        side: "sell".to_string(),
                        price: exec_price,
                        quantity: qty,
                        gross_cost: gross,
                        fee,
                        slippage_cost: slippage,
                        net_cost: gross - fee,
                        position_after: 0,
                        equity_after: cash,
                    });
                }
            }
            Signal::Hold => {}
        }

        // Track metrics
        if position != 0 { periods_in_market += 1; }
        if position.abs() > max_position_held { max_position_held = position.abs(); }

        // Equity point
        let unrealized = position as f64 * candle.close;
        let equity = cash + unrealized;
        if equity > peak_equity { peak_equity = equity; }
        let drawdown = if peak_equity > 0.0 { ((peak_equity - equity) / peak_equity) * 100.0 } else { 0.0 };
        if drawdown > max_drawdown_pct { max_drawdown_pct = drawdown; }

        equity_curve.push(EquityPoint {
            idx,
            timestamp: candle.timestamp,
            equity,
            position,
            unrealized_pnl: unrealized - (position as f64 * candles[0].close), // rough
            drawdown_pct: drawdown,
        });
    }

    // Compute final metrics
    let final_equity = cash + position as f64 * candles.last().map(|c| c.close).unwrap_or(0.0);
    let total_return = final_equity - config.initial_capital;
    let total_return_pct = if config.initial_capital > 0.0 {
        (total_return / config.initial_capital) * 100.0
    } else { 0.0 };

    let total_fees: f64 = trades.iter().map(|t| t.fee).sum();
    let total_slippage: f64 = trades.iter().map(|t| t.slippage_cost).sum();

    // Win/loss analysis
    let trade_pnls = compute_trade_pnls(&trades);
    let winning = trade_pnls.iter().filter(|&&p| p > 0.0).count();
    let losing = trade_pnls.iter().filter(|&&p| p < 0.0).count();
    let win_rate = if !trade_pnls.is_empty() {
        (winning as f64 / trade_pnls.len() as f64) * 100.0
    } else { 0.0 };

    let gross_profit: f64 = trade_pnls.iter().filter(|&&p| p > 0.0).sum();
    let gross_loss: f64 = trade_pnls.iter().filter(|&&p| p < 0.0).map(|p| p.abs()).sum();
    let profit_factor = if gross_loss > 0.0 { gross_profit / gross_loss } else { f64::INFINITY };

    let avg_pnl = if !trade_pnls.is_empty() {
        trade_pnls.iter().sum::<f64>() / trade_pnls.len() as f64
    } else { 0.0 };
    let best_pnl = trade_pnls.iter().cloned().fold(0.0_f64, f64::max);
    let worst_pnl = trade_pnls.iter().cloned().fold(0.0_f64, f64::min);

    // Sharpe ratio (annualized, using daily returns approximation)
    let sharpe = compute_sharpe(&equity_curve, config.risk_free_rate, candles.first().map(|c| c.period_seconds).unwrap_or(60));

    let time_in_market = if !candles.is_empty() {
        (periods_in_market as f64 / candles.len() as f64) * 100.0
    } else { 0.0 };

    let resolution_str = candles.first()
        .map(|c| match c.period_seconds {
            60 => "1m", 300 => "5m", 3600 => "1h", 86400 => "1d", _ => "unknown"
        })
        .unwrap_or("unknown");

    BacktestResult {
        metrics: BacktestMetrics {
            strategy_name: strategy.name().to_string(),
            market_id: market_id.to_string(),
            outcome_id: outcome_id.to_string(),
            resolution: resolution_str.to_string(),
            candles_evaluated: candles.len(),
            total_trades: trades.len(),
            winning_trades: winning,
            losing_trades: losing,
            win_rate,
            initial_capital: config.initial_capital,
            final_equity,
            total_return,
            total_return_pct,
            total_fees_paid: total_fees,
            total_slippage_cost: total_slippage,
            max_drawdown_pct,
            sharpe_ratio: sharpe,
            profit_factor,
            avg_trade_pnl: avg_pnl,
            best_trade_pnl: best_pnl,
            worst_trade_pnl: worst_pnl,
            max_position_held,
            time_in_market_pct: time_in_market,
        },
        trades,
        equity_curve,
        computed_at: Utc::now().to_rfc3339(),
    }
}

/// Run a backtest using data from the PriceIndex.
pub fn run_backtest_from_index(
    strategy: &mut dyn Strategy,
    price_index: &PriceIndex,
    market_id: &str,
    outcome_id: &str,
    resolution: Resolution,
    config: &BacktestConfig,
) -> Option<BacktestResult> {
    let candles = price_index.query_candles(market_id, outcome_id, resolution, None, None, 10_000);
    if candles.len() < 2 {
        return None;
    }
    Some(run_backtest(strategy, &candles, config, market_id, outcome_id))
}

// ─── Built-in Strategies ────────────────────────────────────

/// Mean Reversion — buy when price drops below SMA, sell when above.
pub struct MeanReversionStrategy {
    window: usize,
    buy_threshold: f64,   // buy when price < SMA * (1 - threshold)
    sell_threshold: f64,  // sell when price > SMA * (1 + threshold)
    position_size: i64,
}

impl MeanReversionStrategy {
    pub fn new(window: usize, buy_threshold: f64, sell_threshold: f64, position_size: i64) -> Self {
        Self { window, buy_threshold, sell_threshold, position_size }
    }
}

impl Strategy for MeanReversionStrategy {
    fn name(&self) -> &str { "MeanReversion" }

    fn evaluate(&mut self, candles: &[Candle], idx: usize) -> Signal {
        if idx < self.window { return Signal::Hold; }

        let sma: f64 = candles[idx + 1 - self.window..=idx]
            .iter()
            .map(|c| c.close)
            .sum::<f64>() / self.window as f64;

        let price = candles[idx].close;
        let lower = sma * (1.0 - self.buy_threshold);
        let upper = sma * (1.0 + self.sell_threshold);

        if price < lower {
            Signal::Buy(self.position_size)
        } else if price > upper {
            Signal::Sell(self.position_size)
        } else {
            Signal::Hold
        }
    }

    fn reset(&mut self) {}
}

/// Momentum — buy on breakout above N-period high, sell on breakdown below N-period low.
pub struct MomentumStrategy {
    lookback: usize,
    position_size: i64,
    in_position: bool,
}

impl MomentumStrategy {
    pub fn new(lookback: usize, position_size: i64) -> Self {
        Self { lookback, position_size, in_position: false }
    }
}

impl Strategy for MomentumStrategy {
    fn name(&self) -> &str { "Momentum" }

    fn evaluate(&mut self, candles: &[Candle], idx: usize) -> Signal {
        if idx < self.lookback { return Signal::Hold; }

        let window = &candles[idx - self.lookback..idx];
        let period_high = window.iter().map(|c| c.high).fold(0.0_f64, f64::max);
        let period_low = window.iter().map(|c| c.low).fold(f64::MAX, f64::min);
        let price = candles[idx].close;

        if !self.in_position && price > period_high {
            self.in_position = true;
            Signal::Buy(self.position_size)
        } else if self.in_position && price < period_low {
            self.in_position = false;
            Signal::ClosePosition
        } else {
            Signal::Hold
        }
    }

    fn reset(&mut self) {
        self.in_position = false;
    }
}

/// Threshold Band — buy below lower threshold, sell above upper threshold.
/// Simple range-trading strategy for markets with clear probability bands.
pub struct ThresholdBandStrategy {
    lower: f64,
    upper: f64,
    position_size: i64,
    in_position: bool,
}

impl ThresholdBandStrategy {
    pub fn new(lower: f64, upper: f64, position_size: i64) -> Self {
        Self { lower, upper, position_size, in_position: false }
    }
}

impl Strategy for ThresholdBandStrategy {
    fn name(&self) -> &str { "ThresholdBand" }

    fn evaluate(&mut self, candles: &[Candle], idx: usize) -> Signal {
        let price = candles[idx].close;

        if !self.in_position && price < self.lower {
            self.in_position = true;
            Signal::Buy(self.position_size)
        } else if self.in_position && price > self.upper {
            self.in_position = false;
            Signal::ClosePosition
        } else {
            Signal::Hold
        }
    }

    fn reset(&mut self) {
        self.in_position = false;
    }
}

/// Arbitrage Replay — given two market candle series, buy the cheaper
/// and sell the more expensive when the spread exceeds a threshold.
pub struct ArbitrageReplayStrategy {
    /// Second market's candles (the "comparison" market).
    comparison_candles: Vec<Candle>,
    spread_threshold: f64,
    position_size: i64,
    in_position: bool,
}

impl ArbitrageReplayStrategy {
    pub fn new(comparison_candles: Vec<Candle>, spread_threshold: f64, position_size: i64) -> Self {
        Self { comparison_candles, spread_threshold, position_size, in_position: false }
    }
}

impl Strategy for ArbitrageReplayStrategy {
    fn name(&self) -> &str { "ArbitrageReplay" }

    fn evaluate(&mut self, candles: &[Candle], idx: usize) -> Signal {
        if idx >= self.comparison_candles.len() { return Signal::Hold; }

        let price_a = candles[idx].close;
        let price_b = self.comparison_candles[idx].close;
        let spread = (price_a - price_b).abs();

        if !self.in_position && spread > self.spread_threshold {
            self.in_position = true;
            if price_a < price_b {
                Signal::Buy(self.position_size) // buy cheaper market
            } else {
                Signal::Hold // would sell other market — not modeled here
            }
        } else if self.in_position && spread < self.spread_threshold * 0.3 {
            // Spread converged — close position
            self.in_position = false;
            Signal::ClosePosition
        } else {
            Signal::Hold
        }
    }

    fn reset(&mut self) {
        self.in_position = false;
    }
}

// ─── Strategy Factory ────────────────────────────────────────

/// Parse a strategy name and parameters into a boxed Strategy.
pub fn create_strategy(name: &str, params: &HashMap<String, f64>) -> Option<Box<dyn Strategy>> {
    match name.to_lowercase().as_str() {
        "mean_reversion" | "meanreversion" => {
            let window = params.get("window").copied().unwrap_or(20.0) as usize;
            let buy_thresh = params.get("buy_threshold").copied().unwrap_or(0.05);
            let sell_thresh = params.get("sell_threshold").copied().unwrap_or(0.05);
            let size = params.get("position_size").copied().unwrap_or(10.0) as i64;
            Some(Box::new(MeanReversionStrategy::new(window, buy_thresh, sell_thresh, size)))
        }
        "momentum" => {
            let lookback = params.get("lookback").copied().unwrap_or(10.0) as usize;
            let size = params.get("position_size").copied().unwrap_or(10.0) as i64;
            Some(Box::new(MomentumStrategy::new(lookback, size)))
        }
        "threshold_band" | "thresholdband" | "band" => {
            let lower = params.get("lower").copied().unwrap_or(0.30);
            let upper = params.get("upper").copied().unwrap_or(0.70);
            let size = params.get("position_size").copied().unwrap_or(10.0) as i64;
            Some(Box::new(ThresholdBandStrategy::new(lower, upper, size)))
        }
        _ => None,
    }
}

/// List available built-in strategies with their parameter descriptions.
pub fn available_strategies() -> Vec<StrategyInfo> {
    vec![
        StrategyInfo {
            name: "mean_reversion".to_string(),
            description: "Buy when price drops below SMA, sell when above. Classic mean-reversion.".to_string(),
            parameters: vec![
                ParamInfo { name: "window".into(), description: "SMA lookback period".into(), default: 20.0 },
                ParamInfo { name: "buy_threshold".into(), description: "Buy when price < SMA * (1 - threshold)".into(), default: 0.05 },
                ParamInfo { name: "sell_threshold".into(), description: "Sell when price > SMA * (1 + threshold)".into(), default: 0.05 },
                ParamInfo { name: "position_size".into(), description: "Contracts per trade".into(), default: 10.0 },
            ],
        },
        StrategyInfo {
            name: "momentum".to_string(),
            description: "Buy on breakout above N-period high, sell on breakdown below N-period low.".to_string(),
            parameters: vec![
                ParamInfo { name: "lookback".into(), description: "Lookback period for high/low".into(), default: 10.0 },
                ParamInfo { name: "position_size".into(), description: "Contracts per trade".into(), default: 10.0 },
            ],
        },
        StrategyInfo {
            name: "threshold_band".to_string(),
            description: "Buy below lower threshold, sell above upper. Range-trading for prediction markets.".to_string(),
            parameters: vec![
                ParamInfo { name: "lower".into(), description: "Lower price threshold (probability)".into(), default: 0.30 },
                ParamInfo { name: "upper".into(), description: "Upper price threshold (probability)".into(), default: 0.70 },
                ParamInfo { name: "position_size".into(), description: "Contracts per trade".into(), default: 10.0 },
            ],
        },
    ]
}

#[derive(Debug, Clone, Serialize)]
pub struct StrategyInfo {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParamInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParamInfo {
    pub name: String,
    pub description: String,
    pub default: f64,
}

// ─── Helpers ─────────────────────────────────────────────────

/// Compute PnL for each round-trip trade (buy→sell pairs).
fn compute_trade_pnls(trades: &[SimulatedTrade]) -> Vec<f64> {
    let mut pnls = Vec::new();
    let mut i = 0;

    while i < trades.len() {
        if trades[i].side == "buy" {
            // Look for matching sell
            if i + 1 < trades.len() && trades[i + 1].side == "sell" {
                let buy_cost = trades[i].net_cost;
                let sell_proceeds = trades[i + 1].net_cost;
                pnls.push(sell_proceeds - buy_cost);
                i += 2;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    pnls
}

/// Compute annualized Sharpe ratio from the equity curve.
fn compute_sharpe(curve: &[EquityPoint], risk_free_rate: f64, period_seconds: u64) -> f64 {
    if curve.len() < 2 { return 0.0; }

    let returns: Vec<f64> = curve.windows(2)
        .map(|w| {
            if w[0].equity > 0.0 {
                (w[1].equity - w[0].equity) / w[0].equity
            } else {
                0.0
            }
        })
        .collect();

    if returns.is_empty() { return 0.0; }

    let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
    let variance = returns.iter()
        .map(|r| (r - mean_return).powi(2))
        .sum::<f64>() / returns.len() as f64;
    let std_dev = variance.sqrt();

    if std_dev == 0.0 { return 0.0; }

    // Annualization factor: how many periods per year
    let periods_per_year = (365.25 * 86400.0) / period_seconds as f64;
    let annualized_return = mean_return * periods_per_year;
    let annualized_std = std_dev * periods_per_year.sqrt();

    (annualized_return - risk_free_rate) / annualized_std
}

// ─── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candles(prices: &[f64], start_ts: i64, period: u64) -> Vec<Candle> {
        prices.iter().enumerate().map(|(i, &p)| {
            Candle {
                open: p,
                high: p + 0.02,
                low: p - 0.02,
                close: p,
                volume: 100,
                timestamp: start_ts + (i as i64 * period as i64),
                period_seconds: period,
            }
        }).collect()
    }

    #[test]
    fn test_threshold_band_basic() {
        let candles = make_candles(&[0.50, 0.45, 0.25, 0.30, 0.60, 0.75, 0.80], 1000, 60);
        let mut strategy = ThresholdBandStrategy::new(0.30, 0.70, 10);
        let config = BacktestConfig::default();

        let result = run_backtest(&mut strategy, &candles, &config, "test", "yes");

        assert_eq!(result.metrics.candles_evaluated, 7);
        assert!(result.metrics.total_trades >= 2, "Should have buy+sell, got {} trades", result.metrics.total_trades);
        assert_eq!(result.equity_curve.len(), 7);
    }

    #[test]
    fn test_mean_reversion_generates_trades() {
        // Price drops below SMA then recovers
        let mut prices = vec![0.50; 25]; // 25 candles at 0.50 to build SMA
        prices.extend_from_slice(&[0.40, 0.38, 0.35, 0.40, 0.50, 0.55, 0.58, 0.60]); // drop then recover
        let candles = make_candles(&prices, 1000, 60);

        let mut strategy = MeanReversionStrategy::new(20, 0.05, 0.05, 10);
        let config = BacktestConfig::default();
        let result = run_backtest(&mut strategy, &candles, &config, "test", "yes");

        assert!(result.metrics.total_trades > 0, "Should generate at least one trade");
    }

    #[test]
    fn test_momentum_breakout() {
        // Sideways then breakout up
        let mut prices: Vec<f64> = (0..15).map(|i| 0.50 + (i as f64 * 0.001)).collect();
        prices.extend_from_slice(&[0.60, 0.65, 0.70, 0.72]); // breakout
        let candles = make_candles(&prices, 1000, 60);

        let mut strategy = MomentumStrategy::new(10, 10);
        let config = BacktestConfig::default();
        let result = run_backtest(&mut strategy, &candles, &config, "test", "yes");

        assert!(result.metrics.total_trades >= 1, "Should detect breakout");
    }

    #[test]
    fn test_hold_generates_no_trades() {
        struct HoldStrategy;
        impl Strategy for HoldStrategy {
            fn name(&self) -> &str { "Hold" }
            fn evaluate(&mut self, _: &[Candle], _: usize) -> Signal { Signal::Hold }
            fn reset(&mut self) {}
        }

        let candles = make_candles(&[0.50, 0.55, 0.60], 1000, 60);
        let config = BacktestConfig::default();
        let result = run_backtest(&mut HoldStrategy, &candles, &config, "test", "yes");

        assert_eq!(result.metrics.total_trades, 0);
        assert_eq!(result.metrics.final_equity, config.initial_capital);
    }

    #[test]
    fn test_fees_and_slippage_reduce_equity() {
        let candles = make_candles(&[0.50, 0.50, 0.50], 1000, 60);

        struct BuyAndSell;
        impl Strategy for BuyAndSell {
            fn name(&self) -> &str { "BuyAndSell" }
            fn evaluate(&mut self, _: &[Candle], idx: usize) -> Signal {
                match idx {
                    0 => Signal::Buy(10),
                    1 => Signal::Sell(10),
                    _ => Signal::Hold,
                }
            }
            fn reset(&mut self) {}
        }

        let config = BacktestConfig {
            initial_capital: 1000.0,
            fee_rate: 0.02,
            slippage_rate: 0.005,
            max_position: 100,
            risk_free_rate: 0.0,
        };

        let result = run_backtest(&mut BuyAndSell, &candles, &config, "test", "yes");

        assert!(result.metrics.final_equity < 1000.0,
            "Fees + slippage should reduce equity, got {}", result.metrics.final_equity);
        assert!(result.metrics.total_fees_paid > 0.0);
    }

    #[test]
    fn test_max_position_enforcement() {
        let candles = make_candles(&[0.20, 0.20, 0.20, 0.20], 1000, 60);

        struct GreedyBuyer;
        impl Strategy for GreedyBuyer {
            fn name(&self) -> &str { "GreedyBuyer" }
            fn evaluate(&mut self, _: &[Candle], _: usize) -> Signal { Signal::Buy(99999) }
            fn reset(&mut self) {}
        }

        let config = BacktestConfig {
            max_position: 50,
            ..BacktestConfig::default()
        };

        let result = run_backtest(&mut GreedyBuyer, &candles, &config, "test", "yes");

        assert!(result.metrics.max_position_held <= 50,
            "Position should be capped at 50, got {}", result.metrics.max_position_held);
    }

    #[test]
    fn test_drawdown_calculation() {
        // Price goes up then crashes
        let candles = make_candles(&[0.50, 0.60, 0.70, 0.80, 0.40, 0.30], 1000, 60);

        struct BuyAndHold;
        impl Strategy for BuyAndHold {
            fn name(&self) -> &str { "BuyAndHold" }
            fn evaluate(&mut self, _: &[Candle], idx: usize) -> Signal {
                if idx == 0 { Signal::Buy(100) } else { Signal::Hold }
            }
            fn reset(&mut self) {}
        }

        let config = BacktestConfig::default();
        let result = run_backtest(&mut BuyAndHold, &candles, &config, "test", "yes");

        assert!(result.metrics.max_drawdown_pct > 0.0,
            "Should have drawdown from peak, got {}", result.metrics.max_drawdown_pct);
    }

    #[test]
    fn test_create_strategy_factory() {
        let mut params = HashMap::new();
        params.insert("window".to_string(), 15.0);

        let s = create_strategy("mean_reversion", &params);
        assert!(s.is_some());
        assert_eq!(s.unwrap().name(), "MeanReversion");

        let s = create_strategy("momentum", &HashMap::new());
        assert!(s.is_some());

        let s = create_strategy("threshold_band", &HashMap::new());
        assert!(s.is_some());

        let s = create_strategy("nonexistent", &HashMap::new());
        assert!(s.is_none());
    }

    #[test]
    fn test_available_strategies_list() {
        let strategies = available_strategies();
        assert_eq!(strategies.len(), 3);
        assert!(strategies.iter().any(|s| s.name == "mean_reversion"));
        assert!(strategies.iter().any(|s| s.name == "momentum"));
        assert!(strategies.iter().any(|s| s.name == "threshold_band"));
    }

    #[test]
    fn test_sharpe_ratio() {
        // Constant positive returns should give high Sharpe
        let curve: Vec<EquityPoint> = (0..100).map(|i| EquityPoint {
            idx: i,
            timestamp: 1000 + i as i64 * 60,
            equity: 10000.0 + i as f64 * 10.0,
            position: 10,
            unrealized_pnl: i as f64 * 10.0,
            drawdown_pct: 0.0,
        }).collect();

        let sharpe = compute_sharpe(&curve, 0.05, 60);
        assert!(sharpe > 0.0, "Positive consistent returns should have positive Sharpe, got {}", sharpe);
    }

    #[test]
    fn test_empty_candles() {
        let mut strategy = ThresholdBandStrategy::new(0.30, 0.70, 10);
        let result = run_backtest(&mut strategy, &[], &BacktestConfig::default(), "test", "yes");
        assert_eq!(result.metrics.total_trades, 0);
        assert_eq!(result.metrics.candles_evaluated, 0);
    }
}
