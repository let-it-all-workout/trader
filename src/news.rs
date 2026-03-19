use chrono::{DateTime, Utc};
use reqwest::Client;
use rss::Channel;
use serde::Deserialize;
use std::collections::HashSet;
use urlencoding::encode;
use tokio::process::Command;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NewsSourceConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NewsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_refresh_secs")]
    pub refresh_secs: u64,
    #[serde(default)]
    pub sources: Vec<NewsSourceConfig>,
    #[serde(default)]
    pub extra_keywords: Vec<String>,
    #[serde(default)]
    pub google_queries: Vec<String>,
    #[serde(default)]
    pub brave: BraveSearchConfig,
    #[serde(default)]
    pub twitter_scan: TwitterScanConfig,
    #[serde(default)]
    pub twitter_browser: TwitterBrowserConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BraveSearchConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TwitterScanConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_nitter_base")]
    pub nitter_rss_base: String,
    #[serde(default)]
    pub nitter_rss_bases: Vec<String>,
    #[serde(default)]
    pub queries: Vec<String>,
}

impl Default for TwitterScanConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            nitter_rss_base: default_nitter_base(),
            nitter_rss_bases: Vec::new(),
            queries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TwitterBrowserConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_twitter_collector_script")]
    pub script_path: String,
    #[serde(default = "default_twitter_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub queries: Vec<String>,
    #[serde(default = "default_twitter_headless")]
    pub headless: bool,
    #[serde(default = "default_twitter_limit")]
    pub per_query_limit: usize,
}

impl Default for TwitterBrowserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            script_path: default_twitter_collector_script(),
            base_url: default_twitter_base_url(),
            queries: Vec::new(),
            headless: default_twitter_headless(),
            per_query_limit: default_twitter_limit(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct BraveApiResponse {
    #[serde(default)]
    web: BraveWeb,
}

#[derive(Debug, Default, Deserialize)]
struct BraveWeb {
    #[serde(default)]
    results: Vec<BraveResult>,
}

#[derive(Debug, Default, Deserialize)]
struct BraveResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    age: String,
}

#[derive(Debug, Default, Deserialize)]
struct BrowserTweet {
    #[serde(default)]
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    published: String,
    #[serde(default)]
    query: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewsSnapshot {
    pub items: Vec<NewsItem>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewsItem {
    pub title: String,
    pub summary: String,
    pub source: String,
    pub url: String,
    pub image_url: String,
    pub published: String,
    pub region: String,
    pub focus: String,
    pub tags: String,
    pub score: i32,
}

#[derive(Debug, Clone)]
struct RankedItem {
    item: NewsItem,
    published_at: Option<DateTime<Utc>>,
}

pub async fn fetch_news_snapshot(
    client: &Client,
    cfg: &NewsConfig,
    max_items: usize,
) -> Result<NewsSnapshot, String> {
    let mut all_items: Vec<RankedItem> = Vec::new();
    let sources = effective_sources(cfg);

    for source in sources {
        if !source.enabled || source.url.is_empty() {
            continue;
        }
        let text = match client.get(&source.url).send().await {
            Ok(resp) => match resp.text().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("news: failed to read source {}: {e}", source.name);
                    continue;
                }
            },
            Err(e) => {
                tracing::warn!("news: failed to fetch source {}: {e}", source.name);
                continue;
            }
        };

        let channel = match Channel::read_from(text.as_bytes()) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("news: source is not valid RSS {}: {e}", source.name);
                continue;
            }
        };

        for entry in channel.items() {
            if let Some(item) = build_item(entry, &source.name, source.priority, cfg) {
                all_items.push(item);
            }
        }
    }

    // Brave Search integration for broader alpha discovery.
    if cfg.brave.enabled {
        let brave_items = fetch_brave_items(client, cfg).await;
        all_items.extend(brave_items);
    }

    // Twitter/X alpha scan via RSS bridge (nitter-compatible search endpoint).
    if cfg.twitter_scan.enabled {
        let twitter_items = fetch_twitter_scan_items(client, cfg).await;
        all_items.extend(twitter_items);
    }

    // Twitter browser collector (direct page scrolling).
    if cfg.twitter_browser.enabled {
        let browser_items = fetch_twitter_browser_items(cfg).await;
        all_items.extend(browser_items);
    }

    let mut seen_links: HashSet<String> = HashSet::new();
    let mut seen_titles: HashSet<String> = HashSet::new();
    all_items.sort_by(|a, b| {
        b.item
            .score
            .cmp(&a.item.score)
            .then_with(|| b.published_at.cmp(&a.published_at))
    });

    let mut deduped = Vec::new();
    for ranked in all_items {
        let url_key = ranked.item.url.trim().to_lowercase();
        let title_key = ranked.item.title.trim().to_lowercase();
        if url_key.is_empty() || title_key.is_empty() {
            continue;
        }
        if seen_links.contains(&url_key) || seen_titles.contains(&title_key) {
            continue;
        }
        seen_links.insert(url_key);
        seen_titles.insert(title_key);
        deduped.push(ranked.item);
        if deduped.len() >= max_items {
            break;
        }
    }

    Ok(NewsSnapshot {
        items: deduped,
        updated_at: Some(Utc::now()),
    })
}

