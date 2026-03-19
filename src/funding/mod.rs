pub mod hyperliquid;
pub mod dydx;
pub mod paradex;
pub mod drift;
pub mod backpack;
pub mod orderly;
pub mod ethereal;
pub mod aster;
pub mod variational;
pub mod pacifica;
pub mod hibachi;
pub mod vest;
pub mod lighter;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct FundingRate {
    pub exchange: String,
    pub symbol: String,
    pub normalized: String,
    /// Raw rate from exchange (decimal fraction)
    pub rate_raw: f64,
    /// 8-hour equivalent rate as percentage (0.01 = 0.01%)
    pub rate_8h_pct: f64,
    /// Annualized rate as percentage (10.0 = 10%)
    pub rate_annual_pct: f64,
    pub mark_price: Option<f64>,
    pub open_interest: Option<f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArbOpportunity {
    pub symbol: String,
    pub long_venue: String,
    pub long_symbol: String,
    pub short_venue: String,
    pub short_symbol: String,
    /// All rates as percentages
    pub long_rate_8h: f64,
    pub long_rate_annual: f64,
    pub short_rate_8h: f64,
    pub short_rate_annual: f64,
    pub spread_8h: f64,
    pub spread_annual: f64,
    pub timestamp: DateTime<Utc>,
}

pub trait FundingSource: Send + Sync {
    fn name(&self) -> &str;
    fn fetch_rates(
        &self,
    ) -> impl std::future::Future<Output = Result<Vec<FundingRate>, FundingError>> + Send;
}

#[derive(Debug)]
pub enum FundingError {
    Http(reqwest::Error),
    Parse(String),
}

impl std::fmt::Display for FundingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "http error: {e}"),
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for FundingError {}

/// Convert a raw decimal-fraction funding rate to an 8-hour equivalent PERCENTAGE.
/// Input: `rate` is a decimal fraction (e.g. 0.0001 = 0.01%).
/// Output: 8-hour equivalent as a percentage (e.g. 0.01 means 0.01%).
pub fn to_8h_pct(rate_decimal: f64, period_hours: f64) -> f64 {
    rate_decimal * (8.0 / period_hours) * 100.0
}

/// Convert 8h percentage rate to annualized percentage.
/// Input and output are both percentages.
pub fn to_annual_pct(rate_8h_pct: f64) -> f64 {
    rate_8h_pct * 3.0 * 365.0
}

pub fn normalize_symbol(symbol: &str, exchange: &str) -> String {
    let s = symbol.trim();
    match exchange {
        "Hyperliquid" => s.to_uppercase(),
        "dYdX" => s.split('-').next().unwrap_or(s).to_uppercase(),
        "Paradex" => s.split('-').next().unwrap_or(s).to_uppercase(),
        "Drift" => s.replace("-PERP", "").to_uppercase(),
        "Backpack" => s.split('_').next().unwrap_or(s).to_uppercase(),
        "Orderly" => {
            let parts: Vec<&str> = s.split('_').collect();
            if parts.len() >= 2 && parts[0] == "PERP" {
                parts[1].to_uppercase()
            } else {
                s.to_uppercase()
            }
        }
        "Ethereal" => s.split('-').next().unwrap_or(s).to_uppercase(),
        "Aster" => normalize_aster_symbol(s),
        "Variational" | "Pacifica" | "Lighter" => s.to_uppercase(),
        // Hibachi: "BTC/USDT-P" → "BTC"
        "Hibachi" => s.split('/').next().unwrap_or(s).to_uppercase(),
        // Vest: "BTC-PERP", "AAPL-USD-PERP" → "BTC", "AAPL"
        "Vest" => {
            let upper = s.to_uppercase();
            upper
                .strip_suffix("-PERP")
                .unwrap_or(&upper)
                .strip_suffix("-USD")
                .unwrap_or(upper.strip_suffix("-PERP").unwrap_or(&upper))
                .to_string()
        }
        _ => s.to_uppercase(),
    }
}

fn normalize_aster_symbol(s: &str) -> String {
    let upper = s.to_uppercase();
    if upper.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return upper;
    }
    for suffix in &["USDT", "USDC", "USD"] {
        if upper.ends_with(suffix) && upper.len() > suffix.len() {
            return upper[..upper.len() - suffix.len()].to_string();
        }
    }
    upper
}

