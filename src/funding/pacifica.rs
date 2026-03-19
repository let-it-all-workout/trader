use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Pacifica {
    client: Client,
}

#[derive(Deserialize)]
struct FundingResponse {
    success: bool,
    data: Option<Vec<FundingEntry>>,
}

#[derive(Deserialize)]
struct FundingEntry {
    oracle_price: Option<String>,
    funding_rate: Option<String>,
    next_funding_rate: Option<String>,
}

const SYMBOLS: &[&str] = &[
    "BTC", "ETH", "SOL", "DOGE", "AVAX", "SUI", "HYPE", "WIF", "ARB", "LINK",
    "AAVE", "UNI", "ENA", "NEAR", "LDO", "CRV", "JUP", "STRK", "MKR", "SEI",
    "TIA", "APT", "INJ", "OP", "FIL", "ONDO", "PEPE", "BONK", "RENDER", "W",
    "STX", "RUNE", "PENDLE", "PYTH", "JTO", "FET",
];

impl Pacifica {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Pacifica {
    fn name(&self) -> &str {
        "Pacifica"
    }

    // Pacifica: rate is decimal fraction, 1h period (settled hourly).
    // API requires per-symbol calls.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let now = Utc::now();
        let mut rates = Vec::new();

        let futures: Vec<_> = SYMBOLS
            .iter()
            .map(|sym| {
                let client = self.client.clone();
                let symbol = sym.to_string();
                async move {
                    let url = format!(
                        "https://api.pacifica.fi/api/v1/funding_rate/history?symbol={}&limit=1",
                        symbol
                    );
                    let resp = client.get(&url).send().await.ok()?;
                    let data: FundingResponse = resp.json().await.ok()?;
                    if !data.success {
                        return None;
                    }
                    let entries = data.data?;
                    let entry = entries.first()?;
                    Some((symbol, entry.funding_rate.clone(), entry.next_funding_rate.clone(), entry.oracle_price.clone()))
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        for result in results.into_iter().flatten() {
            let (symbol, rate_str, next_rate_str, oracle) = result;

            let rate: f64 = next_rate_str
                .as_deref()
                .or(rate_str.as_deref())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 1.0);
            let normalized = normalize_symbol(&symbol, "Pacifica");

            rates.push(FundingRate {
                exchange: "Pacifica".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: oracle.as_deref().and_then(|s| s.parse().ok()),
                open_interest: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
