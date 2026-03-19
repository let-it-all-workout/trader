use super::{http_client, EndgameOpportunity, PolymarketError};
use chrono::Utc;

const MIN_PROBABILITY: f64 = 0.90;
const MAX_DAYS: i64 = 30;
const MIN_VOLUME_24H: f64 = 1000.0;

pub async fn scan_endgame() -> Result<Vec<EndgameOpportunity>, PolymarketError> {
    let client = http_client();

    let mut all_markets: Vec<serde_json::Value> = Vec::new();
    for offset in (0..500).step_by(100) {
        let url = format!(
            "https://gamma-api.polymarket.com/markets?active=true&closed=false&limit=100&offset={offset}&order=volume24hr&ascending=false"
        );
        let text = match client.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("endgame: failed reading markets page {}: {e}", offset / 100);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("endgame: failed fetching markets page {}: {e}", offset / 100);
                continue;
            }
        };
        let markets: Vec<serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("endgame: failed parsing markets page {}: {e}", offset / 100);
                continue;
            }
        };
        let count = markets.len();
        all_markets.extend(markets);
        if count < 100 {
            break;
        }
    }

    let now = Utc::now();
    let mut opps = Vec::new();

    for m in &all_markets {
        let (yes_price, no_price) = match extract_prices(m) {
            Some(p) => p,
            None => continue,
        };

        if yes_price <= 0.0 || yes_price >= 1.0 || no_price <= 0.0 || no_price >= 1.0 {
            continue;
        }

        let high_prob = yes_price.max(no_price);
        if high_prob < MIN_PROBABILITY {
            continue;
        }

        let end_date_str = match m.get("endDate").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };

        let end_dt = match parse_datetime(end_date_str) {
            Some(dt) => dt,
            None => continue,
        };

        let days_left = (end_dt - now).num_days();
        if days_left <= 0 || days_left > MAX_DAYS {
            continue;
        }

        let vol_24h = as_f64(m.get("volume24hr")).unwrap_or(0.0);
        if vol_24h < MIN_VOLUME_24H {
            continue;
        }

        let (side, buy_price) = if yes_price >= no_price {
            ("YES", yes_price)
        } else {
            ("NO", no_price)
        };

        let profit_pct = (1.0 / buy_price - 1.0) * 100.0;
        let annualized = profit_pct * (365.0 / days_left as f64);

        if profit_pct < 0.1 {
            continue;
        }

        let question = m.get("question").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let market_id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let slug = m.get("events")
            .and_then(|v| v.as_array())
            .and_then(|events| events.first())
            .and_then(|e| e.get("slug"))
            .and_then(|v| v.as_str())
            .or_else(|| m.get("slug").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();
        let url = if slug.is_empty() {
            String::new()
        } else {
            format!("https://polymarket.com/event/{slug}")
        };
        let liq = as_f64(m.get("liquidity")).or_else(|| as_f64(m.get("liquidityClob"))).unwrap_or(0.0);

        opps.push(EndgameOpportunity {
            market_id,
            question,
            url,
            side: side.to_string(),
            buy_price,
            profit_pct,
            days_to_resolution: days_left,
            annualized_return: annualized,
            volume_24h: vol_24h,
            liquidity: liq,
            end_date: end_date_str.to_string(),
        });
    }

    opps.sort_by(|a, b| {
        b.annualized_return
            .partial_cmp(&a.annualized_return)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    opps.truncate(50);

    Ok(opps)
}

fn extract_prices(market: &serde_json::Value) -> Option<(f64, f64)> {
    if let Some(last) = as_f64(market.get("lastTradePrice")) {
        if last > 0.0 && last < 1.0 {
            return Some((last, 1.0 - last));
        }
    }
    if let Some(bid) = as_f64(market.get("bestBid")) {
        if let Some(ask) = as_f64(market.get("bestAsk")) {
            if bid > 0.0 && ask > 0.0 {
                let mid = (bid + ask) / 2.0;
                return Some((mid, 1.0 - mid));
            }
        }
    }
    let prices_val = market.get("outcomePrices")?;
    let prices: Vec<String> = if let Some(s) = prices_val.as_str() {
        serde_json::from_str(s).ok()?
    } else if let Some(arr) = prices_val.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        return None;
    };

    let yes = prices.first()?.parse::<f64>().ok()?;
    let no = prices.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0 - yes);
    Some((yes, no))
}

fn parse_datetime(s: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|dt| dt.and_utc())
        })
}

fn as_f64(val: Option<&serde_json::Value>) -> Option<f64> {
    let v = val?;
    if let Some(f) = v.as_f64() {
        return Some(f);
    }
    if let Some(s) = v.as_str() {
        return s.parse::<f64>().ok();
    }
    None
}
