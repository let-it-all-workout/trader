use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Backpack {
    client: Client,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkPriceEntry {
    symbol: String,
    funding_rate: Option<String>,
    mark_price: Option<String>,
}

impl Backpack {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Backpack {
    fn name(&self) -> &str {
        "Backpack"
    }

    // Backpack: rate is decimal fraction, 1h period
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://api.backpack.exchange/api/v1/markPrices")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: Vec<MarkPriceEntry> = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for entry in data {
            if !entry.symbol.contains("PERP") {
                continue;
            }

            let rate: f64 = entry
                .funding_rate
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 1.0);
            let symbol = entry.symbol;
            let normalized = normalize_symbol(&symbol, "Backpack");

            rates.push(FundingRate {
                exchange: "Backpack".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: entry.mark_price.as_deref().and_then(|s| s.parse().ok()),
                open_interest: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
