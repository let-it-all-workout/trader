use super::{http_client, CorrelationDetail, CorrelationOpportunity, PolymarketError};

const MIN_INCONSISTENCY_PCT: f64 = 3.0;

pub async fn scan_correlations() -> Result<Vec<CorrelationOpportunity>, PolymarketError> {
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
                    tracing::warn!("correlation: failed reading events page {}: {e}", offset / 100);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("correlation: failed fetching events page {}: {e}", offset / 100);
                continue;
            }
        };
        let events: Vec<serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("correlation: failed parsing events page {}: {e}", offset / 100);
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
            Some(m) => m,
            None => continue,
        };

        let title = event.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let event_id = event.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let slug = event.get("slug").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = if slug.is_empty() {
            String::new()
        } else {
            format!("https://polymarket.com/event/{slug}")
        };

        // Multi-outcome events: sum of all YES prices should ~= 1.0
        if markets.len() >= 3 {
            let mut yes_sum = 0.0f64;
            let mut details = Vec::new();
            let mut valid = true;

            for m in markets {
                if let Some(yp) = extract_yes_price(m) {
                    yes_sum += yp;
                    let question = m.get("groupItemTitle")
                        .or_else(|| m.get("question"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    details.push(CorrelationDetail { question, yes_price: yp });
                } else {
                    valid = false;
                    break;
                }
            }

            if !valid {
                continue;
            }

            let inconsistency = (yes_sum - 1.0).abs() * 100.0;
            if inconsistency >= MIN_INCONSISTENCY_PCT {
                details.sort_by(|a, b| b.yes_price.partial_cmp(&a.yes_price).unwrap_or(std::cmp::Ordering::Equal));

                let n = markets.len() as f64;
                let roi_pct = if yes_sum > 1.0 {
                    let cost = n - yes_sum;
                    if cost > 0.0 { (yes_sum - 1.0) / cost * 100.0 } else { 0.0 }
                } else {
                    if yes_sum > 0.0 { (1.0 - yes_sum) / yes_sum * 100.0 } else { 0.0 }
                };

                opps.push(CorrelationOpportunity {
                    event_title: title.clone(),
                    event_id: event_id.clone(),
                    url: url.clone(),
                    parent_question: title.clone(),
                    parent_yes_price: yes_sum,
                    child_sum: yes_sum,
                    inconsistency_pct: inconsistency,
                    roi_pct,
                    num_children: markets.len(),
                    details,
                });
            }
        }

        // For individual 2-outcome markets: YES + NO should ~= 1.0
        for m in markets {
            let (yes, no) = match (extract_yes_price(m), extract_no_price(m)) {
                (Some(y), Some(n)) => (y, n),
                _ => continue,
            };
            let sum = yes + no;
            let inconsistency = (sum - 1.0).abs() * 100.0;
            if inconsistency >= MIN_INCONSISTENCY_PCT {
                let question = m.get("question")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let roi_pct = if sum > 1.0 {
                    let cost = 2.0 - sum;
                    if cost > 0.0 { (sum - 1.0) / cost * 100.0 } else { 0.0 }
                } else {
                    if sum > 0.0 { (1.0 - sum) / sum * 100.0 } else { 0.0 }
                };

                opps.push(CorrelationOpportunity {
                    event_title: title.clone(),
                    event_id: event_id.clone(),
                    url: url.clone(),
                    parent_question: question,
                    parent_yes_price: yes,
                    child_sum: sum,
                    inconsistency_pct: inconsistency,
                    roi_pct,
                    num_children: 2,
                    details: vec![
                        CorrelationDetail { question: "YES".to_string(), yes_price: yes },
                        CorrelationDetail { question: "NO".to_string(), yes_price: no },
                    ],
                });
            }
        }
    }

    opps.sort_by(|a, b| b.inconsistency_pct.partial_cmp(&a.inconsistency_pct).unwrap_or(std::cmp::Ordering::Equal));
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

fn extract_no_price(market: &serde_json::Value) -> Option<f64> {
    let prices_val = market.get("outcomePrices")?;
    let prices: Vec<String> = if let Some(s) = prices_val.as_str() {
        serde_json::from_str(s).ok()?
    } else if let Some(arr) = prices_val.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        return None;
    };
    prices.get(1)?.parse::<f64>().ok()
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
