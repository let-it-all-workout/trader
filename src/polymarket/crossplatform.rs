use super::{http_client, CrossPlatformOpportunity, PolymarketError};
use std::collections::HashSet;

const MIN_SPREAD_PCT: f64 = 2.0;
const MIN_SIMILARITY: f64 = 0.30;

const STOP_WORDS: &[&str] = &[
    "will", "the", "be", "by", "a", "an", "of", "in", "to", "is", "it", "and", "or", "on", "at",
    "for", "as", "before", "after", "next", "first", "has", "have", "been", "are", "was", "were",
    "do", "does", "did", "not", "no", "yes", "this", "that", "from", "with", "its", "their",
    "who", "what", "when", "where", "how", "which", "than", "then", "there", "here", "more",
    "most", "least", "between", "into", "out", "up", "down", "over", "under", "about", "above",
    "each", "every", "any", "all", "both", "other", "some", "such", "only", "same", "so",
];

pub async fn scan_crossplatform() -> Result<Vec<CrossPlatformOpportunity>, PolymarketError> {
    let client = http_client();

    let (poly_markets, kalshi_markets) =
        tokio::join!(fetch_polymarket_markets(&client), fetch_kalshi_markets(&client));

    let poly = poly_markets.unwrap_or_default();
    let kalshi = kalshi_markets.unwrap_or_default();

    tracing::info!(
        "crossplatform: poly={} kalshi={} — starting matching",
        poly.len(),
        kalshi.len()
    );

    if poly.is_empty() || kalshi.is_empty() {
        return Ok(Vec::new());
    }

    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();

    let mut opps = Vec::new();

    for pm in &poly {
        let pm_kw = extract_keywords(&pm.title_norm, &stop);
        if pm_kw.len() < 2 {
            continue;
        }

        let mut best_match: Option<(f64, &KalshiMarket)> = None;

        for km in &kalshi {
            let km_kw = extract_keywords(&km.title_norm, &stop);
            if km_kw.len() < 2 {
                continue;
            }

            let sim = keyword_similarity(&pm_kw, &km_kw);
            if sim < MIN_SIMILARITY {
                continue;
            }

            match &best_match {
                Some((best_sim, _)) if sim <= *best_sim => {}
                _ => best_match = Some((sim, km)),
            }
        }

        let (sim, km) = match best_match {
            Some(m) => m,
            None => continue,
        };

        let spread = (pm.yes_price - km.yes_price).abs();
        let mid = (pm.yes_price + km.yes_price) / 2.0;
        if mid <= 0.0 {
            continue;
        }
        let spread_pct = (spread / mid) * 100.0;

        if spread_pct < MIN_SPREAD_PCT {
            continue;
        }

        let (buy_platform, sell_platform) = if pm.yes_price < km.yes_price {
            ("Polymarket".to_string(), "Kalshi".to_string())
        } else {
            ("Kalshi".to_string(), "Polymarket".to_string())
        };

        tracing::debug!(
            "crossplatform match (sim={sim:.2}): PM=\"{}\" <-> KS=\"{}\" spread={spread_pct:.1}%",
            &pm.title[..pm.title.len().min(50)],
            &km.title[..km.title.len().min(50)]
        );

        opps.push(CrossPlatformOpportunity {
            event_title: pm.title.clone(),
            polymarket_question: pm.question.clone(),
            polymarket_url: pm.url.clone(),
            kalshi_ticker: km.ticker.clone(),
            kalshi_url: km.url.clone(),
            polymarket_yes: pm.yes_price,
            kalshi_yes: km.yes_price,
            spread_pct,
            buy_platform,
            sell_platform,
            polymarket_volume: pm.volume_24h,
        });
    }

    opps.sort_by(|a, b| {
        b.spread_pct
            .partial_cmp(&a.spread_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    opps.dedup_by(|a, b| a.polymarket_question == b.polymarket_question);
    opps.truncate(50);

    tracing::info!("crossplatform: {} opportunities found", opps.len());

    Ok(opps)
}

struct PolyMarket {
    title: String,
    title_norm: String,
    question: String,
    url: String,
    yes_price: f64,
    volume_24h: f64,
}

struct KalshiMarket {
    title: String,
    title_norm: String,
    ticker: String,
    url: String,
    yes_price: f64,
}

async fn fetch_polymarket_markets(
    client: &reqwest::Client,
) -> Result<Vec<PolyMarket>, PolymarketError> {
    let mut all: Vec<PolyMarket> = Vec::new();
    for offset in (0..500).step_by(100) {
        let url = format!(
            "https://gamma-api.polymarket.com/markets?active=true&closed=false&limit=100&offset={offset}&order=volume24hr&ascending=false"
        );
        let text = match client.get(&url).send().await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("crossplatform: poly fetch page {} err: {e}", offset / 100);
                continue;
            }
        };
        let markets: Vec<serde_json::Value> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let count = markets.len();
        for m in markets {
            let question = m
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = m
                .get("groupItemTitle")
                .or_else(|| m.get("question"))
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
            let yes_price = extract_yes_price(&m).unwrap_or(0.0);
            let vol = as_f64(m.get("volume24hr")).unwrap_or(0.0);
            if yes_price > 0.0 {
                all.push(PolyMarket {
                    title_norm: normalize(&title),
                    title,
                    question,
                    url,
                    yes_price,
                    volume_24h: vol,
                });
            }
        }
        if count < 100 {
            break;
        }
    }
    Ok(all)
}

async fn fetch_kalshi_markets(
    client: &reqwest::Client,
) -> Result<Vec<KalshiMarket>, PolymarketError> {
    let mut all: Vec<KalshiMarket> = Vec::new();
    let mut cursor = String::new();

    for _ in 0..3 {
        let url = if cursor.is_empty() {
            "https://api.elections.kalshi.com/trade-api/v2/events?limit=200&status=open&with_nested_markets=true".to_string()
        } else {
            format!(
                "https://api.elections.kalshi.com/trade-api/v2/events?limit=200&status=open&with_nested_markets=true&cursor={cursor}"
            )
        };

        let text = match client.get(&url).send().await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(e) => {
                tracing::warn!("crossplatform: kalshi fetch err: {e}");
                break;
            }
        };
        let val: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("crossplatform: kalshi parse err: {e}");
                break;
            }
        };

        let events = val
            .get("events")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let count = events.len();

        for event in events {
            let markets = event
                .get("markets")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            for m in markets {
                let title = m
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ticker = m
                    .get("ticker")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let yes_price = kalshi_price(&m);

                if yes_price > 0.01 && yes_price < 0.99 {
                    let kalshi_url = format!("https://kalshi.com/markets/{ticker}");
                    all.push(KalshiMarket {
                        title_norm: normalize(&title),
                        title,
                        ticker,
                        url: kalshi_url,
                        yes_price,
                    });
                }
            }
        }

        cursor = val
            .get("cursor")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if cursor.is_empty() || count < 200 {
            break;
        }
    }

    tracing::info!("crossplatform: fetched {} kalshi markets", all.len());
    Ok(all)
}

