use super::{http_client, NewsArbOpportunity, PolymarketError};

const MIN_MOVE_PCT: f64 = 5.0;
const MIN_VOLUME_24H: f64 = 5000.0;

pub async fn scan_newsarb() -> Result<Vec<NewsArbOpportunity>, PolymarketError> {
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
                    tracing::warn!("newsarb: failed reading page {}: {e}", offset / 100);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("newsarb: failed fetching page {}: {e}", offset / 100);
                continue;
            }
        };
        let markets: Vec<serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("newsarb: failed parsing page {}: {e}", offset / 100);
                continue;
            }
        };
        let count = markets.len();
        all_markets.extend(markets);
        if count < 100 {
            break;
        }
    }

    let mut opps = Vec::new();

    for m in &all_markets {
        let vol_24h = as_f64(m.get("volume24hr")).unwrap_or(0.0);
        if vol_24h < MIN_VOLUME_24H {
            continue;
        }

        let current_price = match extract_yes_price(m) {
            Some(p) if p > 0.0 && p < 1.0 => p,
            _ => continue,
        };

        let price_change_pct = detect_price_move(m, current_price);
        if price_change_pct.abs() < MIN_MOVE_PCT {
            continue;
        }

        let direction = if price_change_pct > 0.0 {
            "UP"
        } else {
            "DOWN"
        };

        let question = m
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let market_id = m
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
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
        let event_title = m
            .get("groupItemTitle")
            .or_else(|| m.get("question"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let liq = as_f64(m.get("liquidity"))
            .or_else(|| as_f64(m.get("liquidityClob")))
            .unwrap_or(0.0);

        opps.push(NewsArbOpportunity {
            market_id,
            question,
            url,
            event_title,
            current_price,
            price_change_pct: price_change_pct.abs(),
            direction: direction.to_string(),
            volume_24h: vol_24h,
            liquidity: liq,
        });
    }

    opps.sort_by(|a, b| {
        b.price_change_pct
            .partial_cmp(&a.price_change_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    opps.truncate(50);

    Ok(opps)
}

fn detect_price_move(market: &serde_json::Value, current_price: f64) -> f64 {
    // Use oneDayPriceChange if available (direct percentage)
    if let Some(change) = as_f64(market.get("oneDayPriceChange")) {
        let pct = change * 100.0;
        if pct.abs() > 0.01 {
            return pct;
        }
    }

    // Fall back to comparing current vs previous day price
    if let Some(prev) = as_f64(market.get("previousDayPrice"))
        .or_else(|| as_f64(market.get("lastTradePrice")))
    {
        if prev > 0.001 {
            return ((current_price - prev) / prev) * 100.0;
        }
    }

    // Check volume-based spike: high volume relative to liquidity suggests news
    let vol = as_f64(market.get("volume24hr")).unwrap_or(0.0);
    let liq = as_f64(market.get("liquidity")).unwrap_or(1.0).max(1.0);
    let vol_ratio = vol / liq;
    if vol_ratio > 2.0 {
        let dist_from_mid = (current_price - 0.5).abs() * 100.0;
        if dist_from_mid > 15.0 {
            return dist_from_mid * vol_ratio.min(5.0) / 5.0;
        }
    }

    0.0
}

fn extract_yes_price(market: &serde_json::Value) -> Option<f64> {
    if let Some(p) = as_f64(market.get("lastTradePrice")) {
        if p > 0.0 {
            return Some(p);
        }
    }
    if let Some(bid) = as_f64(market.get("bestBid")) {
        if let Some(ask) = as_f64(market.get("bestAsk")) {
            if bid > 0.0 && ask > 0.0 {
                return Some((bid + ask) / 2.0);
            }
        }
    }
    let prices_val = market.get("outcomePrices")?;
    let prices: Vec<String> = if let Some(s) = prices_val.as_str() {
        serde_json::from_str(s).ok()?
    } else if let Some(arr) = prices_val.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        return None;
    };
    prices.first()?.parse::<f64>().ok()
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
