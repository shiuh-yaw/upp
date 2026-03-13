// Gateway configuration — loaded from environment variables and config files.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,

    // Provider credentials
    pub kalshi_api_key_id: Option<String>,
    pub kalshi_private_key_path: Option<String>,
    pub polymarket_wallet_key: Option<String>,
    pub opinion_api_key: Option<String>,

    // Cache settings
    #[serde(default = "default_market_cache_ttl")]
    pub market_cache_ttl_seconds: u64,
    #[serde(default = "default_orderbook_cache_ttl")]
    pub orderbook_cache_ttl_ms: u64,

    // Redis (for multi-node state sharing)
    pub redis_url: Option<String>,

    // Rate limiting — Light tier (health, metrics, .well-known)
    #[serde(default = "default_rate_limit_light_burst")]
    pub rate_limit_light_burst: u32,
    #[serde(default = "default_rate_limit_light_rps")]
    pub rate_limit_light_rps: f64,

    // Rate limiting — Standard tier (markets, search, discovery)
    #[serde(default = "default_rate_limit_standard_burst")]
    pub rate_limit_standard_burst: u32,
    #[serde(default = "default_rate_limit_standard_rps")]
    pub rate_limit_standard_rps: f64,

    // Rate limiting — Heavy tier (compute-intensive operations)
    #[serde(default = "default_rate_limit_heavy_burst")]
    pub rate_limit_heavy_burst: u32,
    #[serde(default = "default_rate_limit_heavy_rps")]
    pub rate_limit_heavy_rps: f64,

    // Rate limiting — WebSocket tier
    #[serde(default = "default_rate_limit_ws_burst")]
    pub rate_limit_ws_burst: u32,
    #[serde(default = "default_rate_limit_ws_rps")]
    pub rate_limit_ws_rps: f64,
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_market_cache_ttl() -> u64 { 300 }     // 5 minutes
fn default_orderbook_cache_ttl() -> u64 { 500 }  // 500ms

// Rate limit tier defaults
fn default_rate_limit_light_burst() -> u32 { 200 }
fn default_rate_limit_light_rps() -> f64 { 100.0 }
fn default_rate_limit_standard_burst() -> u32 { 50 }
fn default_rate_limit_standard_rps() -> f64 { 20.0 }
fn default_rate_limit_heavy_burst() -> u32 { 20 }
fn default_rate_limit_heavy_rps() -> f64 { 5.0 }
fn default_rate_limit_ws_burst() -> u32 { 10 }
fn default_rate_limit_ws_rps() -> f64 { 2.0 }

impl GatewayConfig {
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv(); // Load .env if present

        let config = config::Config::builder()
            .add_source(config::File::with_name("config/gateway").required(false))
            .add_source(config::Environment::with_prefix("UPP"))
            .build()?;

        Ok(config.try_deserialize()?)
    }

    /// Build a RateLimitConfig from this gateway configuration.
    pub fn rate_limit_config(&self) -> crate::middleware::rate_limit::RateLimitConfig {
        use crate::middleware::rate_limit::{RateLimitConfig, RateLimitTierConfig, RateLimitTier};

        let mut tiers = HashMap::new();
        tiers.insert(
            RateLimitTier::Light,
            RateLimitTierConfig {
                max_burst: self.rate_limit_light_burst,
                requests_per_second: self.rate_limit_light_rps,
            },
        );
        tiers.insert(
            RateLimitTier::Standard,
            RateLimitTierConfig {
                max_burst: self.rate_limit_standard_burst,
                requests_per_second: self.rate_limit_standard_rps,
            },
        );
        tiers.insert(
            RateLimitTier::Heavy,
            RateLimitTierConfig {
                max_burst: self.rate_limit_heavy_burst,
                requests_per_second: self.rate_limit_heavy_rps,
            },
        );
        tiers.insert(
            RateLimitTier::WebSocket,
            RateLimitTierConfig {
                max_burst: self.rate_limit_ws_burst,
                requests_per_second: self.rate_limit_ws_rps,
            },
        );

        RateLimitConfig {
            tiers,
            cleanup_interval_secs: 60,
            bucket_expiry_secs: 300,
        }
    }
}
