use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Ethereal {
    client: Client,
}

#[derive(Deserialize)]
struct ProductsResponse {
    data: Vec<Product>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    ticker: Option<String>,
    display_ticker: Option<String>,
    status: Option<String>,
    funding_rate1h: Option<String>,
    open_interest: Option<String>,
}

impl Ethereal {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Ethereal {
    fn name(&self) -> &str {
        "Ethereal"
    }

    // Ethereal: rate is decimal fraction, 1h period
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://api.ethereal.trade/v1/product?order=asc&orderBy=createdAt")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: ProductsResponse = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for product in data.data {
            if product.status.as_deref() != Some("ACTIVE") {
                continue;
            }

            let rate: f64 = product
                .funding_rate1h
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 1.0);
            let symbol = product
                .display_ticker
                .or(product.ticker)
                .unwrap_or_default();
            let normalized = normalize_symbol(&symbol, "Ethereal");

            rates.push(FundingRate {
                exchange: "Ethereal".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: None,
                open_interest: product
                    .open_interest
                    .as_deref()
                    .and_then(|s| s.parse().ok()),
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
