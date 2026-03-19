use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct DyDx {
    client: Client,
}

#[derive(Deserialize)]
struct MarketsResponse {
    markets: std::collections::HashMap<String, MarketData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarketData {
    ticker: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    next_funding_rate: Option<String>,
    #[serde(default)]
    oracle_price: Option<String>,
    #[serde(default)]
    open_interest: Option<String>,
}

impl DyDx {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for DyDx {
    fn name(&self) -> &str {
        "dYdX"
    }

    // dYdX v4: rate is decimal fraction, 1h period
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://indexer.dydx.trade/v4/perpetualMarkets")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: MarketsResponse = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for (_key, market) in data.markets {
            if market.status.as_deref() != Some("ACTIVE") {
                continue;
            }

            let oi: f64 = market
                .open_interest
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            if oi == 0.0 {
                continue;
            }

            let rate: f64 = market
                .next_funding_rate
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 1.0);

            rates.push(FundingRate {
                exchange: "dYdX".into(),
                symbol: market.ticker.clone(),
                normalized: normalize_symbol(&market.ticker, "dYdX"),
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: market.oracle_price.as_deref().and_then(|s| s.parse().ok()),
                open_interest: Some(oi),
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
