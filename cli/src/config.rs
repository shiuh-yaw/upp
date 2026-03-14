use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub gateway_url: String,
    pub api_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gateway_url: "http://localhost:9090".to_string(),
            api_key: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)?;
        let config = toml::from_str(&content).unwrap_or_default();
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Cannot determine home directory"))?
            .join(".upp");

        Ok(config_dir.join("config.toml"))
    }

    pub fn with_url(mut self, url: Option<String>) -> Self {
        if let Some(url) = url {
            self.gateway_url = url;
        }
        self
    }

    pub fn with_api_key(mut self, api_key: Option<String>) -> Self {
        if let Some(api_key) = api_key {
            self.api_key = Some(api_key);
        }
        self
    }

    pub fn gateway_url(&self) -> String {
        self.gateway_url.trim_end_matches('/').to_string()
    }
}
