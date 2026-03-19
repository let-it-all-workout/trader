use super::{normalize_symbol, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Drift {
    client: Client,
}

#[derive(Deserialize)]
struct DriftResponse {
    markets: Vec<MarketStats>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarketStats {
    symbol: String,
    #[serde(default)]
    market_type: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    funding_rate: Option<FundingRateData>,
    #[serde(default)]
    oracle_price: Option<String>,
    #[serde(default)]
    open_interest: Option<OpenInterest>,
}

#[derive(Deserialize)]
struct FundingRateData {
    long: Option<String>,
}

#[derive(Deserialize)]
struct OpenInterest {
    long: Option<String>,
    short: Option<String>,
}

impl Drift {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Drift {
    fn name(&self) -> &str {
        "Drift"
    }

    // Drift: rate is already a PERCENTAGE per hour (not decimal fraction).
    // Docs: "expressed as percentages in decimal form (e.g., 0.00460%)"
    // So 0.000286 means 0.000286% per hour, NOT 0.0286%.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://data.api.drift.trade/stats/markets")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: DriftResponse = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for market in data.markets {
            if market.market_type.as_deref() != Some("perp") {
                continue;
            }

            if market.status.as_deref() != Some("active") {
                continue;
            }

            let oi: f64 = market
                .open_interest
                .as_ref()
                .and_then(|o| {
                    let long: f64 = o.long.as_deref()?.parse().ok()?;
                    let short: f64 = o.short.as_deref()?.parse().ok()?;
                    Some(long + short.abs())
                })
                .unwrap_or(0.0);

            if oi == 0.0 {
                continue;
            }

            let rate: f64 = market
                .funding_rate
                .as_ref()
                .and_then(|fr| fr.long.as_deref())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            // rate is already percentage per hour → convert to 8h percentage
            let rate_8h = rate * 8.0;

            let oracle: Option<f64> = market.oracle_price.as_deref().and_then(|s| s.parse().ok());
            let symbol = market.symbol;
            let normalized = normalize_symbol(&symbol, "Drift");

            rates.push(FundingRate {
                exchange: "Drift".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: oracle,
                open_interest: Some(oi),
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