pub const EXCHANGES: &[&str] = &[
    "Hyperliquid",
    "dYdX",
    "Paradex",
    "Drift",
    "Backpack",
    "Orderly",
    "Ethereal",
    "Aster",
    "Variational",
    "Pacifica",
    "Hibachi",
    "Vest",
    "Lighter",
];

/// min_spread_pct is a percentage threshold (e.g. 0.01 = 0.01%)
pub fn detect_arbs(
    rates: &[FundingRate],
    min_spread_pct: f64,
    exchanges: &HashSet<String>,
) -> Vec<ArbOpportunity> {
    let filtered: Vec<&FundingRate> = rates
        .iter()
        .filter(|r| exchanges.contains(&r.exchange))
        .collect();

    let mut by_symbol: HashMap<&str, Vec<&FundingRate>> = HashMap::new();
    for rate in &filtered {
        by_symbol.entry(&rate.normalized).or_default().push(rate);
    }

    let mut arbs = Vec::new();

    for (symbol, venue_rates) in &by_symbol {
        if venue_rates.len() < 2 {
            continue;
        }

        let unique_exchanges: HashSet<&str> =
            venue_rates.iter().map(|r| r.exchange.as_str()).collect();
        if unique_exchanges.len() < 2 {
            continue;
        }

        let mut sorted = venue_rates.clone();
        sorted.sort_by(|a, b| a.rate_8h_pct.partial_cmp(&b.rate_8h_pct).unwrap());

        let lowest = sorted.first().unwrap();
        let highest = sorted.last().unwrap();

        if lowest.exchange == highest.exchange {
            continue;
        }

        let spread = highest.rate_8h_pct - lowest.rate_8h_pct;
        if spread.abs() >= min_spread_pct {
            arbs.push(ArbOpportunity {
                symbol: symbol.to_string(),
                long_venue: lowest.exchange.clone(),
                long_symbol: lowest.symbol.clone(),
                short_venue: highest.exchange.clone(),
                short_symbol: highest.symbol.clone(),
                long_rate_8h: lowest.rate_8h_pct,
                long_rate_annual: lowest.rate_annual_pct,
                short_rate_8h: highest.rate_8h_pct,
                short_rate_annual: highest.rate_annual_pct,
                spread_8h: spread,
                spread_annual: to_annual_pct(spread),
                timestamp: Utc::now(),
            });
        }
    }

    arbs.sort_by(|a, b| {
        b.spread_8h
            .abs()
            .partial_cmp(&a.spread_8h.abs())
            .unwrap()
    });
    arbs
}

#[derive(Debug, Clone, Serialize)]
pub struct StableArbOpportunity {
    pub symbol: String,
    pub long_venue: String,
    pub short_venue: String,
    pub avg_spread_8h: f64,
    pub avg_spread_annual: f64,
    pub current_spread_annual: f64,
    /// 0.0–1.0, fraction of scan cycles where this spread was positive
    pub consistency: f64,
    pub samples: usize,
    pub long_avg_annual: f64,
    pub short_avg_annual: f64,
}

/// Keeps a rolling window of 8h rates per (exchange, normalized_symbol).
/// Used to compute averaged rates and detect stable arb opportunities.
#[derive(Debug, Clone, Default)]
pub struct RateHistory {
    /// (exchange, normalized_symbol) → ring buffer of 8h_pct values
    entries: HashMap<(String, String), Vec<f64>>,
    max_samples: usize,
}

impl RateHistory {
    pub fn new(max_samples: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_samples,
        }
    }

    pub fn push(&mut self, rates: &[FundingRate]) {
        let mut seen: HashSet<(String, String)> = HashSet::new();
        for r in rates {
            let key = (r.exchange.clone(), r.normalized.clone());
            if !seen.insert(key.clone()) {
                continue;
            }
            let buf = self.entries.entry(key).or_default();
            buf.push(r.rate_8h_pct);
            if buf.len() > self.max_samples {
                buf.remove(0);
            }
        }
    }

    pub fn avg_rate(&self, exchange: &str, symbol: &str) -> Option<f64> {
        let buf = self.entries.get(&(exchange.to_string(), symbol.to_string()))?;
        if buf.is_empty() {
            return None;
        }
        Some(buf.iter().sum::<f64>() / buf.len() as f64)
    }

    pub fn sample_count(&self, exchange: &str, symbol: &str) -> usize {
        self.entries
            .get(&(exchange.to_string(), symbol.to_string()))
            .map(|b| b.len())
            .unwrap_or(0)
    }
}