async fn fetch_brave_items(client: &Client, cfg: &NewsConfig) -> Vec<RankedItem> {
    if cfg.brave.api_key.trim().is_empty() || cfg.brave.queries.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for q in &cfg.brave.queries {
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count=10",
            encode(q)
        );
        let resp = match client
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", cfg.brave.api_key.trim())
            .send()
            .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("news: brave query failed `{}`: {e}", q);
                continue;
            }
        };

        let parsed: BraveApiResponse = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("news: brave parse failed `{}`: {e}", q);
                continue;
            }
        };

        for r in parsed.web.results {
            if r.title.trim().is_empty() || r.url.trim().is_empty() {
                continue;
            }
            let summary = truncate(&strip_html(r.description.trim()), 220);
            let text_blob = format!(
                "{} {} {}",
                r.title.to_lowercase(),
                summary.to_lowercase(),
                "brave search".to_string()
            );
            let region = detect_region(&text_blob);
            let focus = detect_focus(&text_blob);
            let tags = detect_tags(&text_blob, cfg);
            let score = compute_score(&text_blob, 13, &region, &focus, None, cfg);

            out.push(RankedItem {
                item: NewsItem {
                    title: r.title.trim().to_string(),
                    summary,
                    source: "Brave Search".to_string(),
                    url: r.url.trim().to_string(),
                    image_url: String::new(),
                    published: if r.age.trim().is_empty() {
                        "recent".to_string()
                    } else {
                        r.age.trim().to_string()
                    },
                    region,
                    focus,
                    tags,
                    score,
                },
                published_at: None,
            });
        }
    }
    out
}

async fn fetch_twitter_scan_items(client: &Client, cfg: &NewsConfig) -> Vec<RankedItem> {
    if cfg.twitter_scan.queries.is_empty() {
        return Vec::new();
    }

    let mut bases: Vec<String> = cfg
        .twitter_scan
        .nitter_rss_bases
        .iter()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let primary = cfg.twitter_scan.nitter_rss_base.trim().trim_end_matches('/').to_string();
    if !primary.is_empty() && !bases.iter().any(|b| b == &primary) {
        bases.insert(0, primary);
    }
    if bases.is_empty() {
        bases.push(default_nitter_base());
    }

    let mut out = Vec::new();

    for q in &cfg.twitter_scan.queries {
        let mut channel_opt: Option<Channel> = None;
        for base in &bases {
            let url = format!("{base}/search/rss?f=tweets&q={}", encode(q));
            let text = match client.get(&url).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("news: twitter-scan read failed `{}` on {}: {e}", q, base);
                        continue;
                    }
                },
                Err(e) => {
                    tracing::warn!("news: twitter-scan fetch failed `{}` on {}: {e}", q, base);
                    continue;
                }
            };

            match Channel::read_from(text.as_bytes()) {
                Ok(ch) => {
                    channel_opt = Some(ch);
                    break;
                }
                Err(e) => {
                    tracing::warn!("news: twitter-scan rss parse failed `{}` on {}: {e}", q, base);
                }
            }
        }

        let channel = match channel_opt {
            Some(c) => c,
            None => continue,
        };

        for entry in channel.items().iter().take(12) {
            if let Some(mut item) = build_item(entry, "Twitter Alpha Scan", 14, cfg) {
                // Slight boost for social-first alpha leads.
                item.item.score += 6;
                out.push(item);
            }
        }
    }
    out
}

