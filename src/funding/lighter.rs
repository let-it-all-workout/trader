use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

pub struct Lighter {
    client: Client,
}

#[derive(Deserialize)]
struct FundingResponse {
    funding_rates: Vec<LighterFundingRate>,
}

#[derive(Deserialize)]
struct LighterFundingRate {
    exchange: String,
    symbol: String,
    rate: f64,
}

impl Lighter {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Lighter {
    fn name(&self) -> &str {
        "Lighter"
    }

    // Lighter: /api/v1/funding-rates returns rates across multiple exchanges.
    // We only use exchange="lighter" entries. Rate is decimal fraction, 8h-equivalent
    // (verified: the same API reports Binance/Bybit/Hyperliquid at standard 8h rates).
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .get("https://mainnet.zklighter.elliot.ai/api/v1/funding-rates")
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(FundingError::Http)?;

        let data: FundingResponse = resp.json().await.map_err(FundingError::Http)?;
        let now = Utc::now();
        let mut rates = Vec::new();

        for fr in data.funding_rates {
            if fr.exchange != "lighter" {
                continue;
            }

            let rate_8h = to_8h_pct(fr.rate, 8.0);
            let normalized = normalize_symbol(&fr.symbol, "Lighter");

            rates.push(FundingRate {
                exchange: "Lighter".into(),
                symbol: fr.symbol,
                normalized,
                rate_raw: fr.rate,
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
