use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PremiumFeedConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_poll_secs")]
    pub poll_secs: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PremiumFeedsConfig {
    #[serde(default)]
    pub ngx_microstructure: PremiumFeedConfig,
    #[serde(default)]
    pub filings_insider: PremiumFeedConfig,
    #[serde(default)]
    pub institutional_flow: PremiumFeedConfig,
}

#[derive(Debug, Clone, Default)]
pub struct PremiumEvent {
    pub event_type: String,
    pub title: String,
    pub symbol: String,
    pub confidence: i32,
    pub source: String,
    pub url: String,
}

pub async fn fetch_ngx_microstructure(
    cfg: &PremiumFeedConfig,
) -> Result<Vec<PremiumEvent>, String> {
    if !cfg.enabled {
        return Ok(Vec::new());
    }
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        return Err("ngx_microstructure feed enabled but endpoint/api_key missing".to_string());
    }
    // TODO: integrate provider response parser when credentials are provided.
    Ok(Vec::new())
}

pub async fn fetch_filings_insider(
    cfg: &PremiumFeedConfig,
) -> Result<Vec<PremiumEvent>, String> {
    if !cfg.enabled {
        return Ok(Vec::new());
    }
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        return Err("filings_insider feed enabled but endpoint/api_key missing".to_string());
    }
    // TODO: integrate provider response parser when credentials are provided.
    Ok(Vec::new())
}

pub async fn fetch_institutional_flow(
    cfg: &PremiumFeedConfig,
) -> Result<Vec<PremiumEvent>, String> {
    if !cfg.enabled {
        return Ok(Vec::new());
    }
    if cfg.endpoint.is_empty() || cfg.api_key.is_empty() {
        return Err("institutional_flow feed enabled but endpoint/api_key missing".to_string());
    }
    // TODO: integrate provider response parser when credentials are provided.
    Ok(Vec::new())
}

fn default_poll_secs() -> u64 {
    180
}