async fn fetch_twitter_browser_items(cfg: &NewsConfig) -> Vec<RankedItem> {
    if cfg.twitter_browser.queries.is_empty() {
        return Vec::new();
    }

    let payload = serde_json::json!({
        "base_url": cfg.twitter_browser.base_url,
        "queries": cfg.twitter_browser.queries,
        "headless": cfg.twitter_browser.headless,
        "per_query_limit": cfg.twitter_browser.per_query_limit,
    });

    let output = match Command::new("node")
        .arg(&cfg.twitter_browser.script_path)
        .arg(payload.to_string())
        .output()
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("news: twitter-browser collector launch failed: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("news: twitter-browser collector returned error: {}", err.trim());
        return Vec::new();
    }

    let out = String::from_utf8_lossy(&output.stdout);
    let parsed: Vec<BrowserTweet> = match serde_json::from_str(&out) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("news: twitter-browser collector JSON parse failed: {e}");
            return Vec::new();
        }
    };

    let mut items = Vec::new();
    for t in parsed {
        if t.title.trim().is_empty() || t.url.trim().is_empty() {
            continue;
        }
        let summary = if t.summary.trim().is_empty() {
            truncate(&t.title, 220)
        } else {
            truncate(&strip_html(t.summary.trim()), 220)
        };
        let text_blob = format!(
            "{} {} {} {}",
            t.title.to_lowercase(),
            summary.to_lowercase(),
            "twitter browser alpha".to_string(),
            t.query.to_lowercase(),
        );
        let region = detect_region(&text_blob);
        let focus = detect_focus(&text_blob);
        let tags = detect_tags(&text_blob, cfg);
        let mut score = compute_score(&text_blob, 18, &region, &focus, None, cfg);
        score += 10; // prioritize fresh social-first alpha

        items.push(RankedItem {
            item: NewsItem {
                title: t.title,
                summary,
                source: "Twitter Browser Alpha".to_string(),
                url: t.url,
                image_url: String::new(),
                published: if t.published.trim().is_empty() {
                    "recent".to_string()
                } else {
                    t.published
                },
                region,
                focus,
                tags,
                score,
            },
            published_at: None,
        });
    }
    items
}

fn build_item(
    entry: &rss::Item,
    source_name: &str,
    source_priority: i32,
    cfg: &NewsConfig,
) -> Option<RankedItem> {
    let title = entry.title()?.trim().to_string();
    let url = entry.link()?.trim().to_string();
    let description_raw = entry.description().unwrap_or_default().trim().to_string();
    let summary = truncate(&strip_html(&description_raw), 220);
    let image_url = extract_image(entry, &description_raw);
    let published_at = parse_date(entry.pub_date());
    let published = format_relative_time(published_at);

    let text_blob = format!(
        "{} {} {}",
        title.to_lowercase(),
        summary.to_lowercase(),
        source_name.to_lowercase()
    );
    let region = detect_region(&text_blob);
    let focus = detect_focus(&text_blob);
    let tags = detect_tags(&text_blob, cfg);
    let score = compute_score(&text_blob, source_priority, &region, &focus, published_at, cfg);

    Some(RankedItem {
        item: NewsItem {
            title,
            summary,
            source: source_name.to_string(),
            url,
            image_url,
            published,
            region,
            focus,
            tags,
            score,
        },
        published_at,
    })
}

