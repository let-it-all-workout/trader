use crate::news::NewsSnapshot;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct IntelSnapshot {
    pub alerts: Vec<IntelSignal>,
    pub scoreboard: Vec<IntelSignal>,
    pub watchlists: Vec<WatchlistTrigger>,
    pub whales: Vec<WhaleSignal>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct IntelSignal {
    pub title: String,
    pub event_type: String,
    pub signal_strength: i32,
    pub novelty: i32,
    pub impact: i32,
    pub timing: i32,
    pub confidence: i32,
    pub reasons: String,
    pub source: String,
    pub published: String,
    pub region: String,
    pub focus: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct WhaleSignal {
    pub headline: String,
    pub signal_type: String,
    pub confidence: i32,
    pub reason: String,
    pub source: String,
    pub published: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct WatchlistTrigger {
    pub watchlist: String,
    pub trigger_count: usize,
    pub top_signal: String,
    pub avg_confidence: i32,
}

pub fn build_intel_snapshot(news: &NewsSnapshot) -> IntelSnapshot {
    let mut clustered: Vec<IntelSignal> = Vec::new();
    let mut seen_cluster_keys: HashSet<String> = HashSet::new();

    for item in &news.items {
        let text = format!(
            "{} {} {} {} {}",
            item.title.to_lowercase(),
            item.summary.to_lowercase(),
            item.tags.to_lowercase(),
            item.focus.to_lowercase(),
            item.region.to_lowercase()
        );

        let event_type = detect_event_type(&text);
        let signal_strength = ((item.score as f64 / 1.6).round() as i32).clamp(25, 98);
        let novelty = score_novelty(&text);
        let impact = score_impact(&text, &event_type);
        let timing = score_timing(&item.published);
        let confidence = ((signal_strength * 35 + novelty * 15 + impact * 30 + timing * 20) / 100)
            .clamp(20, 99);
        let reasons = explain_reasons(&text, &event_type, confidence);

        // Cluster similar surface headlines into one actionable signal.
        let cluster_key = format!("{}::{}", event_type, fingerprint(&item.title));
        if seen_cluster_keys.contains(&cluster_key) {
            continue;
        }
        seen_cluster_keys.insert(cluster_key);

        clustered.push(IntelSignal {
            title: item.title.clone(),
            event_type,
            signal_strength,
            novelty,
            impact,
            timing,
            confidence,
            reasons,
            source: item.source.clone(),
            published: item.published.clone(),
            region: item.region.clone(),
            focus: item.focus.clone(),
            url: item.url.clone(),
        });
    }

    clustered.sort_by(|a, b| b.confidence.cmp(&a.confidence));
    let alerts: Vec<IntelSignal> = clustered.iter().take(8).cloned().collect();
    let scoreboard: Vec<IntelSignal> = clustered.iter().take(20).cloned().collect();

    let whales: Vec<WhaleSignal> = clustered
        .iter()
        .filter(|s| is_whale_type(&s.event_type) || s.event_type == "stock-spike")
        .filter(|s| s.confidence >= 55)
        .take(10)
        .map(|s| WhaleSignal {
            headline: s.title.clone(),
            signal_type: s.event_type.clone(),
            confidence: s.confidence,
            reason: s.reasons.clone(),
            source: s.source.clone(),
            published: s.published.clone(),
            url: s.url.clone(),
        })
        .collect();

    let watchlists = build_watchlists(&scoreboard);

    IntelSnapshot {
        alerts,
        scoreboard,
        watchlists,
        whales,
        updated_at: Some(Utc::now()),
    }
}

fn detect_event_type(text: &str) -> String {
    if contains_any(text, &["director dealing", "insider", "major shareholder", "substantial holder"]) {
        return "insider-flow".to_string();
    }
    if contains_any(text, &["block trade", "cross deal", "crossing", "large volume"]) {
        return "block-trade-proxy".to_string();
    }
    if contains_any(text, &["institutional", "fund holdings", "accumulation", "distribution"]) {
        return "institutional-flow".to_string();
    }
    if contains_any(text, &["dividend", "qualification date", "closure of register"]) {
        return "dividend-alert".to_string();
    }
    if contains_any(
        text,
        &["results", "earnings", "q1", "q2", "q3", "full year", "audited", "unaudited"],
    ) {
        return "report-release".to_string();
    }
    if contains_any(text, &["spike", "surge", "rally", "jumps", "soars", "top gainer", "52-week high"]) {
        return "stock-spike".to_string();
    }
    if contains_any(text, &["ipo", "listing", "book build", "offer price"]) {
        return "ipo-catalyst".to_string();
    }
    if contains_any(text, &["policy", "regulation", "cbn", "act", "tariff", "approval"]) {
        return "policy-catalyst".to_string();
    }
    "market-intel".to_string()
}

fn score_novelty(text: &str) -> i32 {
    let mut score = 45;
    if contains_any(text, &["first", "new", "launch", "transition", "acquisition completed"]) {
        score += 20;
    }
    if contains_any(text, &["director dealing", "major shareholder", "block trade", "institutional"]) {
        score += 18;
    }
    if contains_any(text, &["rumour", "opinion"]) {
        score -= 10;
    }
    score.clamp(20, 95)
}

fn score_impact(text: &str, event_type: &str) -> i32 {
    let mut score = 50;
    score += match event_type {
        "ipo-catalyst" => 28,
        "insider-flow" => 24,
        "block-trade-proxy" => 24,
        "institutional-flow" => 22,
        "report-release" => 18,
        "dividend-alert" => 16,
        "stock-spike" => 16,
        "policy-catalyst" => 18,
        _ => 8,
    };
    if contains_any(text, &["ngx", "nigerian exchange", "nigeria"]) {
        score += 8;
    }
    if contains_any(text, &["bank", "insurance", "power", "refinery", "agriculture"]) {
        score += 8;
    }
    score.clamp(25, 98)
}

fn score_timing(published: &str) -> i32 {
    let p = published.to_lowercase();
    if p.contains("m ago") {
        return 90;
    }
    if p.contains("h ago") {
        return 78;
    }
    if p.contains("1d ago") || p.contains("2d ago") {
        return 65;
    }
    if p.contains("d ago") {
        return 45;
    }
    40
}

fn explain_reasons(text: &str, event_type: &str, confidence: i32) -> String {
    let mut parts = vec![format!("{event_type} detected")];
    if contains_any(text, &["ngx", "nigerian exchange"]) {
        parts.push("NGX-linked catalyst".to_string());
    }
    if contains_any(text, &["dividend", "qualification date"]) {
        parts.push("cash-return trigger".to_string());
    }
    if contains_any(text, &["results", "earnings", "audited", "q1", "q2", "q3"]) {
        parts.push("fresh company reporting cycle".to_string());
    }
    if contains_any(text, &["spike", "surge", "rally", "top gainer"]) {
        parts.push("momentum/flow anomaly".to_string());
    }
    parts.push(format!("confidence {confidence}%"));
    parts.join(" · ")
}

fn is_whale_type(event_type: &str) -> bool {
    matches!(
        event_type,
        "insider-flow" | "block-trade-proxy" | "institutional-flow"
    )
}

fn build_watchlists(scoreboard: &[IntelSignal]) -> Vec<WatchlistTrigger> {
    let mut groups: HashMap<String, Vec<&IntelSignal>> = HashMap::new();
    groups.insert("Event-driven NGX".to_string(), Vec::new());
    groups.insert("Nigeria/Africa Growth".to_string(), Vec::new());
    groups.insert("Global Risk-On/Off".to_string(), Vec::new());

    for s in scoreboard {
        if is_event_driven_ngx(s) {
            groups
                .entry("Event-driven NGX".to_string())
                .or_default()
                .push(s);
        }
        if is_nigeria_africa_growth(s) {
            groups
                .entry("Nigeria/Africa Growth".to_string())
                .or_default()
                .push(s);
        }
        if is_global_risk(s) {
            groups
                .entry("Global Risk-On/Off".to_string())
                .or_default()
                .push(s);
        }
    }

    let mut out = Vec::new();
    for (watchlist, signals) in groups {
        if signals.is_empty() {
            out.push(WatchlistTrigger {
                watchlist,
                trigger_count: 0,
                top_signal: "no triggers".to_string(),
                avg_confidence: 0,
            });
            continue;
        }

        let trigger_count = signals.len();
        let top_signal = signals
            .iter()
            .max_by_key(|s| s.confidence)
            .map(|s| s.event_type.clone())
            .unwrap_or_else(|| "market-intel".to_string());
        let avg_confidence =
            (signals.iter().map(|s| s.confidence).sum::<i32>() as f64 / trigger_count as f64)
                .round() as i32;
        out.push(WatchlistTrigger {
            watchlist,
            trigger_count,
            top_signal,
            avg_confidence,
        });
    }

    out.sort_by(|a, b| b.avg_confidence.cmp(&a.avg_confidence));
    out
}

fn is_event_driven_ngx(s: &IntelSignal) -> bool {
    s.region == "Nigeria"
        && matches!(
            s.event_type.as_str(),
            "ipo-catalyst"
                | "report-release"
                | "dividend-alert"
                | "insider-flow"
                | "block-trade-proxy"
                | "institutional-flow"
                | "policy-catalyst"
        )
}

fn is_nigeria_africa_growth(s: &IntelSignal) -> bool {
    (s.region == "Nigeria" || s.region == "Africa")
        && matches!(
            s.focus.as_str(),
            "Energy"
                | "Financials"
                | "Agriculture"
                | "Chips"
                | "Cloud/DataCenter"
                | "Models"
                | "Applications"
                | "Crypto"
                | "Markets"
        )
}

fn is_global_risk(s: &IntelSignal) -> bool {
    s.region == "Global"
        || matches!(s.focus.as_str(), "Crypto" | "Markets")
        || matches!(s.event_type.as_str(), "stock-spike" | "policy-catalyst")
}

fn fingerprint(title: &str) -> String {
    title
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join("-")
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}
