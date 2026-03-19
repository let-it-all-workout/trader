use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Vest {
    client: Client,
}

#[derive(Deserialize)]
struct TickerResponse {
    tickers: Vec<Ticker>,
}

#[derive(Deserialize)]
struct Ticker {
    symbol: String,
    #[serde(rename = "closePrice")]
    close_price: Option<String>,
}

#[derive(Deserialize)]
struct FundingEntry {
    #[serde(rename = "oneHrFundingRate")]
    one_hr_funding_rate: Option<String>,
}

impl Vest {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Vest {
    fn name(&self) -> &str {
        "Vest"
    }

    // Vest: 1h funding period. oneHrFundingRate is decimal fraction per hour.
    // Two-step: get ticker list, then fetch funding for each.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let ticker_resp: TickerResponse = self
            .client
            .get("https://server-prod.hz.vestmarkets.com/v2/ticker/24hr")
            .send()
            .await
            .map_err(FundingError::Http)?
            .json()
            .await
            .map_err(FundingError::Http)?;

        let now = Utc::now();
        let mut rates = Vec::new();

        let futures: Vec<_> = ticker_resp
            .tickers
            .iter()
            .filter(|t| t.symbol.ends_with("-PERP"))
            .map(|t| {
                let client = self.client.clone();
                let symbol = t.symbol.clone();
                let price = t.close_price.clone();
                async move {
                    let url = format!(
                        "https://server-prod.hz.vestmarkets.com/v2/funding/history?symbol={}&limit=1",
                        symbol
                    );
                    let resp = client.get(&url).send().await.ok()?;
                    let entries: Vec<FundingEntry> = resp.json().await.ok()?;
                    let entry = entries.into_iter().next()?;
                    Some((symbol, entry.one_hr_funding_rate, price))
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        for result in results.into_iter().flatten() {
            let (symbol, rate_str, price_str) = result;

            let rate: f64 = rate_str
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            let rate_8h = to_8h_pct(rate, 1.0);
            let normalized = normalize_symbol(&symbol, "Vest");

            rates.push(FundingRate {
                exchange: "Vest".into(),
                symbol,
                normalized,
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: price_str
                    .as_deref()
                    .and_then(|s| s.parse().ok()),
                open_interest: None,
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