fn compute_score(
    text_blob: &str,
    source_priority: i32,
    region: &str,
    focus: &str,
    published_at: Option<DateTime<Utc>>,
    cfg: &NewsConfig,
) -> i32 {
    let mut score = source_priority;

    score += match region {
        "Nigeria" => 35,
        "Africa" => 25,
        _ => 10,
    };

    score += match focus {
        "Energy" | "Chips" | "Cloud/DataCenter" | "Models" | "Applications" => 15,
        _ => 10,
    };

    for needle in catalyst_keywords() {
        if text_blob.contains(needle) {
            score += 7;
        }
    }
    if contains_any(
        text_blob,
        &[
            "ngx",
            "nigerian exchange",
            "uk exchange",
            "london stock exchange",
            "lse",
            "dual listing",
            "cross listing",
            "financial sector",
            "banking",
            "insurance",
            "stock price",
            "share price",
        ],
    ) {
        score += 10;
    }
    if contains_any(
        text_blob,
        &[
            "earnings report",
            "results",
            "q1",
            "q2",
            "q3",
            "full year",
            "audited",
            "unaudited",
        ],
    ) {
        score += 10;
    }
    if contains_any(
        text_blob,
        &[
            "dividend",
            "qualification date",
            "closure of register",
            "dividend declaration",
        ],
    ) {
        score += 10;
    }
    if contains_any(
        text_blob,
        &[
            "spike",
            "surge",
            "rally",
            "jumps",
            "soars",
            "top gainer",
            "52-week high",
        ],
    ) {
        score += 9;
    }
    for kw in &cfg.extra_keywords {
        if !kw.trim().is_empty() && text_blob.contains(&kw.to_lowercase()) {
            score += 4;
        }
    }

    if let Some(ts) = published_at {
        let age_hours = (Utc::now() - ts).num_hours();
        if age_hours <= 6 {
            score += 15;
        } else if age_hours <= 24 {
            score += 8;
        } else if age_hours <= 72 {
            score += 3;
        }
    }
    score
}

fn detect_region(text: &str) -> String {
    if contains_any(
        text,
        &[
            "nigeria", "abuja", "lagos", "naira", "cbn", "nigerian exchange", "ngx", "bpe",
        ],
    ) {
        return "Nigeria".to_string();
    }
    if contains_any(
        text,
        &[
            "africa", "african", "kenya", "ghana", "south africa", "nairobi", "egypt",
        ],
    ) {
        return "Africa".to_string();
    }
    "Global".to_string()
}

fn detect_focus(text: &str) -> String {
    if contains_any(
        text,
        &[
            "crypto",
            "bitcoin",
            "ethereum",
            "stablecoin",
            "defi",
            "on-chain",
            "exchange inflow",
            "whale",
            "token",
        ],
    ) {
        return "Crypto".to_string();
    }
    if contains_any(
        text,
        &[
            "power", "electricity", "refinery", "oil", "gas", "energy", "disco", "genco",
        ],
    ) {
        return "Energy".to_string();
    }
    if contains_any(text, &["chip", "semiconductor", "gpu", "nvidia", "foundry"]) {
        return "Chips".to_string();
    }
    if contains_any(
        text,
        &[
            "data center", "datacenter", "cloud", "hyperscale", "server farm",
        ],
    ) {
        return "Cloud/DataCenter".to_string();
    }
    if contains_any(
        text,
        &[
            "foundation model", "llm", "model training", "model release", "openai", "anthropic",
        ],
    ) {
        return "Models".to_string();
    }
    if contains_any(
        text,
        &["ai app", "copilot", "automation", "agent", "workflow", "saas"],
    ) {
        return "Applications".to_string();
    }
    if contains_any(text, &["agriculture", "agri", "fertilizer", "farm", "food"]) {
        return "Agriculture".to_string();
    }
    if contains_any(
        text,
        &[
            "bank",
            "banking",
            "insurance",
            "asset management",
            "financial services",
            "fintech",
            "payment",
        ],
    ) {
        return "Financials".to_string();
    }
    if contains_any(text, &["ipo", "listing", "earnings", "market", "stocks", "dividend"]) {
        return "Markets".to_string();
    }
    "Business".to_string()
}

