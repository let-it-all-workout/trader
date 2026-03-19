use super::{normalize_symbol, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Variational {
    client: Client,
}

#[derive(Deserialize)]
struct StatsResponse {
    listings: Vec<Listing>,
}

#[derive(Deserialize)]
struct Listing {
    ticker: String,
    mark_price: Option<String>,
    funding_rate: Option<String>,
    funding_interval_s: Option<u64>,
    #[serde(default)]
    open_interest: Option<OiData>,
}

#[derive(Deserialize)]
struct OiData {
    long_open_interest: Option<String>,
    short_open_interest: Option<String>,
}

impl Variational {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Variational {
    fn name(&self) -> &str {
        "Variational"
    }

    // Variational: funding_rate is annualized decimal (0.1095 = 10.95% APR).
    // funding_interval_s indicates settlement frequency but rate is always annualized.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://omni-client-api.prod.ap-northeast-1.variational.io/metadata/stats")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: StatsResponse = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for listing in data.listings {
            let rate_annual_dec: f64 = listing
                .funding_rate
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let oi: f64 = listing
                .open_interest
                .as_ref()
                .map(|o| {
                    let long: f64 = o
                        .long_open_interest
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    let short: f64 = o
                        .short_open_interest
                        .as_deref()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    long + short.abs()
                })
                .unwrap_or(0.0);

            if oi == 0.0 {
                continue;
            }

            let rate_annual_pct = rate_annual_dec * 100.0;
            let rate_8h_pct = rate_annual_pct / (3.0 * 365.0);

            let symbol = listing.ticker;
            let normalized = normalize_symbol(&symbol, "Variational");

            rates.push(FundingRate {
                exchange: "Variational".into(),
                symbol,
                normalized,
                rate_raw: rate_annual_dec,
                rate_8h_pct,
                rate_annual_pct,
                mark_price: listing
                    .mark_price
                    .as_deref()
                    .and_then(|s| s.parse().ok()),
                open_interest: Some(oi),
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
