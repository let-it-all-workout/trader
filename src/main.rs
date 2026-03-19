mod agents;
mod config;
mod funding;
mod intel;
mod news;
mod premium_feeds;
mod polymarket;
mod telegram;

use agents::analytics::AnalyticsAgent;
use agents::executor::ExecutorAgent;
use agents::polymarket::PolymarketAgent;
use agents::polymarket::PolymarketData;
use agents::scanner::ScannerAgent;
use agents::signals::SignalsAgent;
use agents::{AgentInfo, AgentManager, AgentStatus};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use config::Config;
use funding::{detect_arbs, ArbOpportunity, ExchangeFilter, FundingRate, StableArbOpportunity, EXCHANGES};
use intel::{IntelSignal, IntelSnapshot, WatchlistTrigger, WhaleSignal};
use news::{NewsConfig, NewsItem, NewsSnapshot, NewsSourceCounts};
use premium_feeds::{PremiumFeedsConfig, fetch_filings_insider, fetch_institutional_flow, fetch_ngx_microstructure};
use polymarket::{
    CorrelationOpportunity, CrossPlatformOpportunity, EndgameOpportunity,
    MarketMakingOpportunity, NegRiskOpportunity, NewsArbOpportunity,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use telegram::TelegramBot;
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};
use tower_http::services::ServeDir;

struct HtmlTemplate<T: Template>(T);

impl<T: Template> IntoResponse for HtmlTemplate<T> {
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain")],
                format!("template error: {e}"),
            )
                .into_response(),
        }
    }
}

// -- Sub-tool definition --

struct SubTool {
    icon: &'static str,
    name: &'static str,
    description: &'static str,
    href: &'static str,
}

fn scanner_tools() -> Vec<SubTool> {
    vec![SubTool {
        icon: "◎",
        name: "Funding Rate",
        description: "DEX perp funding rate arb scanner across 8 exchanges",
        href: "/scanner/funding",
    }]
}

fn executor_tools() -> Vec<SubTool> {
    vec![SubTool {
        icon: "⟐",
        name: "Order Manager",
        description: "Place and manage trade orders",
        href: "/executor/orders",
    }]
}

fn analytics_tools() -> Vec<SubTool> {
    vec![SubTool {
        icon: "▦",
        name: "Portfolio",
        description: "P&L tracking, portfolio stats, and performance",
        href: "/analytics/portfolio",
    }]
}

fn signals_tools() -> Vec<SubTool> {
    vec![SubTool {
        icon: "⫸",
        name: "Signal Feed",
        description: "Live signal stream from data analysis",
        href: "/signals/feed",
    }]
}

fn polymarket_tools() -> Vec<SubTool> {
    vec![SubTool {
        icon: "◆",
        name: "Opportunity Scanner",
        description: "Neg-risk, endgame, and correlation mispricing detection",
        href: "/polymarket/scanner",
    }]
}

// -- Templates --

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    agents: Vec<AgentInfo>,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    agents: Vec<AgentInfo>,
}

#[derive(Template)]
#[template(path = "agent_view.html")]
struct AgentViewTemplate {
    agent: AgentInfo,
    tools: Vec<SubTool>,
}

#[derive(Template)]
#[template(path = "funding_page.html")]
struct FundingPageTemplate {
    rates: Vec<FundingRate>,
    arbs: Vec<ArbOpportunity>,
    stable_arbs: Vec<StableArbOpportunity>,
    total_arbs: usize,
    total_markets: usize,
    num_exchanges: usize,
    scanner_status: AgentStatus,
    exchanges: Vec<(String, bool)>,
}

#[derive(Template)]
#[template(path = "funding_table.html")]
struct FundingTableTemplate {
    rates: Vec<FundingRate>,
    arbs: Vec<ArbOpportunity>,
    stable_arbs: Vec<StableArbOpportunity>,
    total_arbs: usize,
}