fn detect_tags(text: &str, cfg: &NewsConfig) -> String {
    let mut tags = Vec::new();
    for (name, words) in [
        ("ipo", vec!["ipo", "public offer", "listing", "book build"]),
        ("privatization", vec!["privatisation", "privatization", "bpe"]),
        ("agriculture", vec!["agriculture", "farm", "fertilizer", "food"]),
        ("policy", vec!["regulation", "policy", "act", "ministry", "cbn"]),
        ("earnings", vec!["earnings", "profit", "revenue", "guidance"]),
        ("m&a", vec!["acquisition", "merger", "stake", "buyout"]),
        ("cross-listing", vec!["uk exchange", "london stock exchange", "lse", "dual listing", "cross listing", "gdr"]),
        ("financials", vec!["banking", "bank", "insurance", "financial sector", "fintech"]),
        ("stock-spike", vec!["spike", "surge", "rally", "jumps", "soars", "top gainer", "52-week high"]),
        ("dividend-alert", vec!["dividend", "qualification date", "closure of register", "dividend declaration"]),
        ("report-release", vec!["earnings report", "results", "q1", "q2", "q3", "full year", "audited", "unaudited"]),
        ("crypto-alpha", vec!["crypto", "bitcoin", "ethereum", "stablecoin", "defi", "whale"]),
    ] {
        if words.iter().any(|w| text.contains(w)) {
            tags.push(name.to_string());
        }
    }
    for kw in &cfg.extra_keywords {
        let k = kw.trim().to_lowercase();
        if !k.is_empty() && text.contains(&k) {
            tags.push(k);
        }
    }
    if tags.is_empty() {
        "markets".to_string()
    } else {
        tags.join(", ")
    }
}

fn parse_date(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let s = raw?;
    DateTime::parse_from_rfc2822(s)
        .ok()
        .map(|v| v.with_timezone(&Utc))
        .or_else(|| DateTime::parse_from_rfc3339(s).ok().map(|v| v.with_timezone(&Utc)))
}

fn format_relative_time(ts: Option<DateTime<Utc>>) -> String {
    match ts {
        None => "unknown time".to_string(),
        Some(t) => {
            let d = Utc::now() - t;
            let mins = d.num_minutes();
            if mins < 60 {
                return format!("{mins}m ago");
            }
            let hrs = d.num_hours();
            if hrs < 24 {
                return format!("{hrs}h ago");
            }
            format!("{}d ago", d.num_days())
        }
    }
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut t = s[..max].to_string();
    t.push_str("...");
    t
}

fn extract_image(item: &rss::Item, description: &str) -> String {
    if let Some(enclosure) = item.enclosure() {
        let url = enclosure.url();
        if !url.is_empty() {
            return url.to_string();
        }
    }
    if let Some(media) = item.extensions().get("media") {
        if let Some(contents) = media.get("content") {
            for c in contents {
                if let Some(url) = c.attrs.get("url") {
                    if !url.is_empty() {
                        return url.clone();
                    }
                }
            }
        }
    }
    let marker = "img src=\"";
    if let Some(start) = description.find(marker) {
        let rest = &description[start + marker.len()..];
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
    }
    String::new()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| text.contains(n))
}

fn catalyst_keywords() -> &'static [&'static str] {
    &[
        "ipo",
        "listing",
        "public offer",
        "qualification date",
        "rights issue",
        "board",
        "acquisition",
        "merger",
        "dividend",
        "tariff",
        "regulation",
        "approval",
        "debt",
        "privatization",
        "privatisation",
        "earnings report",
        "results",
        "q1",
        "q2",
        "q3",
        "full year",
        "audited",
        "unaudited",
        "dividend",
        "qualification date",
        "closure of register",
        "spike",
        "surge",
        "rally",
        "jumps",
        "soars",
        "top gainer",
        "52-week high",
        "financial sector",
        "banking",
        "insurance",
        "stock price",
        "share price",
        "uk exchange",
        "london stock exchange",
        "lse",
        "dual listing",
        "cross listing",
        "gdr",
    ]
}

