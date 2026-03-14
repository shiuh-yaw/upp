// Gateway configuration — loaded from environment variables and config files.

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayConfig {
    // Environment and core settings
    #[serde(default = "default_environment")]
    pub environment: String,

    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,

    // Logging
    #[serde(default = "default_log_format")]
    pub log_format: String,

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

    // Authentication & CORS
    #[serde(default = "default_auth_required")]
    pub auth_required: bool,
    pub jwt_secret: Option<String>,
    #[serde(default = "default_cors_origins")]
    pub cors_origins: Vec<String>,

    // Connection management
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_graceful_shutdown_timeout_secs")]
    pub graceful_shutdown_timeout_secs: u64,

    // TLS/SSL
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

// Environment and core defaults
fn default_environment() -> String { "dev".to_string() }
fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_grpc_port() -> u16 { 50051 }
fn default_log_format() -> String { "json".to_string() }

// Cache defaults
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

// Auth & CORS defaults
fn default_auth_required() -> bool { false }
fn default_cors_origins() -> Vec<String> { vec!["*".to_string()] }

// Connection management defaults
fn default_max_connections() -> u32 { 10000 }
fn default_graceful_shutdown_timeout_secs() -> u64 { 30 }

impl GatewayConfig {
    /// Load configuration with environment variable overrides.
    /// Order of precedence: env vars > config/gateway.{env}.toml > config/gateway.toml > defaults
    pub fn load() -> Result<Self> {
        let _ = dotenvy::dotenv(); // Load .env if present

        // Determine environment from UPP_ENVIRONMENT or default to "dev"
        let env = std::env::var("UPP_ENVIRONMENT")
            .unwrap_or_else(|_| "dev".to_string());

        Self::load_for_env(&env)
    }

    /// Load configuration for a specific environment.
    /// Loads base config, then overlays environment-specific config, then applies env vars.
    pub fn load_for_env(env: &str) -> Result<Self> {
        let builder = config::Config::builder()
            // Base configuration
            .add_source(config::File::with_name("config/gateway").required(false))
            // Environment-specific configuration
            .add_source(
                config::File::with_name(&format!("config/gateway.{}", env))
                    .required(false)
            )
            // Environment variables (highest priority for overrides)
            .add_source(config::Environment::with_prefix("UPP"));

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }

    /// Get environment name (dev, staging, prod).
    pub fn env_name(&self) -> &str {
        &self.environment
    }

    /// Check if we're in development mode.
    pub fn is_dev(&self) -> bool {
        self.environment == "dev"
    }

    /// Check if we're in production mode.
    pub fn is_prod(&self) -> bool {
        self.environment == "prod"
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
            redis_url: self.redis_url.clone(),
            use_sliding_window: false,
        }
    }
}
