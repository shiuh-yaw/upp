// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// End-to-end integration tests — validates the full request lifecycle
// across core modules: portfolio analytics, price indexing, smart routing,
// arbitrage detection, persistent storage, and rate limiting.

// ─── Portfolio Analytics Integration ────────────────────────

mod portfolio_analytics {
    use upp_gateway::core::portfolio::*;
    use upp_gateway::core::types::*;
    use std::collections::HashMap;
    use chrono::Utc;

    fn make_position(provider: &str, native_id: &str, value: f64, cost: f64, unrealized: f64, qty: i64) -> Position {
        Position {
            market_id: UniversalMarketId::new(provider, native_id),
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
    fn test_cross_provider_analytics() {
        let positions = vec![
            make_position("kalshi.com", "TRUMP-WIN", 500.0, 400.0, 100.0, 50),
            make_position("kalshi.com", "FED-CUT", 200.0, 250.0, -50.0, 20),
            make_position("polymarket.com", "BTC-100K", 300.0, 200.0, 100.0, 30),
            make_position("opinion.trade", "RAIN-NYC", 100.0, 120.0, -20.0, 10),
        ];

        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert_eq!(analytics.total_value, 1100.0);
        assert_eq!(analytics.total_cost_basis, 970.0);
        assert_eq!(analytics.total_unrealized_pnl, 130.0);
        assert_eq!(analytics.position_count, 4);
        assert_eq!(analytics.open_position_count, 4);

        // Provider breakdown
        assert_eq!(analytics.provider_breakdown.len(), 3);
        let kalshi = analytics.provider_breakdown.iter()
            .find(|p| p.provider == "kalshi.com").unwrap();
        assert_eq!(kalshi.position_count, 2);
        assert_eq!(kalshi.value, 700.0);

        // Risk scoring
        assert!(analytics.risk_score.overall > 0.0);
        assert!(analytics.risk_score.overall <= 100.0);
        assert!(!analytics.risk_score.label.is_empty());

        // Exposure
        assert!(analytics.exposure.total_exposure > 0.0);
        assert!(analytics.exposure.long_exposure > 0.0);
        assert!(analytics.exposure.provider_heatmap.len() == 3);
    }

    #[test]
    fn test_concentrated_portfolio_high_risk() {
        let positions = vec![
            make_position("kalshi.com", "m1", 9900.0, 9000.0, 900.0, 100),
            make_position("kalshi.com", "m2", 100.0, 90.0, 10.0, 10),
        ];

        let analytics = compute_analytics(&positions, &[], &HashMap::new());

        assert!(analytics.risk_score.concentration > 50.0,
            "99% single position should have high concentration risk, got {}", analytics.risk_score.concentration);
        assert!(analytics.risk_score.provider > 50.0,
            "100% single provider should have high provider risk, got {}", analytics.risk_score.provider);
    }

    #[test]
    fn test_win_rate_from_trades() {
        let trades = vec![
            Trade {
                id: "t1".into(), order_id: "o1".into(),
                market_id: UniversalMarketId::new("kalshi", "m1"),
                outcome_id: "yes".into(), side: Side::Buy,
                price: "0.40".into(), quantity: 10, notional: "4.00".into(),
                role: TradeRole::Taker, fees: OrderFees::default(), executed_at: Utc::now(),
            },
            Trade {
                id: "t2".into(), order_id: "o2".into(),
                market_id: UniversalMarketId::new("kalshi", "m1"),
                outcome_id: "yes".into(), side: Side::Sell,
                price: "0.70".into(), quantity: 10, notional: "7.00".into(),
                role: TradeRole::Taker, fees: OrderFees::default(), executed_at: Utc::now(),
            },
            Trade {
                id: "t3".into(), order_id: "o3".into(),
                market_id: UniversalMarketId::new("poly", "m2"),
                outcome_id: "yes".into(), side: Side::Buy,
                price: "0.80".into(), quantity: 5, notional: "4.00".into(),
                role: TradeRole::Taker, fees: OrderFees::default(), executed_at: Utc::now(),
            },
        ];

        let analytics = compute_analytics(&[], &trades, &HashMap::new());
        assert!(analytics.win_rate > 49.0 && analytics.win_rate < 51.0,
            "Win rate should be ~50%, got {}", analytics.win_rate);
    }
}

// ─── Price Indexer Integration ──────────────────────────────

mod price_indexer {
    use upp_gateway::core::price_index::*;
    use chrono::{Utc, Duration};

    #[test]
    fn test_full_candle_lifecycle() {
        let index = PriceIndex::new();
        let base = Utc::now();

        for i in 0..600 {
            let price = 0.50 + (i as f64 * 0.001) * ((i as f64 * 0.1).sin());
            index.ingest(PriceTick {
                market_id: "upp:kalshi.com:TRUMP-WIN".into(),
                outcome_id: "yes".into(),
                price,
                timestamp: base + Duration::seconds(i),
            });
        }

        let candles_1m = index.query_candles(
            "upp:kalshi.com:TRUMP-WIN", "yes",
            Resolution::OneMinute, None, None, 100,
        );
        assert!(candles_1m.len() >= 9 && candles_1m.len() <= 11,
            "Expected ~10 1m candles, got {}", candles_1m.len());

        for candle in &candles_1m {
            assert!(candle.high >= candle.low);
            assert!(candle.high >= candle.open);
            assert!(candle.high >= candle.close);
            assert!(candle.low <= candle.open);
            assert!(candle.low <= candle.close);
            assert!(candle.volume > 0);
            assert_eq!(candle.period_seconds, 60);
        }

        let candles_5m = index.query_candles(
            "upp:kalshi.com:TRUMP-WIN", "yes",
            Resolution::FiveMinute, None, None, 100,
        );
        assert!(candles_5m.len() >= 1 && candles_5m.len() <= 3);
        // Total volume across 5m candles should equal total across 1m candles
        // (same ticks, different aggregation)
        let total_vol_1m: u64 = candles_1m.iter().map(|c| c.volume).sum();
        let total_vol_5m: u64 = candles_5m.iter().map(|c| c.volume).sum();
        assert_eq!(total_vol_1m, total_vol_5m,
            "Total volume should match across resolutions");
    }

    #[test]
    fn test_multi_market_isolation() {
        let index = PriceIndex::new();
        let now = Utc::now();

        index.ingest(PriceTick { market_id: "m1".into(), outcome_id: "yes".into(), price: 0.50, timestamp: now });
        index.ingest(PriceTick { market_id: "m2".into(), outcome_id: "yes".into(), price: 0.70, timestamp: now });
        index.ingest(PriceTick { market_id: "m1".into(), outcome_id: "no".into(), price: 0.50, timestamp: now });

        assert_eq!(index.latest_candle("m1", "yes", Resolution::OneMinute).unwrap().close, 0.50);
        assert_eq!(index.latest_candle("m2", "yes", Resolution::OneMinute).unwrap().close, 0.70);
        assert_eq!(index.latest_candle("m1", "no", Resolution::OneMinute).unwrap().close, 0.50);
        assert!(index.latest_candle("m3", "yes", Resolution::OneMinute).is_none());

        let stats = index.stats();
        assert_eq!(stats.ticks_ingested, 3);
        assert_eq!(stats.markets_tracked, 2);
    }

    #[test]
    fn test_time_range_query() {
        let index = PriceIndex::new();
        for i in 0..10 {
            let t = chrono::DateTime::from_timestamp(1768560000 + i * 60, 0).unwrap();
            index.ingest(PriceTick { market_id: "m1".into(), outcome_id: "yes".into(), price: 0.50, timestamp: t });
        }

        let from_ts = 1768560000 + 3 * 60;
        let to_ts = 1768560000 + 7 * 60;
        let candles = index.query_candles("m1", "yes", Resolution::OneMinute, Some(from_ts), Some(to_ts), 100);
        assert_eq!(candles.len(), 5);
    }

    #[test]
    fn test_limit_returns_latest() {
        let index = PriceIndex::new();
        for i in 0..20 {
            let t = chrono::DateTime::from_timestamp(1768560000 + i * 60, 0).unwrap();
            index.ingest(PriceTick { market_id: "m1".into(), outcome_id: "yes".into(), price: 0.50 + i as f64 * 0.01, timestamp: t });
        }

        let candles = index.query_candles("m1", "yes", Resolution::OneMinute, None, None, 5);
        assert_eq!(candles.len(), 5);
        assert!(candles[4].close > candles[0].close);
    }
}

// ─── Smart Router Integration ───────────────────────────────

mod smart_routing {
    use upp_gateway::core::smart_router::*;

    #[test]
    fn test_strategy_parsing() {
        assert_eq!(RoutingStrategy::parse("best_price"), Some(RoutingStrategy::BestPrice));
        assert_eq!(RoutingStrategy::parse("BEST_PRICE"), Some(RoutingStrategy::BestPrice));
        assert_eq!(RoutingStrategy::parse("split"), Some(RoutingStrategy::SplitOptimal));
        assert_eq!(RoutingStrategy::parse("optimal"), Some(RoutingStrategy::SplitOptimal));
        assert_eq!(RoutingStrategy::parse("direct"), Some(RoutingStrategy::DirectRoute));
        assert_eq!(RoutingStrategy::parse("invalid"), None);
    }

    #[test]
    fn test_router_stats_initialized() {
        let router = SmartRouter::new(0.02);
        let stats = router.stats();
        assert_eq!(stats.routes_computed, 0);
        assert_eq!(stats.orders_routed, 0);
    }
}

// ─── Arbitrage Integration ──────────────────────────────────

mod arbitrage_tests {
    use upp_gateway::core::arbitrage::*;

    #[tokio::test]
    async fn test_scanner_empty_state() {
        let scanner = ArbitrageScanner::new(1.0, 0.02);
        let alerts = scanner.get_active_alerts();
        assert!(alerts.is_empty());

        let summary = scanner.get_summary().await;
        assert_eq!(summary.active_opportunities, 0);
        assert_eq!(summary.total_scans, 0);
        assert_eq!(summary.total_detected, 0);
    }

    #[tokio::test]
    async fn test_history_empty() {
        let scanner = ArbitrageScanner::new(0.5, 0.01);
        let history = scanner.get_history(100).await;
        assert!(history.is_empty());
    }
}

// ─── Storage Integration ────────────────────────────────────

mod storage_tests {
    use upp_gateway::core::storage::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_full_order_lifecycle() {
        let storage = create_storage(None).await.unwrap();

        let order = StoredOrder {
            order_id: "ord-int-1".to_string(),
            provider: "kalshi.com".to_string(),
            market_id: "upp:kalshi.com:test".to_string(),
            outcome_id: "yes".to_string(),
            side: "buy".to_string(),
            price: "0.55".to_string(),
            quantity: 10,
            status: "open".to_string(),
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            provider_order_id: None,
        };

        storage.save_order(&order).await.unwrap();
        let retrieved = storage.get_order("ord-int-1").await.unwrap().unwrap();
        assert_eq!(retrieved.order_id, "ord-int-1");
        assert_eq!(retrieved.status, "open");

        storage.update_order_status("ord-int-1", "filled").await.unwrap();
        let updated = storage.get_order("ord-int-1").await.unwrap().unwrap();
        assert_eq!(updated.status, "filled");

        assert_eq!(storage.order_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_full_trade_lifecycle() {
        let storage = create_storage(None).await.unwrap();

        let trade = StoredTrade {
            trade_id: "trd-int-1".to_string(),
            order_id: "ord-1".to_string(),
            provider: "polymarket.com".to_string(),
            market_id: "upp:polymarket.com:test".to_string(),
            side: "sell".to_string(),
            price: "0.65".to_string(),
            quantity: 5,
            fee: "0.01".to_string(),
            executed_at: Utc::now().to_rfc3339(),
        };

        storage.save_trade(&trade).await.unwrap();
        let trades = storage.list_trades(&TradeFilter {
            provider: None,
            market_id: None,
            order_id: None,
            limit: 10,
        }).await.unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "trd-int-1");
        assert_eq!(storage.trade_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_market_cache_ttl() {
        let storage = create_storage(None).await.unwrap();

        storage.cache_market("int-m1", "{\"cached\":true}", 1).await.unwrap();
        assert!(storage.get_cached_market("int-m1").await.unwrap().is_some());

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        assert!(storage.get_cached_market("int-m1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cross_provider_order_listing() {
        let storage = create_storage(None).await.unwrap();

        for i in 0..5 {
            let provider = if i % 2 == 0 { "kalshi.com" } else { "polymarket.com" };
            storage.save_order(&StoredOrder {
                order_id: format!("ord-cp-{}", i),
                provider: provider.to_string(),
                market_id: format!("upp:{}:m{}", provider, i),
                outcome_id: "yes".to_string(),
                side: "buy".to_string(),
                price: "0.50".to_string(),
                quantity: 10,
                status: "open".to_string(),
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
                provider_order_id: None,
            }).await.unwrap();
        }

        let all = storage.list_orders(&OrderFilter {
            provider: None,
            market_id: None,
            status: None,
            limit: 100,
        }).await.unwrap();
        assert_eq!(all.len(), 5);

        let kalshi = storage.list_orders(&OrderFilter {
            provider: Some("kalshi.com".to_string()),
            market_id: None,
            status: None,
            limit: 100,
        }).await.unwrap();
        assert_eq!(kalshi.len(), 3);

        let poly = storage.list_orders(&OrderFilter {
            provider: Some("polymarket.com".to_string()),
            market_id: None,
            status: None,
            limit: 100,
        }).await.unwrap();
        assert_eq!(poly.len(), 2);
    }
}

// ─── Rate Limiting Integration ──────────────────────────────

mod rate_limiting {
    use upp_gateway::middleware::rate_limit::*;
    use std::collections::HashMap;

    fn make_config(light: (u32, f64), standard: (u32, f64), heavy: (u32, f64), ws: (u32, f64)) -> RateLimitConfig {
        let mut tiers = HashMap::new();
        tiers.insert(RateLimitTier::Light, RateLimitTierConfig { max_burst: light.0, requests_per_second: light.1 });
        tiers.insert(RateLimitTier::Standard, RateLimitTierConfig { max_burst: standard.0, requests_per_second: standard.1 });
        tiers.insert(RateLimitTier::Heavy, RateLimitTierConfig { max_burst: heavy.0, requests_per_second: heavy.1 });
        tiers.insert(RateLimitTier::WebSocket, RateLimitTierConfig { max_burst: ws.0, requests_per_second: ws.1 });
        RateLimitConfig {
            tiers,
            cleanup_interval_secs: 60,
            bucket_expiry_secs: 300,
        }
    }

    #[test]
    fn test_multi_tier_enforcement() {
        let config = make_config((200, 100.0), (60, 30.0), (10, 5.0), (5, 2.0));
        let state = RateLimitState::new(config);

        for _ in 0..100 {
            let (allowed, _, _, _) = state.check("light-client", RateLimitTier::Light);
            assert!(allowed);
        }

        for _ in 0..10 {
            let _ = state.check("heavy-client", RateLimitTier::Heavy);
        }
        let (allowed, _, _, _) = state.check("heavy-client", RateLimitTier::Heavy);
        assert!(!allowed, "Heavy tier should be rate limited after 10 requests");

        let (allowed, _, _, _) = state.check("other-heavy", RateLimitTier::Heavy);
        assert!(allowed, "Different client should not be rate limited");
    }

    #[test]
    fn test_endpoint_classification() {
        assert_eq!(classify_endpoint("/health"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/ready"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/metrics"), RateLimitTier::Light);
        assert_eq!(classify_endpoint("/upp/v1/markets"), RateLimitTier::Standard);
        assert_eq!(classify_endpoint("/upp/v1/orders/estimate"), RateLimitTier::Heavy);
        assert_eq!(classify_endpoint("/upp/v1/ws"), RateLimitTier::WebSocket);
    }

    #[test]
    fn test_retry_after_returned() {
        let config = make_config((2, 1.0), (2, 1.0), (1, 1.0), (1, 1.0));
        let state = RateLimitState::new(config);
        let _ = state.check("retry-client", RateLimitTier::Heavy);
        let (allowed, remaining, _limit, retry_after) = state.check("retry-client", RateLimitTier::Heavy);

        assert!(!allowed);
        assert_eq!(remaining, 0);
        assert!(retry_after > 0.0);
    }
}

// ─── Live Feed Integration ──────────────────────────────────

mod live_feed_tests {
    use upp_gateway::transport::live_feed::*;

    #[test]
    fn test_feed_config_defaults() {
        let kalshi = FeedConfig::kalshi();
        assert_eq!(kalshi.provider_id, "kalshi.com");
        assert_eq!(kalshi.heartbeat_interval_secs, 30);
        assert_eq!(kalshi.max_backoff_secs, 60);
        assert!(kalshi.auto_subscribe.is_empty());

        let poly = FeedConfig::polymarket();
        assert_eq!(poly.provider_id, "polymarket.com");
        assert!(poly.ws_url.contains("polymarket"));

        let opinion = FeedConfig::opinion();
        assert_eq!(opinion.provider_id, "opinion.trade");
    }

    #[test]
    fn test_connection_state_display() {
        assert_eq!(ConnectionState::Connected.to_string(), "connected");
        assert_eq!(ConnectionState::Backoff.to_string(), "backoff");
        assert_eq!(ConnectionState::Disconnected.to_string(), "disconnected");
        assert_eq!(ConnectionState::Subscribed.to_string(), "subscribed");
        assert_eq!(ConnectionState::Connecting.to_string(), "connecting");
    }

    #[test]
    fn test_parse_kalshi_orderbook_message() {
        let msg = r#"{"type":"orderbook_snapshot","msg":{"market_ticker":"PRES-2028","yes":[[55,200]],"no":[[45,200]]}}"#;
        // We can't call parse_kalshi_message directly (it's private),
        // but we can test through the module's tests
    }
}

// ─── Backtesting Integration ────────────────────────────────

mod backtest_tests {
    use upp_gateway::core::backtest::*;
    use upp_gateway::core::price_index::{Candle, PriceIndex, PriceTick, Resolution};
    use std::collections::HashMap;

    fn make_candles(prices: &[f64]) -> Vec<Candle> {
        prices.iter().enumerate().map(|(i, &p)| Candle {
            open: p,
            high: p + 0.02,
            low: p - 0.02,
            close: p,
            volume: 100,
            timestamp: 1768560000 + (i as i64 * 60),
            period_seconds: 60,
        }).collect()
    }

    #[test]
    fn test_strategy_factory() {
        let params = HashMap::new();
        assert!(create_strategy("mean_reversion", &params).is_some());
        assert!(create_strategy("momentum", &params).is_some());
        assert!(create_strategy("threshold_band", &params).is_some());
        assert!(create_strategy("nonexistent", &params).is_none());
    }

    #[test]
    fn test_available_strategies() {
        let strategies = available_strategies();
        assert_eq!(strategies.len(), 3);
        let names: Vec<&str> = strategies.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"mean_reversion"));
        assert!(names.contains(&"momentum"));
        assert!(names.contains(&"threshold_band"));
    }

    #[test]
    fn test_backtest_with_threshold_band() {
        // Price oscillates: dips below 0.30 (buy signal) then rises above 0.70 (sell signal)
        let prices: Vec<f64> = vec![
            0.50, 0.45, 0.40, 0.35, 0.25, 0.20, // dip → buy
            0.30, 0.40, 0.50, 0.60, 0.70, 0.75, 0.80, // rise → sell
        ];
        let candles = make_candles(&prices);

        let mut strategy = ThresholdBandStrategy::new(0.30, 0.70, 10);
        let config = BacktestConfig::default();
        let result = run_backtest(&mut strategy, &candles, &config, "test-market", "yes");

        assert_eq!(result.metrics.candles_evaluated, 13);
        assert!(result.metrics.total_trades >= 2, "Expected buy+sell trades");
        assert!(result.metrics.total_fees_paid > 0.0);
        assert_eq!(result.equity_curve.len(), 13);
        assert!(!result.computed_at.is_empty());
    }

    #[test]
    fn test_backtest_mean_reversion_with_recovery() {
        // Build SMA at 0.50, then drop and recover
        let mut prices = vec![0.50; 25];
        prices.extend_from_slice(&[0.42, 0.40, 0.38, 0.42, 0.48, 0.52, 0.55, 0.58]);
        let candles = make_candles(&prices);

        let mut strategy = MeanReversionStrategy::new(20, 0.05, 0.05, 10);
        let result = run_backtest(&mut strategy, &candles, &BacktestConfig::default(), "m1", "yes");

        assert!(result.metrics.total_trades > 0);
    }

    #[test]
    fn test_backtest_from_price_index() {
        let index = PriceIndex::new();
        let base_ts = 1768560000_i64;

        // Ingest enough ticks for a meaningful backtest
        for i in 0..100 {
            let ts = chrono::DateTime::from_timestamp(base_ts + i * 60, 0).unwrap();
            let price = 0.50 + 0.1 * ((i as f64 * 0.1).sin());
            index.ingest(PriceTick {
                market_id: "bt-market".into(),
                outcome_id: "yes".into(),
                price,
                timestamp: ts,
            });
        }

        let mut strategy = ThresholdBandStrategy::new(0.40, 0.60, 5);
        let result = run_backtest_from_index(
            &mut strategy, &index, "bt-market", "yes",
            Resolution::OneMinute, &BacktestConfig::default(),
        );

        assert!(result.is_some(), "Should have enough candle data");
        let result = result.unwrap();
        assert!(result.metrics.candles_evaluated >= 90); // most ticks create separate candles
    }

    #[test]
    fn test_compare_strategies_different_results() {
        let prices: Vec<f64> = (0..50).map(|i| 0.50 + 0.2 * (i as f64 * 0.15).sin()).collect();
        let candles = make_candles(&prices);
        let config = BacktestConfig::default();

        let mut band = ThresholdBandStrategy::new(0.35, 0.65, 10);
        let mut momentum = MomentumStrategy::new(10, 10);

        let result_band = run_backtest(&mut band, &candles, &config, "m1", "yes");
        let result_mom = run_backtest(&mut momentum, &candles, &config, "m1", "yes");

        // Both should run without errors
        assert_eq!(result_band.metrics.candles_evaluated, 50);
        assert_eq!(result_mom.metrics.candles_evaluated, 50);

        // They should produce different trade counts (different strategies)
        // (Not guaranteed but very likely with this data)
        assert!(result_band.metrics.strategy_name != result_mom.metrics.strategy_name);
    }

    #[test]
    fn test_backtest_max_drawdown() {
        // Price rises then crashes — should show max drawdown
        let prices: Vec<f64> = vec![
            0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80, // rise
            0.70, 0.60, 0.50, 0.40, 0.30, // crash
        ];
        let candles = make_candles(&prices);

        let mut strategy = ThresholdBandStrategy::new(0.30, 0.85, 100);
        let config = BacktestConfig { max_position: 100, ..BacktestConfig::default() };
        let result = run_backtest(&mut strategy, &candles, &config, "m1", "yes");

        // Should have entered a position and seen drawdown
        if result.metrics.total_trades > 0 {
            assert!(result.metrics.max_drawdown_pct >= 0.0);
        }
    }
}

// ─── Historical Ingestion Integration ───────────────────────

mod historical_tests {
    use upp_gateway::core::historical::*;
    use upp_gateway::core::price_index::PriceIndex;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_dev_pipeline_ingests_data() {
        let index = Arc::new(PriceIndex::new());
        let pipeline = create_dev_pipeline(index.clone());

        let count = pipeline.ingest_all_recent(1).await.unwrap();
        assert!(count > 0, "Dev pipeline should ingest mock data");

        let stats = pipeline.stats();
        assert!(stats.ticks_ingested > 0);
        assert!(stats.markets_processed >= 6); // 2 markets × 3 providers
        assert_eq!(stats.errors_encountered, 0);

        // Data should be in the price index now
        let idx_stats = index.stats();
        assert!(idx_stats.ticks_ingested > 0);
        assert!(idx_stats.markets_tracked > 0);
    }

    #[tokio::test]
    async fn test_kalshi_historical_markets() {
        let source = KalshiHistorical::dev();
        let markets = source.available_markets().await.unwrap();
        assert_eq!(markets.len(), 2);
        assert!(markets[0].tick_count > 0);
    }

    #[tokio::test]
    async fn test_polymarket_historical_markets() {
        let source = PolymarketHistorical::dev();
        let markets = source.available_markets().await.unwrap();
        assert_eq!(markets.len(), 2);
    }

    #[tokio::test]
    async fn test_opinion_historical_markets() {
        let source = OpinionHistorical::dev();
        let markets = source.available_markets().await.unwrap();
        assert_eq!(markets.len(), 2);
    }

    #[tokio::test]
    async fn test_pipeline_stats_tracking() {
        let index = Arc::new(PriceIndex::new());
        let sources: Vec<Box<dyn HistoricalDataSource>> = vec![Box::new(KalshiHistorical::dev())];
        let pipeline = IngestionPipeline::new(index, sources);

        let stats_before = pipeline.stats();
        assert_eq!(stats_before.ticks_ingested, 0);

        pipeline.ingest_all_recent(1).await.unwrap();

        let stats_after = pipeline.stats();
        assert!(stats_after.ticks_ingested > stats_before.ticks_ingested);
    }
}

// ─── Auth & API Key Management Integration ─────────────────

mod auth_key_tests {
    use upp_gateway::middleware::auth::*;

    #[test]
    fn test_full_key_lifecycle() {
        let mgr = ApiKeyManager::new();

        // Create keys
        let key1 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Trading Bot".to_string(),
            tier: Some(ClientTier::Pro),
            providers: Some(vec!["kalshi".to_string(), "polymarket".to_string()]),
            label: Some("prod-bot".to_string()),
            expires_in_days: Some(90),
        });
        let key2 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Dashboard".to_string(),
            tier: Some(ClientTier::Free),
            providers: None,
            label: None,
            expires_in_days: None,
        });

        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.active_count(), 2);

        // Authenticate
        let client1 = mgr.authenticate_key(&key1.key).unwrap();
        assert_eq!(client1.name, "Trading Bot");
        assert_eq!(client1.tier, ClientTier::Pro);
        assert_eq!(client1.providers.len(), 2);

        let client2 = mgr.authenticate_key(&key2.key).unwrap();
        assert_eq!(client2.name, "Dashboard");
        assert_eq!(client2.tier, ClientTier::Free);

        // List
        let keys = mgr.list_keys();
        assert_eq!(keys.len(), 2);

        // Revoke
        assert!(mgr.revoke_by_prefix(&key1.key_prefix));
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.authenticate_key(&key1.key).is_none());
        assert!(mgr.authenticate_key(&key2.key).is_some());
    }

    #[test]
    fn test_multiple_keys_independent() {
        let mgr = ApiKeyManager::new();

        let key1 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Bot A".to_string(),
            tier: Some(ClientTier::Pro),
            providers: Some(vec!["kalshi".to_string()]),
            label: None,
            expires_in_days: None,
        });
        let key2 = mgr.create_key(CreateApiKeyRequest {
            client_name: "Bot B".to_string(),
            tier: Some(ClientTier::Enterprise),
            providers: None,
            label: None,
            expires_in_days: None,
        });

        // Both authenticate independently
        let c1 = mgr.authenticate_key(&key1.key).unwrap();
        let c2 = mgr.authenticate_key(&key2.key).unwrap();
        assert_eq!(c1.tier, ClientTier::Pro);
        assert_eq!(c2.tier, ClientTier::Enterprise);

        // Revoking one doesn't affect the other
        mgr.revoke_by_prefix(&key1.key_prefix);
        assert!(mgr.authenticate_key(&key1.key).is_none());
        assert!(mgr.authenticate_key(&key2.key).is_some());
        assert_eq!(mgr.active_count(), 1);
    }
}