#[derive(Template)]
#[template(path = "coming_soon.html")]
struct ComingSoonTemplate {
    title: String,
}

#[derive(Template)]
#[template(path = "polymarket_page.html")]
struct PolymarketPageTemplate {
    negrisk: Vec<NegRiskOpportunity>,
    endgame: Vec<EndgameOpportunity>,
    correlation: Vec<CorrelationOpportunity>,
    marketmaking: Vec<MarketMakingOpportunity>,
    crossplatform: Vec<CrossPlatformOpportunity>,
    newsarb: Vec<NewsArbOpportunity>,
    total_opps: usize,
    scanner_status: AgentStatus,
}

#[derive(Template)]
#[template(path = "shell.html")]
struct ShellTemplate {
    content: String,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "polymarket_table.html")]
struct PolymarketTableTemplate {
    negrisk: Vec<NegRiskOpportunity>,
    endgame: Vec<EndgameOpportunity>,
    correlation: Vec<CorrelationOpportunity>,
    marketmaking: Vec<MarketMakingOpportunity>,
    crossplatform: Vec<CrossPlatformOpportunity>,
    newsarb: Vec<NewsArbOpportunity>,
    total_opps: usize,
}

#[derive(Template)]
#[template(path = "nigeria_news_section.html")]
struct NigeriaNewsTemplate {
    items: Vec<NewsItem>,
    total: usize,
    updated_label: String,
    source_counts: NewsSourceCounts,
}

#[derive(Template)]
#[template(path = "intel_panel.html")]
struct IntelPanelTemplate {
    alerts: Vec<IntelSignal>,
    scoreboard: Vec<IntelSignal>,
    watchlists: Vec<WatchlistTrigger>,
    whales: Vec<WhaleSignal>,
    updated_label: String,
}

#[derive(Template)]
#[template(path = "intel_alerts.html")]
struct IntelAlertsTemplate {
    alerts: Vec<IntelSignal>,
    whales: Vec<WhaleSignal>,
}

#[derive(Template)]
#[template(path = "intel_scoreboard.html")]
struct IntelScoreboardTemplate {
    scoreboard: Vec<IntelSignal>,
}

#[derive(Template)]
#[template(path = "intel_watchlists.html")]
struct IntelWatchlistsTemplate {
    watchlists: Vec<WatchlistTrigger>,
}

// -- State --

#[derive(Clone)]
struct AppState {
    manager: Arc<AgentManager>,
    latest_rates: Arc<RwLock<Vec<FundingRate>>>,
    latest_arbs: Arc<RwLock<Vec<ArbOpportunity>>>,
    latest_stable_arbs: Arc<RwLock<Vec<StableArbOpportunity>>>,
    exchange_filter: Arc<RwLock<ExchangeFilter>>,
    polymarket_data: Arc<RwLock<PolymarketData>>,
    nigeria_news: Arc<RwLock<NewsSnapshot>>,
    intel_data: Arc<RwLock<IntelSnapshot>>,
}

