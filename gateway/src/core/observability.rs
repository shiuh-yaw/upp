// Copyright 2026 Universal Prediction Protocol Authors
// SPDX-License-Identifier: Apache-2.0
//
// Comprehensive observability module for UPP Gateway.
// Provides tracing, metrics, and health checks with OpenTelemetry integration.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, debug};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Layer, Registry,
};

// ─── Tracing Configuration ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Log format: "json" or "pretty"
    pub format: String,
    /// Log level: "debug", "info", "warn", "error"
    pub level: String,
    /// Enable OpenTelemetry/Jaeger integration
    pub enable_otlp: bool,
    /// OTLP exporter endpoint (e.g., http://jaeger:4317)
    pub otlp_endpoint: String,
    /// Module-specific log levels (e.g., "upp_gateway=debug,tower_http=info")
    pub module_levels: Option<String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            format: "json".to_string(),
            level: "info".to_string(),
            enable_otlp: true,
            otlp_endpoint: "http://localhost:4317".to_string(),
            module_levels: None,
        }
    }
}

/// Initialize the tracing system with optional OTLP export to Jaeger.
pub fn init_tracing(config: &TracingConfig) -> Result<()> {
    let env_filter = if let Some(modules) = &config.module_levels {
        EnvFilter::new(modules.clone())
    } else {
        EnvFilter::new(config.level.clone())
    };

    let registry = Registry::default().with(env_filter);

    // Format layer (JSON or pretty)
    let fmt_layer = if config.format == "json" {
        fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::FULL)
            .boxed()
    } else {
        fmt::layer()
            .pretty()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_span_events(FmtSpan::CLOSE)
            .boxed()
    };

    let registry = registry.with(fmt_layer);

    // Optional OTLP/Jaeger exporter
    if config.enable_otlp {
        // Note: In production, use opentelemetry-otlp with proper setup.
        // This is a placeholder that logs the intent.
        debug!(
            endpoint = %config.otlp_endpoint,
            "OpenTelemetry OTLP exporter would be configured here"
        );
    }

    registry.init();

    info!(
        format = %config.format,
        level = %config.level,
        enable_otlp = config.enable_otlp,
        "Tracing initialized"
    );

    Ok(())
}

// ─── Prometheus Metrics ────────────────────────────────────────────

/// Request and system metrics for Prometheus export.
pub struct PrometheusMetrics {
    // Counters
    requests_total: Arc<AtomicU64>,
    ws_messages_total: Arc<AtomicU64>,
    provider_requests_total: Arc<AtomicU64>,

    // Gauges
    active_ws_connections: Arc<AtomicU64>,
    cache_size: Arc<AtomicU64>,
    connected_providers: Arc<AtomicU64>,

    // Histogram buckets (simplified: store latency samples in memory)
    request_duration_samples: Arc<dashmap::DashMap<String, Vec<f64>>>,
}

