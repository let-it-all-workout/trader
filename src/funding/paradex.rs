use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Paradex {
    client: Client,
}

#[derive(Deserialize)]
struct SummaryResponse {
    results: Vec<MarketSummary>,
}

#[derive(Deserialize)]
struct MarketSummary {
    symbol: String,
    #[serde(default)]
    funding_rate: Option<String>,
    #[serde(default)]
    mark_price: Option<String>,
    #[serde(default)]
    open_interest: Option<String>,
}

impl Paradex {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Paradex {
    fn name(&self) -> &str {
        "Paradex"
    }

    // Paradex: rate is decimal fraction, 8h period
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://api.prod.paradex.trade/v1/markets/summary?market=ALL")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: SummaryResponse = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for market in data.results {
            if !market.symbol.contains("PERP") {
                continue;
            }

            let rate: f64 = market
                .funding_rate
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 8.0);

            rates.push(FundingRate {
                exchange: "Paradex".into(),
                symbol: market.symbol.clone(),
                normalized: normalize_symbol(&market.symbol, "Paradex"),
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: market.mark_price.as_deref().and_then(|s| s.parse().ok()),
                open_interest: market.open_interest.as_deref().and_then(|s| s.parse().ok()),
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
