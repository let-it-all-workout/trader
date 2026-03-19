use super::{normalize_symbol, to_8h_pct, to_annual_pct, FundingError, FundingRate, FundingSource};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

pub struct Hyperliquid {
    client: Client,
}

#[derive(Deserialize)]
struct AssetCtx {
    funding: String,
    #[serde(rename = "markPx")]
    mark_px: String,
    #[serde(rename = "openInterest")]
    open_interest: String,
}

#[derive(Deserialize)]
struct UniverseItem {
    name: String,
    #[serde(default, rename = "isDelisted")]
    is_delisted: bool,
}

#[derive(Deserialize)]
struct Meta {
    universe: Vec<UniverseItem>,
}

impl Hyperliquid {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl FundingSource for Hyperliquid {
    fn name(&self) -> &str {
        "Hyperliquid"
    }

    // Hyperliquid: rate is decimal fraction, 1h period (settled hourly).
    // The rate is computed from an 8h formula but paid out every hour.
    async fn fetch_rates(&self) -> Result<Vec<FundingRate>, FundingError> {
        let resp = self
            .client
            .post("https://api.hyperliquid.xyz/info")
            .json(&json!({"type": "metaAndAssetCtxs"}))
            .send()
            .await
            .map_err(FundingError::Http)?;

        let body: serde_json::Value = resp.json().await.map_err(FundingError::Http)?;

        let arr = body
            .as_array()
            .ok_or_else(|| FundingError::Parse("expected array".into()))?;

        if arr.len() < 2 {
            return Err(FundingError::Parse("unexpected response shape".into()));
        }

        let meta: Meta = serde_json::from_value(arr[0].clone())
            .map_err(|e| FundingError::Parse(format!("meta: {e}")))?;

        let ctxs: Vec<AssetCtx> = serde_json::from_value(arr[1].clone())
            .map_err(|e| FundingError::Parse(format!("ctxs: {e}")))?;

        let now = Utc::now();
        let mut rates = Vec::new();

        for (item, ctx) in meta.universe.iter().zip(ctxs.iter()) {
            if item.is_delisted {
                continue;
            }

            let oi: f64 = ctx.open_interest.parse().unwrap_or(0.0);
            if oi == 0.0 {
                continue;
            }

            let rate: f64 = ctx.funding.parse().unwrap_or(0.0);
            let rate_8h = to_8h_pct(rate, 1.0);

            rates.push(FundingRate {
                exchange: "Hyperliquid".into(),
                symbol: item.name.clone(),
                normalized: normalize_symbol(&item.name, "Hyperliquid"),
                rate_raw: rate,
                rate_8h_pct: rate_8h,
                rate_annual_pct: to_annual_pct(rate_8h),
                mark_price: ctx.mark_px.parse().ok(),
                open_interest: Some(oi),
                timestamp: now,
            });
        }

        Ok(rates)
    }
}