/// Detect stable arbs: uses averaged rates from history and requires
/// the spread to have been consistently positive across scan cycles.
pub fn detect_stable_arbs(
    current_rates: &[FundingRate],
    history: &RateHistory,
    min_spread_pct: f64,
    min_consistency: f64,
    min_samples: usize,
    exchanges: &HashSet<String>,
) -> Vec<StableArbOpportunity> {
    let filtered: Vec<&FundingRate> = current_rates
        .iter()
        .filter(|r| exchanges.contains(&r.exchange))
        .collect();

    let mut by_symbol: HashMap<&str, Vec<&FundingRate>> = HashMap::new();
    for rate in &filtered {
        by_symbol.entry(&rate.normalized).or_default().push(rate);
    }

    let mut arbs = Vec::new();

    for (symbol, venue_rates) in &by_symbol {
        if venue_rates.len() < 2 {
            continue;
        }

        let unique_exchanges: HashSet<&str> =
            venue_rates.iter().map(|r| r.exchange.as_str()).collect();
        if unique_exchanges.len() < 2 {
            continue;
        }

        for i in 0..venue_rates.len() {
            for j in (i + 1)..venue_rates.len() {
                let a = venue_rates[i];
                let b = venue_rates[j];
                if a.exchange == b.exchange {
                    continue;
                }

                let samples_a = history.sample_count(&a.exchange, symbol);
                let samples_b = history.sample_count(&b.exchange, symbol);
                let samples = samples_a.min(samples_b);
                if samples < min_samples {
                    continue;
                }

                let avg_a = match history.avg_rate(&a.exchange, symbol) {
                    Some(v) => v,
                    None => continue,
                };
                let avg_b = match history.avg_rate(&b.exchange, symbol) {
                    Some(v) => v,
                    None => continue,
                };

                let (long_ex, short_ex, long_avg, short_avg) = if avg_a < avg_b {
                    (&a.exchange, &b.exchange, avg_a, avg_b)
                } else {
                    (&b.exchange, &a.exchange, avg_b, avg_a)
                };

                let avg_spread = short_avg - long_avg;
                if avg_spread < min_spread_pct {
                    continue;
                }

                let buf_long = history
                    .entries
                    .get(&(long_ex.clone(), symbol.to_string()));
                let buf_short = history
                    .entries
                    .get(&(short_ex.clone(), symbol.to_string()));

                let consistency = match (buf_long, buf_short) {
                    (Some(bl), Some(bs)) => {
                        let n = bl.len().min(bs.len());
                        if n == 0 {
                            0.0
                        } else {
                            let positive = bl
                                .iter()
                                .rev()
                                .zip(bs.iter().rev())
                                .take(n)
                                .filter(|(l, s)| *s - *l > 0.0)
                                .count();
                            positive as f64 / n as f64
                        }
                    }
                    _ => continue,
                };

                if consistency < min_consistency {
                    continue;
                }

                let current_a = a.rate_8h_pct;
                let current_b = b.rate_8h_pct;
                let current_spread = if avg_a < avg_b {
                    current_b - current_a
                } else {
                    current_a - current_b
                };

                arbs.push(StableArbOpportunity {
                    symbol: symbol.to_string(),
                    long_venue: long_ex.clone(),
                    short_venue: short_ex.clone(),
                    avg_spread_8h: avg_spread,
                    avg_spread_annual: to_annual_pct(avg_spread),
                    current_spread_annual: to_annual_pct(current_spread),
                    consistency,
                    samples,
                    long_avg_annual: to_annual_pct(long_avg),
                    short_avg_annual: to_annual_pct(short_avg),
                });
            }
        }
    }

    arbs.sort_by(|a, b| {
        b.avg_spread_annual
            .abs()
            .partial_cmp(&a.avg_spread_annual.abs())
            .unwrap()
    });
    arbs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeFilter {
    pub exchanges: HashSet<String>,
}

impl Default for ExchangeFilter {
    fn default() -> Self {
        Self {
            exchanges: EXCHANGES.iter().map(|s| s.to_string()).collect(),
        }
    }
}
