use super::{http_client, NegRiskOpportunity, OutcomePrice, PolymarketError};

const MIN_MISPRICING_PCT: f64 = 0.5;

pub async fn scan_negrisk() -> Result<Vec<NegRiskOpportunity>, PolymarketError> {
    let client = http_client();

    let mut all_events: Vec<serde_json::Value> = Vec::new();
    for offset in (0..300).step_by(100) {
        let url = format!(
            "https://gamma-api.polymarket.com/events?active=true&closed=false&limit=100&offset={offset}&order=volume24hr&ascending=false"
        );
        let text = match client.get(&url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!("negrisk: failed reading events page {}: {e}", offset / 100);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("negrisk: failed fetching events page {}: {e}", offset / 100);
                continue;
            }
        };
        let events: Vec<serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("negrisk: failed parsing events page {}: {e}", offset / 100);
                continue;
            }
        };
        let count = events.len();
        all_events.extend(events);
        if count < 100 {
            break;
        }
    }

    let mut opps = Vec::new();

    for event in &all_events {
        let markets = match event.get("markets").and_then(|v| v.as_array()) {
            Some(m) if m.len() >= 3 => m,
            _ => continue,
        };

        // Only process events with mutually exclusive outcomes (negRisk markets)
        let event_neg_risk = event.get("negRisk").and_then(|v| v.as_bool()).unwrap_or(false)
            || event.get("enableNegRisk").and_then(|v| v.as_bool()).unwrap_or(false);
        let market_neg_risk = markets.iter().any(|m| {
            m.get("negRisk").and_then(|v| v.as_bool()).unwrap_or(false)
        });
        if !event_neg_risk && !market_neg_risk {
            continue;
        }

        let title = event.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let event_id = event.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let slug = event.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = if slug.is_empty() {
            String::new()
        } else {
            format!("https://polymarket.com/event/{slug}")
        };
        let vol_24h = as_f64(event.get("volume24hr")).unwrap_or(0.0);
        let liq = as_f64(event.get("liquidity")).unwrap_or(0.0);

        let mut yes_sum = 0.0f64;
        let mut outcomes = Vec::new();
        let mut all_valid = true;

        for m in markets {
            if let Some(yp) = extract_yes_price(m) {
                yes_sum += yp;
                let question = m.get("groupItemTitle")
                    .or_else(|| m.get("question"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                outcomes.push(OutcomePrice { question, yes_price: yp });
            } else {
                all_valid = false;
                break;
            }
        }

        if !all_valid || outcomes.is_empty() {
            continue;
        }

        let mispricing = yes_sum - 1.0;
        let mispricing_pct = mispricing.abs() * 100.0;

        if mispricing_pct < MIN_MISPRICING_PCT {
            continue;
        }

        outcomes.sort_by(|a, b| b.yes_price.partial_cmp(&a.yes_price).unwrap_or(std::cmp::Ordering::Equal));

        opps.push(NegRiskOpportunity {
            event_id,
            event_title: title,
            url,
            num_outcomes: markets.len(),
            yes_sum,
            mispricing,
            profit_margin_pct: mispricing_pct,
            volume_24h: vol_24h,
            liquidity: liq,
            outcomes,
        });
    }

    opps.sort_by(|a, b| b.profit_margin_pct.partial_cmp(&a.profit_margin_pct).unwrap_or(std::cmp::Ordering::Equal));
    opps.truncate(50);

    Ok(opps)
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
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
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
