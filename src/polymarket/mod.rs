pub mod negrisk;
pub mod endgame;
pub mod correlation;
pub mod marketmaking;
pub mod crossplatform;
pub mod newsarb;

use serde::Serialize;

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default()
}

// ── Opportunity types ──

#[derive(Debug, Clone, Serialize)]
pub struct NegRiskOpportunity {
    pub event_id: String,
    pub event_title: String,
    pub url: String,
    pub num_outcomes: usize,
    pub yes_sum: f64,
    /// Positive = overpriced (buy all NO), negative = underpriced
    pub mispricing: f64,
    pub profit_margin_pct: f64,
    pub volume_24h: f64,
    pub liquidity: f64,
    pub outcomes: Vec<OutcomePrice>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutcomePrice {
    pub question: String,
    pub yes_price: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndgameOpportunity {
    pub market_id: String,
    pub question: String,
    pub url: String,
    pub side: String,
    pub buy_price: f64,
    pub profit_pct: f64,
    pub days_to_resolution: i64,
    pub annualized_return: f64,
    pub volume_24h: f64,
    pub liquidity: f64,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrelationOpportunity {
    pub event_title: String,
    pub event_id: String,
    pub url: String,
    pub parent_question: String,
    pub parent_yes_price: f64,
    pub child_sum: f64,
    pub inconsistency_pct: f64,
    /// Actual ROI on capital deployed
    pub roi_pct: f64,
    pub num_children: usize,
    pub details: Vec<CorrelationDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrelationDetail {
    pub question: String,
    pub yes_price: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketMakingOpportunity {
    pub market_id: String,
    pub question: String,
    pub url: String,
    pub best_bid: f64,
    pub best_ask: f64,
    pub spread_pct: f64,
    pub mid_price: f64,
    pub volume_24h: f64,
    pub liquidity: f64,
    pub bid_depth: f64,
    pub ask_depth: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrossPlatformOpportunity {
    pub event_title: String,
    pub polymarket_question: String,
    pub polymarket_url: String,
    pub kalshi_ticker: String,
    pub kalshi_url: String,
    pub polymarket_yes: f64,
    pub kalshi_yes: f64,
    pub spread_pct: f64,
    pub buy_platform: String,
    pub sell_platform: String,
    pub polymarket_volume: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewsArbOpportunity {
    pub market_id: String,
    pub question: String,
    pub url: String,
    pub event_title: String,
    pub current_price: f64,
    pub price_change_pct: f64,
    pub direction: String,
    pub volume_24h: f64,
    pub liquidity: f64,
}

// ── Error type ──

#[derive(Debug)]
#[allow(dead_code)]
pub enum PolymarketError {
    Http(reqwest::Error),
    Parse(String),
}

impl std::fmt::Display for PolymarketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "http: {e}"),
            Self::Parse(msg) => write!(f, "parse: {msg}"),
        }
    }
}

impl std::error::Error for PolymarketError {}