fn effective_sources(cfg: &NewsConfig) -> Vec<NewsSourceConfig> {
    if !cfg.google_queries.is_empty() {
        let mut out = Vec::new();
        for q in &cfg.google_queries {
            if q.trim().is_empty() {
                continue;
            }
            out.push(NewsSourceConfig {
                name: format!("Google News: {q}"),
                url: format!(
                    "https://news.google.com/rss/search?q={}&hl=en-NG&gl=NG&ceid=NG:en",
                    encode(q)
                ),
                enabled: true,
                priority: 14,
            });
        }
        if !out.is_empty() {
            return out;
        }
    }
    if !cfg.sources.is_empty() {
        return cfg.sources.clone();
    }
    default_sources()
}

fn default_sources() -> Vec<NewsSourceConfig> {
    vec![
        NewsSourceConfig {
            name: "Nigeria Business (Google News)".to_string(),
            url: "https://news.google.com/rss/search?q=Nigeria+business+markets+IPO+power".to_string(),
            enabled: true,
            priority: 12,
        },
        NewsSourceConfig {
            name: "Africa Markets (Google News)".to_string(),
            url: "https://news.google.com/rss/search?q=Africa+markets+business+investment".to_string(),
            enabled: true,
            priority: 10,
        },
        NewsSourceConfig {
            name: "Energy & Power Nigeria (Google News)".to_string(),
            url: "https://news.google.com/rss/search?q=Nigeria+power+sector+DisCo+GenCo+BPE+IPO".to_string(),
            enabled: true,
            priority: 14,
        },
        NewsSourceConfig {
            name: "AI Infra (Google News)".to_string(),
            url: "https://news.google.com/rss/search?q=AI+chips+cloud+data+center+markets".to_string(),
            enabled: true,
            priority: 8,
        },
        NewsSourceConfig {
            name: "Agriculture Markets (Google News)".to_string(),
            url: "https://news.google.com/rss/search?q=Nigeria+Africa+agriculture+commodities+markets".to_string(),
            enabled: true,
            priority: 9,
        },
        NewsSourceConfig {
            name: "NGX Financial Sector Reports".to_string(),
            url: "https://news.google.com/rss/search?q=NGX+Nigeria+financial+sector+earnings+report+dividend".to_string(),
            enabled: true,
            priority: 15,
        },
        NewsSourceConfig {
            name: "Nigeria Stocks Price Spikes".to_string(),
            url: "https://news.google.com/rss/search?q=NGX+top+gainers+stock+price+surge+Nigeria".to_string(),
            enabled: true,
            priority: 14,
        },
        NewsSourceConfig {
            name: "Nigeria UK Cross Listing".to_string(),
            url: "https://news.google.com/rss/search?q=Nigeria+company+UK+exchange+listing+LSE".to_string(),
            enabled: true,
            priority: 16,
        },
        NewsSourceConfig {
            name: "Zenith UK Listing Watch".to_string(),
            url: "https://news.google.com/rss/search?q=Zenith+Bank+UK+exchange+listing".to_string(),
            enabled: true,
            priority: 18,
        },
    ]
}

fn default_true() -> bool {
    true
}

fn default_refresh_secs() -> u64 {
    180
}

fn default_nitter_base() -> String {
    "https://nitter.poast.org".to_string()
}

fn default_twitter_collector_script() -> String {
    "twitter_collector.js".to_string()
}

fn default_twitter_base_url() -> String {
    "https://x.com".to_string()
}

fn default_twitter_headless() -> bool {
    true
}

fn default_twitter_limit() -> usize {
    10
}
