use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;

pub struct Aster {
    client: Client,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PremiumIndexEntry {
    symbol: String,
    mark_price: Option<String>,
    last_funding_rate: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FundingInfoEntry {
    symbol: String,
    funding_interval_hours: Option<f64>,
}

impl Aster {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Aster {
    fn name(&self) -> &str {
        "Aster"
    }

    // Aster: rate is decimal fraction. Funding interval varies per symbol
    // (1h, 2h, 4h, or 8h) — fetched from /fapi/v3/fundingInfo.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let info_resp = self
            .client
            .get("https://fapi.asterdex.com/fapi/v3/fundingInfo")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let info: Vec<FundingInfoEntry> = info_resp.json().await.map_err(FundingError::Http)?;
        let intervals: HashMap<String, f64> = info
            .into_iter()
            .map(|e| (e.symbol, e.funding_interval_hours.unwrap_or(8.0)))
            .collect();

        let resp = self
            .client
            .get("https://fapi.asterdex.com/fapi/v3/premiumIndex")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: Vec<PremiumIndexEntry> = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for entry in data {
            let rate: f64 = entry
                .last_funding_rate
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let period_h = intervals.get(&entry.symbol).copied().unwrap_or(8.0);
            let rate_8h = to_8h_pct(rate, period_h);
            let symbol = entry.symbol;
            let normalized = normalize_symbol(&symbol, "Aster");

            rates.push(FundingRate {
                exchange: "Aster".into(),
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