fn kalshi_price(m: &serde_json::Value) -> f64 {
    // Try _dollars fields first (already 0.0-1.0 range)
    for field in ["last_price_dollars", "yes_bid_dollars", "yes_ask_dollars"] {
        if let Some(v) = m.get(field) {
            let f = if let Some(n) = v.as_f64() {
                n
            } else if let Some(s) = v.as_str() {
                s.parse::<f64>().unwrap_or(0.0)
            } else {
                0.0
            };
            if f > 0.001 {
                return f;
            }
        }
    }
    // Fall back to cent-denominated fields
    for field in ["last_price", "yes_bid", "yes_ask"] {
        if let Some(v) = m.get(field) {
            let cents = if let Some(n) = v.as_f64() {
                n
            } else if let Some(s) = v.as_str() {
                s.parse::<f64>().unwrap_or(0.0)
            } else {
                0.0
            };
            if cents > 0.0 {
                return if cents > 1.0 { cents / 100.0 } else { cents };
            }
        }
    }
    0.0
}

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_keywords<'a>(normalized: &'a str, stop: &HashSet<&str>) -> HashSet<&'a str> {
    normalized
        .split_whitespace()
        .filter(|w| w.len() >= 3 && !stop.contains(w))
        .collect()
}

fn keyword_similarity(a: &HashSet<&str>, b: &HashSet<&str>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count();
    if intersection < 2 {
        return 0.0;
    }
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        return 0.0;
    }
    intersection as f64 / union
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