// -- Handlers --

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.manager.agent_list().await;
    HtmlTemplate(IndexTemplate {
        agents,
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn dashboard(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.manager.agent_list().await;
    HtmlTemplate(DashboardTemplate { agents })
}

async fn agent_view(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agents = state.manager.agent_list().await;
    let agent = match agents.into_iter().find(|a| a.id == id) {
        Some(a) => a,
        None => return (StatusCode::NOT_FOUND, "agent not found").into_response(),
    };

    let tools = match id.as_str() {
        "scanner" => scanner_tools(),
        "executor" => executor_tools(),
        "analytics" => analytics_tools(),
        "signals" => signals_tools(),
        "polymarket" => polymarket_tools(),
        _ => vec![],
    };

    HtmlTemplate(AgentViewTemplate { agent, tools }).into_response()
}

async fn agent_controls(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let agents = state.manager.agent_list().await;
    let agent = match agents.into_iter().find(|a| a.id == id) {
        Some(a) => a,
        None => return Html("".to_string()).into_response(),
    };

    let html = if agent.status == AgentStatus::Running {
        format!(
            "<button class=\"btn btn-danger\" hx-post=\"/agents/{}/stop\" hx-target=\"#agent-controls\" hx-swap=\"innerHTML\">Stop</button>",
            id,
        )
    } else {
        format!(
            "<button class=\"btn btn-start\" hx-post=\"/agents/{}/start\" hx-target=\"#agent-controls\" hx-swap=\"innerHTML\">Start</button>",
            id,
        )
    };
    Html(html).into_response()
}

async fn start_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match id.as_str() {
        "scanner" => {
            state
                .manager
                .start(ScannerAgent::new(
                    state.latest_rates.clone(),
                    state.latest_arbs.clone(),
                    state.latest_stable_arbs.clone(),
                    state.exchange_filter.clone(),
                ))
                .await
        }
        "executor" => state.manager.start(ExecutorAgent::new()).await,
        "analytics" => state.manager.start(AnalyticsAgent::new()).await,
        "signals" => state.manager.start(SignalsAgent::new()).await,
        "polymarket" => {
            state
                .manager
                .start(PolymarketAgent::new(state.polymarket_data.clone()))
                .await
        }
        _ => return (StatusCode::NOT_FOUND, "unknown agent").into_response(),
    }

    agent_controls(State(state), Path(id)).await.into_response()
}

async fn stop_agent(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    state.manager.stop(&id).await;
    agent_controls(State(state), Path(id)).await.into_response()
}

async fn funding_page(State(state): State<AppState>) -> impl IntoResponse {
    let filter = state.exchange_filter.read().await.clone();
    let all_rates = state.latest_rates.read().await.clone();

    let rates: Vec<FundingRate> = all_rates
        .into_iter()
        .filter(|r| filter.exchanges.contains(&r.exchange))
        .collect();
    let all_arbs = detect_arbs(&rates, 0.005, &filter.exchanges);
    let total_arbs = all_arbs.len();
    let total_markets = rates.len();
    let num_exchanges = filter.exchanges.len();
    let arbs: Vec<ArbOpportunity> = all_arbs.into_iter().take(24).collect();

    let stable_arbs = state.latest_stable_arbs.read().await.clone();
    let stable_arbs: Vec<StableArbOpportunity> = stable_arbs.into_iter().take(24).collect();

    let agents = state.manager.agent_list().await;
    let scanner_status = agents
        .into_iter()
        .find(|a| a.id == "scanner")
        .map(|a| a.status)
        .unwrap_or(AgentStatus::Offline);

    let exchanges: Vec<(String, bool)> = EXCHANGES
        .iter()
        .map(|ex| {
            let active = filter.exchanges.contains(*ex);
            (ex.to_string(), active)
        })
        .collect();

    HtmlTemplate(FundingPageTemplate {
        rates,
        arbs,
        stable_arbs,
        total_arbs,
        total_markets,
        num_exchanges,
        scanner_status,
        exchanges,
    })
}

#[derive(Deserialize)]
struct FilterQuery {
    #[serde(default)]
    exchanges: Option<String>,
}

async fn funding_status(
    State(state): State<AppState>,
    Query(query): Query<FilterQuery>,
) -> impl IntoResponse {
    let filter = state.exchange_filter.read().await.clone();

    let active = if let Some(ref ex_str) = query.exchanges {
        ex_str.split(',').map(|s| s.to_string()).collect::<HashSet<_>>()
    } else {
        filter.exchanges.clone()
    };

    let all_rates = state.latest_rates.read().await.clone();
    let rates: Vec<FundingRate> = all_rates
        .into_iter()
        .filter(|r| active.contains(&r.exchange))
        .collect();
    let all_arbs = detect_arbs(&rates, 0.005, &active);
    let total_arbs = all_arbs.len();
    let arbs: Vec<ArbOpportunity> = all_arbs.into_iter().take(24).collect();

    let stable_arbs = state.latest_stable_arbs.read().await.clone();
    let stable_arbs: Vec<StableArbOpportunity> = stable_arbs.into_iter().take(24).collect();

    HtmlTemplate(FundingTableTemplate {
        rates,
        arbs,
        stable_arbs,
        total_arbs,
    })
}

async fn toggle_exchange(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    {
        let mut filter = state.exchange_filter.write().await;
        if filter.exchanges.contains(&name) {
            if filter.exchanges.len() > 1 {
                filter.exchanges.remove(&name);
            }
        } else {
            filter.exchanges.insert(name);
        }
    }

    funding_page(State(state)).await.into_response()
}

async fn polymarket_page(headers: HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    let data = state.polymarket_data.read().await;
    let total_opps = data.negrisk.len()
        + data.endgame.len()
        + data.correlation.len()
        + data.marketmaking.len()
        + data.crossplatform.len()
        + data.newsarb.len();

    let agents = state.manager.agent_list().await;
    let scanner_status = agents
        .into_iter()
        .find(|a| a.id == "polymarket")
        .map(|a| a.status)
        .unwrap_or(AgentStatus::Offline);

    let partial = PolymarketPageTemplate {
        negrisk: data.negrisk.clone(),
        endgame: data.endgame.clone(),
        correlation: data.correlation.clone(),
        marketmaking: data.marketmaking.clone(),
        crossplatform: data.crossplatform.clone(),
        newsarb: data.newsarb.clone(),
        total_opps,
        scanner_status,
    };

    let is_htmx = headers.get("HX-Request").is_some();
    if is_htmx {
        HtmlTemplate(partial).into_response()
    } else {
        let content = partial.render().unwrap_or_default();
        HtmlTemplate(ShellTemplate {
            content,
            version: env!("CARGO_PKG_VERSION"),
        })
        .into_response()
    }
}

async fn polymarket_status(State(state): State<AppState>) -> impl IntoResponse {
    let data = state.polymarket_data.read().await;
    let total_opps = data.negrisk.len()
        + data.endgame.len()
        + data.correlation.len()
        + data.marketmaking.len()
        + data.crossplatform.len()
        + data.newsarb.len();

    HtmlTemplate(PolymarketTableTemplate {
        negrisk: data.negrisk.clone(),
        endgame: data.endgame.clone(),
        correlation: data.correlation.clone(),
        marketmaking: data.marketmaking.clone(),
        crossplatform: data.crossplatform.clone(),
        newsarb: data.newsarb.clone(),
        total_opps,
    })
}

async fn nigeria_news_status(State(state): State<AppState>) -> impl IntoResponse {
    let snapshot = state.nigeria_news.read().await.clone();
    let updated_label = match snapshot.updated_at {
        Some(t) => format!("updated {}", t.format("%H:%M:%S UTC")),
        None => "waiting for first fetch...".to_string(),
    };
    HtmlTemplate(NigeriaNewsTemplate {
        total: snapshot.items.len(),
        items: snapshot.items,
        updated_label,
        source_counts: snapshot.source_counts,
    })
}

async fn intel_status(State(state): State<AppState>) -> impl IntoResponse {
    let intel = state.intel_data.read().await.clone();
    let updated_label = match intel.updated_at {
        Some(t) => format!("updated {}", t.format("%H:%M:%S UTC")),
        None => "waiting for first signal pass...".to_string(),
    };

    HtmlTemplate(IntelPanelTemplate {
        alerts: intel.alerts,
        scoreboard: intel.scoreboard,
        watchlists: intel.watchlists,
        whales: intel.whales,
        updated_label,
    })
}

async fn intel_alerts(State(state): State<AppState>) -> impl IntoResponse {
    let intel = state.intel_data.read().await.clone();
    HtmlTemplate(IntelAlertsTemplate {
        alerts: intel.alerts,
        whales: intel.whales,
    })
}

async fn intel_scoreboard(State(state): State<AppState>) -> impl IntoResponse {
    let intel = state.intel_data.read().await.clone();
    HtmlTemplate(IntelScoreboardTemplate {
        scoreboard: intel.scoreboard,
    })
}

async fn intel_watchlists(State(state): State<AppState>) -> impl IntoResponse {
    let intel = state.intel_data.read().await.clone();
    HtmlTemplate(IntelWatchlistsTemplate {
        watchlists: intel.watchlists,
    })
}

async fn coming_soon(Path(path): Path<String>) -> impl IntoResponse {
    HtmlTemplate(ComingSoonTemplate {
        title: path.replace('/', " > "),
    })
}

// -- Main --

fn create_agents(
    latest_rates: Arc<RwLock<Vec<FundingRate>>>,
    latest_arbs: Arc<RwLock<Vec<ArbOpportunity>>>,
    latest_stable_arbs: Arc<RwLock<Vec<StableArbOpportunity>>>,
    exchange_filter: Arc<RwLock<ExchangeFilter>>,
    polymarket_data: Arc<RwLock<PolymarketData>>,
) -> Vec<
    Box<
        dyn FnOnce(
            &AgentManager,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>>,
    >,
> {
    vec![
        Box::new(move |m: &AgentManager| {
            Box::pin(m.register(ScannerAgent::new(latest_rates, latest_arbs, latest_stable_arbs, exchange_filter)))
        }),
        Box::new(|m: &AgentManager| Box::pin(m.register(ExecutorAgent::new()))),
        Box::new(|m: &AgentManager| Box::pin(m.register(AnalyticsAgent::new()))),
        Box::new(|m: &AgentManager| Box::pin(m.register(SignalsAgent::new()))),
        Box::new(move |m: &AgentManager| {
            Box::pin(m.register(PolymarketAgent::new(polymarket_data)))
        }),
    ]
}

fn spawn_news_updater(
    news_cache: Arc<RwLock<NewsSnapshot>>,
    intel_cache: Arc<RwLock<IntelSnapshot>>,
    news_cfg: NewsConfig,
    premium_cfg: PremiumFeedsConfig,
) {
    if !news_cfg.enabled {
        return;
    }
    let refresh_secs = news_cfg.refresh_secs.max(60);
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        loop {
            match news::fetch_news_snapshot(&client, &news_cfg, 80).await {
                Ok(snapshot) => {
                    *news_cache.write().await = snapshot.clone();
                    *intel_cache.write().await = intel::build_intel_snapshot(&snapshot);
                }
                Err(e) => tracing::warn!("news: refresh failed: {e}"),
            }

            // Premium placeholders: validates configuration and keeps connector wiring alive.
            if let Err(e) = fetch_ngx_microstructure(&premium_cfg.ngx_microstructure).await {
                tracing::warn!("premium ngx_microstructure: {e}");
            }
            if let Err(e) = fetch_filings_insider(&premium_cfg.filings_insider).await {
                tracing::warn!("premium filings_insider: {e}");
            }
            if let Err(e) = fetch_institutional_flow(&premium_cfg.institutional_flow).await {
                tracing::warn!("premium institutional_flow: {e}");
            }
            sleep(Duration::from_secs(refresh_secs)).await;
        }
    });
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut config = Config::load(std::path::Path::new("polym.toml")).unwrap_or_else(|e| {
        tracing::warn!("no polym.toml found ({e}), using defaults");
        Config {
            server: config::ServerConfig { port: 3000 },
            telegram: config::TelegramConfig {
                bot_token: String::new(),
                channels: Default::default(),
            },
            agents: Default::default(),
            news: Default::default(),
            premium_feeds: Default::default(),
        }
    });

    // Railway/containers typically inject PORT at runtime.
    if let Ok(port_raw) = std::env::var("PORT") {
        if let Ok(port) = port_raw.parse::<u16>() {
            config.server.port = port;
        }
    }

    // Prefer environment secrets over file-stored API keys.
    if let Ok(k) = std::env::var("BRAVE_API_KEY") {
        if !k.trim().is_empty() {
            config.news.brave.api_key = k;
            config.news.brave.enabled = true;
        }
    }

    let telegram = TelegramBot::new(&config.telegram.bot_token);
    let default_channel = config.channel_id("default").map(|s| s.to_string());
    let manager = Arc::new(AgentManager::new(
        telegram,
        config.agents.clone(),
        default_channel,
    ));

    let latest_rates: Arc<RwLock<Vec<FundingRate>>> = Arc::new(RwLock::new(Vec::new()));
    let latest_arbs: Arc<RwLock<Vec<ArbOpportunity>>> = Arc::new(RwLock::new(Vec::new()));
    let latest_stable_arbs: Arc<RwLock<Vec<StableArbOpportunity>>> =
        Arc::new(RwLock::new(Vec::new()));
    let exchange_filter: Arc<RwLock<ExchangeFilter>> =
        Arc::new(RwLock::new(ExchangeFilter::default()));
    let polymarket_data: Arc<RwLock<PolymarketData>> =
        Arc::new(RwLock::new(PolymarketData::default()));
    let nigeria_news: Arc<RwLock<NewsSnapshot>> = Arc::new(RwLock::new(NewsSnapshot::default()));
    let intel_data: Arc<RwLock<IntelSnapshot>> = Arc::new(RwLock::new(IntelSnapshot::default()));
    let news_config = config.news.clone();
    let premium_config = config.premium_feeds.clone();

    spawn_news_updater(
        nigeria_news.clone(),
        intel_data.clone(),
        news_config,
        premium_config,
    );

    for factory in create_agents(
        latest_rates.clone(),
        latest_arbs.clone(),
        latest_stable_arbs.clone(),
        exchange_filter.clone(),
        polymarket_data.clone(),
    ) {
        factory(&manager).await;
    }

    manager
        .start(ScannerAgent::new(
            latest_rates.clone(),
            latest_arbs.clone(),
            latest_stable_arbs.clone(),
            exchange_filter.clone(),
        ))
        .await;
    tracing::info!("scanner auto-started");

    manager
        .start(PolymarketAgent::new(polymarket_data.clone()))
        .await;
    tracing::info!("polymarket agent auto-started");

    let state = AppState {
        manager: manager.clone(),
        latest_rates,
        latest_arbs,
        latest_stable_arbs,
        exchange_filter,
        polymarket_data,
        nigeria_news,
        intel_data,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/dashboard", get(dashboard))
        .route("/agents/{id}/view", get(agent_view))
        .route("/agents/{id}/controls", get(agent_controls))
        .route("/agents/{id}/start", post(start_agent))
        .route("/agents/{id}/stop", post(stop_agent))
        .route("/scanner/funding", get(funding_page))
        .route("/funding/status", get(funding_status))
        .route("/funding/toggle/{name}", post(toggle_exchange))
        .route("/polymarket/scanner", get(polymarket_page))
        .route("/polymarket/status", get(polymarket_status))
        .route("/news/nigeria/status", get(nigeria_news_status))
        .route("/intel/status", get(intel_status))
        .route("/intel/alerts", get(intel_alerts))
        .route("/intel/scoreboard", get(intel_scoreboard))
        .route("/intel/watchlists", get(intel_watchlists))
        .route("/executor/{*rest}", get(coming_soon))
        .route("/analytics/{*rest}", get(coming_soon))
        .route("/signals/{*rest}", get(coming_soon))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind");

    tracing::info!("polym running at http://localhost:{}", config.server.port);
    println!("polym running at http://localhost:{}", config.server.port);

    axum::serve(listener, app).await.expect("server error");
}
