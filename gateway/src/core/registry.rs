// Provider registry — manages all registered provider adapters.

use super::config::GatewayConfig;
use crate::adapters::{UppProvider, ProviderManifest, ProviderHealth};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn UppProvider>>,
}

impl ProviderRegistry {
    pub async fn new(config: &GatewayConfig) -> Result<Self> {
        let mut providers: HashMap<String, Arc<dyn UppProvider>> = HashMap::new();

        // Register Kalshi
        // - With auth: full trading capabilities
        // - Without auth: public market data only (default)
        if let (Some(key_id), Some(_key_path)) = (&config.kalshi_api_key_id, &config.kalshi_private_key_path) {
            let adapter = crate::adapters::kalshi::KalshiAdapter::new_authenticated(
                key_id.clone(),
                vec![], // TODO: Load private key from path
            );
            providers.insert("kalshi.com".to_string(), Arc::new(adapter));
            tracing::info!("Registered Kalshi adapter (authenticated)");
        } else {
            let adapter = crate::adapters::kalshi::KalshiAdapter::new_public();
            providers.insert("kalshi.com".to_string(), Arc::new(adapter));
            tracing::info!("Registered Kalshi adapter (public, read-only)");
        }

        // Register Polymarket
        // - With wallet: full trading capabilities
        // - Without wallet: public market data only (default)
        if let Some(ref wallet_key) = config.polymarket_wallet_key {
            let adapter = crate::adapters::polymarket::PolymarketAdapter::new_authenticated(
                wallet_key.clone(),
            );
            providers.insert("polymarket.com".to_string(), Arc::new(adapter));
            tracing::info!("Registered Polymarket adapter (authenticated)");
        } else {
            let adapter = crate::adapters::polymarket::PolymarketAdapter::new_public();
            providers.insert("polymarket.com".to_string(), Arc::new(adapter));
            tracing::info!("Registered Polymarket adapter (public, read-only)");
        }

        // Register Opinion
        // - Requires API key for all endpoints (even read-only)
        if let Some(api_key) = &config.opinion_api_key {
            let adapter = crate::adapters::opinion::OpinionAdapter::new(api_key.clone());
            providers.insert("opinion.trade".to_string(), Arc::new(adapter));
            tracing::info!("Registered Opinion adapter (with API key)");
        } else {
            let adapter = crate::adapters::opinion::OpinionAdapter::new_without_key();
            providers.insert("opinion.trade".to_string(), Arc::new(adapter));
            tracing::info!("Registered Opinion adapter (no API key — will return errors)");
        }

        tracing::info!("Provider registry initialized with {} providers", providers.len());

        Ok(Self { providers })
    }

    /// Get a specific provider adapter.
    pub fn get(&self, provider_id: &str) -> Option<Arc<dyn UppProvider>> {
        self.providers.get(provider_id).cloned()
    }

    /// Get all registered provider IDs.
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Get a provider's capability manifest.
    pub async fn get_manifest(&self, provider_id: &str) -> Result<ProviderManifest> {
        self.providers
            .get(provider_id)
            .map(|p| p.manifest())
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_id))
    }

    /// List all registered providers' manifests.
    pub async fn list_providers(&self) -> Vec<ProviderManifest> {
        self.providers.values().map(|p| p.manifest()).collect()
    }

    /// Health check for a specific provider.
    pub async fn health_check(&self, provider_id: &str) -> Result<ProviderHealth> {
        let provider = self.providers
            .get(provider_id)
            .ok_or_else(|| anyhow::anyhow!("Provider not found: {}", provider_id))?;
        provider.health_check().await
    }

    /// Health check all providers.
    pub async fn health_check_all(&self) -> Vec<ProviderHealth> {
        let mut results = Vec::new();
        for (id, provider) in &self.providers {
            match provider.health_check().await {
                Ok(health) => results.push(health),
                Err(e) => results.push(ProviderHealth {
                    provider: id.clone(),
                    healthy: false,
                    status: format!("error: {}", e),
                    latency_ms: 0,
                }),
            }
        }
        results
    }
}
