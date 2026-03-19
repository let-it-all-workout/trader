use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use crate::news::NewsConfig;
use crate::premium_feeds::PremiumFeedsConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub telegram: TelegramConfig,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub news: NewsConfig,
    #[serde(default)]
    pub premium_feeds: PremiumFeedsConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TelegramConfig {
    pub bot_token: String,
    #[serde(default)]
    pub channels: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    pub telegram_channel: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, String>,
}

fn default_port() -> u16 {
    3000
}

fn default_true() -> bool {
    true
}

fn default_interval() -> u64 {
    60
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn channel_id(&self, name: &str) -> Option<&str> {
        self.telegram.channels.get(name).map(|s| s.as_str())
    }
}
