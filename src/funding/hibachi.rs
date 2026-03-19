use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Hibachi {
    client: Client,
}

#[derive(Deserialize)]
struct ExchangeInfo {
    #[serde(rename = "futureContracts")]
    future_contracts: Vec<Contract>,
}

#[derive(Deserialize)]
struct Contract {
    symbol: String,
    status: Option<String>,
}

#[derive(Deserialize)]
struct PriceData {
    #[serde(rename = "markPrice")]
    mark_price: Option<String>,
    #[serde(rename = "fundingRateEstimation")]
    funding_rate_estimation: Option<FundingEstimation>,
}

#[derive(Deserialize)]
struct FundingEstimation {
    #[serde(rename = "estimatedFundingRate")]
    estimated_funding_rate: Option<String>,
}

impl Hibachi {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Hibachi {
    fn name(&self) -> &str {
        "Hibachi"
    }

    // Hibachi: 8h funding period. Rate from prices endpoint is the estimated
    // next funding rate as a decimal fraction.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let info: ExchangeInfo = self
            .client
            .get("https://data-api.hibachi.xyz/market/exchange-info")
            .send()
            .await
            .map_err(FundingError::Http)?
            .json()
            .await
            .map_err(FundingError::Http)?;

        let now = Utc::now();
        let mut rates = Vec::new();

        for contract in &info.future_contracts {
            if contract.status.as_deref() == Some("CLOSED") {
                continue;
            }

            let url = format!(
                "https://data-api.hibachi.xyz/market/data/prices?symbol={}",
                urlencoding::encode(&contract.symbol)
            );

            let resp = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(_) => continue,
            };

            let price_data: PriceData = match resp.json().await {
                Ok(d) => d,
                Err(_) => continue,
            };

            let rate: f64 = price_data
                .funding_rate_estimation
                .as_ref()
                .and_then(|f| f.estimated_funding_rate.as_deref())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 8.0);
            let symbol = contract.symbol.clone();
            let normalized = normalize_symbol(&symbol, "Hibachi");

            rates.push(FundingRate {
                exchange: "Hibachi".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: price_data
                    .mark_price
                    .as_deref()
                    .and_then(|s| s.parse().ok()),
                open_interest: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
