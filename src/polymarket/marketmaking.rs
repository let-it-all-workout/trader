use super::{http_client, MarketMakingOpportunity, PolymarketError};

const MIN_SPREAD_PCT: f64 = 0.5;
const MIN_VOLUME_24H: f64 = 500.0;

pub async fn scan_marketmaking() -> Result<Vec<MarketMakingOpportunity>, PolymarketError> {
    let client = http_client();

    let mut all_markets: Vec<serde_json::Value> = Vec::new();
    for offset in (0..300).step_by(100) {
        let url = format!(
            "https://gamma-api.polymarket.com/markets?active=true&closed=false&limit=100&offset={offset}&order=volume24hr&ascending=false"
        );
        let text = match client.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("marketmaking: failed reading page {}: {e}", offset / 100);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("marketmaking: failed fetching page {}: {e}", offset / 100);
                continue;
            }
        };
        let markets: Vec<serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("marketmaking: failed parsing page {}: {e}", offset / 100);
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

        let (best_bid, best_ask) = match extract_bid_ask(m) {
            Some(p) => p,
            None => continue,
        };

        if best_bid <= 0.0 || best_ask <= 0.0 || best_ask <= best_bid {
            continue;
        }

        let spread = best_ask - best_bid;
        let mid_price = (best_bid + best_ask) / 2.0;
        let spread_pct = (spread / mid_price) * 100.0;

        if spread_pct < MIN_SPREAD_PCT {
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
        let liq = as_f64(m.get("liquidity"))
            .or_else(|| as_f64(m.get("liquidityClob")))
            .unwrap_or(0.0);

        opps.push(MarketMakingOpportunity {
            market_id,
            question,
            url,
            best_bid,
            best_ask,
            spread_pct,
            mid_price,
            volume_24h: vol_24h,
            liquidity: liq,
            bid_depth: 0.0,
            ask_depth: 0.0,
        });
    }

    opps.sort_by(|a, b| {
        b.spread_pct
            .partial_cmp(&a.spread_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    opps.truncate(50);

    Ok(opps)
}

fn extract_bid_ask(market: &serde_json::Value) -> Option<(f64, f64)> {
    let best_bid = as_f64(market.get("bestBid"))?;
    let best_ask = as_f64(market.get("bestAsk"))?;
    if best_bid > 0.0 && best_ask > 0.0 {
        return Some((best_bid, best_ask));
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
    let yes_price = prices.first()?.parse::<f64>().ok()?;
    let bid = (yes_price - 0.01).max(0.01);
    let ask = (yes_price + 0.01).min(0.99);
    Some((bid, ask))
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
