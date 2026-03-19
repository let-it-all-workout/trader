use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Orderly {
    client: Client,
}

#[derive(Deserialize)]
struct FundingRatesResponse {
    success: bool,
    data: FundingRatesData,
}

#[derive(Deserialize)]
struct FundingRatesData {
    rows: Vec<FundingRateRow>,
}

#[derive(Deserialize)]
struct FundingRateRow {
    symbol: String,
    est_funding_rate: Option<f64>,
    last_funding_rate: Option<f64>,
}

impl Orderly {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Orderly {
    fn name(&self) -> &str {
        "Orderly"
    }

    // Orderly: rate is decimal fraction, 8h period (verified from API timestamps)
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://api.orderly.org/v1/public/funding_rates")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: FundingRatesResponse = resp.json().await.map_err(FundingError::Http)?;

        if !data.success {
            return Err(FundingError::Parse("orderly returned success=false".into()));
        }

        let now = Utc::now();
        let mut rates = Vec::new();

        for row in data.data.rows {
            let rate = row.est_funding_rate.or(row.last_funding_rate).unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 8.0);
            let symbol = row.symbol;
            let normalized = normalize_symbol(&symbol, "Orderly");

            rates.push(FundingRate {
                exchange: "Orderly".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: None,
                open_interest: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