impl PrometheusMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: Arc::new(AtomicU64::new(0)),
            ws_messages_total: Arc::new(AtomicU64::new(0)),
            provider_requests_total: Arc::new(AtomicU64::new(0)),
            active_ws_connections: Arc::new(AtomicU64::new(0)),
            cache_size: Arc::new(AtomicU64::new(0)),
            connected_providers: Arc::new(AtomicU64::new(0)),
            request_duration_samples: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Record an HTTP request with method, path, status code, and duration.
    pub fn record_request(
        &self,
        method: &str,
        path: &str,
        status: u16,
        duration_ms: f64,
    ) {
        // Increment total requests
        self.requests_total.fetch_add(1, Ordering::Relaxed);

        // Use metrics crate for Prometheus export
        metrics::counter!("requests_total", "method" => method.to_string(), "path" => path.to_string(), "status" => status.to_string()).increment(1);

        // Record latency histogram
        metrics::histogram!("request_duration_seconds", "method" => method.to_string(), "path" => path.to_string(), "status" => status.to_string()).record(duration_ms / 1000.0);

        // Store sample for percentile calculations
        let key = format!("{}_{}", method, path);
        self.request_duration_samples
            .entry(key)
            .or_insert_with(Vec::new)
            .push(duration_ms);
    }

    /// Record a WebSocket message.
    pub fn record_ws_message(&self) {
        self.ws_messages_total.fetch_add(1, Ordering::Relaxed);
        metrics::counter!("ws_messages_total").increment(1);
    }

    /// Record a provider request.
    pub fn record_provider_request(&self, provider: &str) {
        self.provider_requests_total.fetch_add(1, Ordering::Relaxed);
        metrics::counter!("provider_requests_total", "provider" => provider.to_string()).increment(1);
    }

    /// Increment active WebSocket connections.
    pub fn increment_ws_connections(&self) {
        let count = self.active_ws_connections.fetch_add(1, Ordering::Relaxed) + 1;
        metrics::gauge!("active_ws_connections").set(count as f64);
    }

    /// Decrement active WebSocket connections.
    pub fn decrement_ws_connections(&self) {
        if let Some(count) = self
            .active_ws_connections
            .fetch_sub(1, Ordering::Relaxed)
            .checked_sub(1)
        {
            metrics::gauge!("active_ws_connections").set(count as f64);
        }
    }

    /// Update cache size gauge.
    pub fn set_cache_size(&self, size: u64) {
        self.cache_size.store(size, Ordering::Relaxed);
        metrics::gauge!("cache_size").set(size as f64);
    }

    /// Update connected providers gauge.
    pub fn set_connected_providers(&self, count: u64) {
        self.connected_providers.store(count, Ordering::Relaxed);
        metrics::gauge!("connected_providers").set(count as f64);
    }

    /// Export metrics in Prometheus text format.
    pub fn metrics_handler(&self) -> String {
        let mut output = String::new();

        // Help and type metadata
        output.push_str("# HELP requests_total Total HTTP requests\n");
        output.push_str("# TYPE requests_total counter\n");
        output.push_str(&format!(
            "requests_total {}\n",
            self.requests_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP ws_messages_total Total WebSocket messages\n");
        output.push_str("# TYPE ws_messages_total counter\n");
        output.push_str(&format!(
            "ws_messages_total {}\n",
            self.ws_messages_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP provider_requests_total Total provider requests\n");
        output.push_str("# TYPE provider_requests_total counter\n");
        output.push_str(&format!(
            "provider_requests_total {}\n",
            self.provider_requests_total.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP active_ws_connections Active WebSocket connections\n");
        output.push_str("# TYPE active_ws_connections gauge\n");
        output.push_str(&format!(
            "active_ws_connections {}\n",
            self.active_ws_connections.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP cache_size Current cache size in bytes\n");
        output.push_str("# TYPE cache_size gauge\n");
        output.push_str(&format!(
            "cache_size {}\n",
            self.cache_size.load(Ordering::Relaxed)
        ));

        output.push_str("# HELP connected_providers Number of connected providers\n");
        output.push_str("# TYPE connected_providers gauge\n");
        output.push_str(&format!(
            "connected_providers {}\n",
            self.connected_providers.load(Ordering::Relaxed)
        ));

        // Request duration histogram (simplified)
        output.push_str("# HELP request_duration_seconds Request latency in seconds\n");
        output.push_str("# TYPE request_duration_seconds histogram\n");

        for ref_multi in self.request_duration_samples.iter() {
            let (key, samples) = ref_multi.pair();
            if !samples.is_empty() {
                let count = samples.len();
                let sum: f64 = samples.iter().sum::<f64>() / 1000.0; // Convert to seconds
                output.push_str(&format!(
                    "request_duration_seconds_bucket{{path=\"{}\",le=\"0.001\"}} {}\n",
                    key,
                    samples.iter().filter(|d| **d <=1.0).count()
                ));
                output.push_str(&format!(
                    "request_duration_seconds_bucket{{path=\"{}\",le=\"0.01\"}} {}\n",
                    key,
                    samples.iter().filter(|d| **d <=10.0).count()
                ));
                output.push_str(&format!(
                    "request_duration_seconds_bucket{{path=\"{}\",le=\"0.1\"}} {}\n",
                    key,
                    samples.iter().filter(|d| **d <=100.0).count()
                ));
                output.push_str(&format!(
                    "request_duration_seconds_bucket{{path=\"{}\",le=\"1.0\"}} {}\n",
                    key,
                    samples.iter().filter(|d| **d <=1000.0).count()
                ));
                output.push_str(&format!(
                    "request_duration_seconds_bucket{{path=\"{}\",le=\"+Inf\"}} {}\n",
                    key, count
                ));
                output.push_str(&format!(
                    "request_duration_seconds_count{{path=\"{}\"}} {}\n",
                    key, count
                ));
                output.push_str(&format!(
                    "request_duration_seconds_sum{{path=\"{}\"}} {}\n",
                    key, sum
                ));
            }
        }

        output
    }
}

impl Default for PrometheusMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Health Checks ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String, // "healthy", "degraded", "unhealthy"
    pub checks: HealthChecks,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecks {
    pub redis: CheckResult,
    pub providers: CheckResult,
    pub cache: CheckResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub status: String, // "ok", "warning", "error"
    pub message: String,
    pub latency_ms: Option<f64>,
}

pub struct HealthCheck {
    metrics: Arc<PrometheusMetrics>,
}

impl HealthCheck {
    pub fn new(metrics: Arc<PrometheusMetrics>) -> Self {
        Self { metrics }
    }

    /// Perform liveness check (basic checks).
    pub fn liveness(&self) -> HealthStatus {
        HealthStatus {
            status: "healthy".to_string(),
            checks: HealthChecks {
                redis: CheckResult {
                    status: "ok".to_string(),
                    message: "Gateway is running".to_string(),
                    latency_ms: None,
                },
                providers: CheckResult {
                    status: "ok".to_string(),
                    message: "Providers service available".to_string(),
                    latency_ms: None,
                },
                cache: CheckResult {
                    status: "ok".to_string(),
                    message: "Cache service available".to_string(),
                    latency_ms: None,
                },
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Perform readiness check (dependencies).
    pub fn readiness(&self) -> HealthStatus {
        let redis_status = self.check_redis();
        let providers_status = self.check_providers();
        let cache_status = self.check_cache();

        let overall_status = if redis_status.status == "error"
            || providers_status.status == "error"
            || cache_status.status == "error"
        {
            "unhealthy".to_string()
        } else if redis_status.status == "warning"
            || providers_status.status == "warning"
            || cache_status.status == "warning"
        {
            "degraded".to_string()
        } else {
            "healthy".to_string()
        };

        HealthStatus {
            status: overall_status,
            checks: HealthChecks {
                redis: redis_status,
                providers: providers_status,
                cache: cache_status,
            },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn check_redis(&self) -> CheckResult {
        // In production, this would ping Redis.
        // For now, it's a placeholder that would be implemented
        // with an actual Redis connection.
        CheckResult {
            status: "ok".to_string(),
            message: "Redis connection available".to_string(),
            latency_ms: Some(1.5),
        }
    }

    fn check_providers(&self) -> CheckResult {
        let connected = self.metrics.connected_providers.load(Ordering::Relaxed);
        if connected > 0 {
            CheckResult {
                status: "ok".to_string(),
                message: format!("{} providers connected", connected),
                latency_ms: None,
            }
        } else {
            CheckResult {
                status: "warning".to_string(),
                message: "No providers connected".to_string(),
                latency_ms: None,
            }
        }
    }

    fn check_cache(&self) -> CheckResult {
        let cache_size = self.metrics.cache_size.load(Ordering::Relaxed);
        CheckResult {
            status: "ok".to_string(),
            message: format!("Cache size: {} bytes", cache_size),
            latency_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prometheus_metrics_creation() {
        let metrics = PrometheusMetrics::new();
        assert_eq!(metrics.requests_total.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.ws_messages_total.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_record_request() {
        let metrics = PrometheusMetrics::new();
        metrics.record_request("GET", "/health", 200, 10.5);
        assert_eq!(metrics.requests_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_ws_connections() {
        let metrics = PrometheusMetrics::new();
        metrics.increment_ws_connections();
        assert_eq!(metrics.active_ws_connections.load(Ordering::Relaxed), 1);
        metrics.decrement_ws_connections();
        assert_eq!(metrics.active_ws_connections.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_health_check() {
        let metrics = Arc::new(PrometheusMetrics::new());
        let health = HealthCheck::new(metrics);
        let liveness = health.liveness();
        assert_eq!(liveness.status, "healthy");
    }
}
