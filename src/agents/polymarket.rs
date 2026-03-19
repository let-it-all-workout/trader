use super::Agent;
use crate::polymarket::{
    correlation::scan_correlations,
    crossplatform::scan_crossplatform,
    endgame::scan_endgame,
    marketmaking::scan_marketmaking,
    negrisk::scan_negrisk,
    newsarb::scan_newsarb,
    CorrelationOpportunity, CrossPlatformOpportunity, EndgameOpportunity,
    MarketMakingOpportunity, NegRiskOpportunity, NewsArbOpportunity,
};
use std::sync::Arc;
use tokio::sync::RwLock;

const SCAN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

#[derive(Debug, Clone, Default)]
pub struct PolymarketData {
    pub negrisk: Vec<NegRiskOpportunity>,
    pub endgame: Vec<EndgameOpportunity>,
    pub correlation: Vec<CorrelationOpportunity>,
    pub marketmaking: Vec<MarketMakingOpportunity>,
    pub crossplatform: Vec<CrossPlatformOpportunity>,
    pub newsarb: Vec<NewsArbOpportunity>,
}

pub struct PolymarketAgent {
    pub latest_data: Arc<RwLock<PolymarketData>>,
}

impl PolymarketAgent {
    pub fn new(latest_data: Arc<RwLock<PolymarketData>>) -> Self {
        Self { latest_data }
    }
}

impl Agent for PolymarketAgent {
    fn id(&self) -> &str {
        "polymarket"
    }

    fn name(&self) -> &str {
        "Polymarket"
    }

    fn icon(&self) -> &str {
        "◆"
    }

    fn description(&self) -> &str {
        "Scans Polymarket for structural mispricing and opportunities"
    }

    async fn tick(&mut self) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let (nr_res, eg_res, co_res, mm_res, cp_res, na_res) = tokio::join!(
            tokio::time::timeout(SCAN_TIMEOUT, scan_negrisk()),
            tokio::time::timeout(SCAN_TIMEOUT, scan_endgame()),
            tokio::time::timeout(SCAN_TIMEOUT, scan_correlations()),
            tokio::time::timeout(SCAN_TIMEOUT, scan_marketmaking()),
            tokio::time::timeout(SCAN_TIMEOUT, scan_crossplatform()),
            tokio::time::timeout(SCAN_TIMEOUT, scan_newsarb()),
        );

        let negrisk = unwrap_scan("negrisk", nr_res);
        let endgame = unwrap_scan("endgame", eg_res);
        let correlation = unwrap_scan("correlation", co_res);
        let marketmaking = unwrap_scan("marketmaking", mm_res);
        let crossplatform = unwrap_scan("crossplatform", cp_res);
        let newsarb = unwrap_scan("newsarb", na_res);

        let total = negrisk.len()
            + endgame.len()
            + correlation.len()
            + marketmaking.len()
            + crossplatform.len()
            + newsarb.len();

        if total == 0 {
            return Ok(None);
        }

        let mut parts: Vec<String> = Vec::new();

        if !negrisk.is_empty() {
            parts.push(format!("NR:{}", negrisk.len()));
        }
        if !endgame.is_empty() {
            parts.push(format!("EG:{}", endgame.len()));
        }
        if !correlation.is_empty() {
            parts.push(format!("CO:{}", correlation.len()));
        }
        if !marketmaking.is_empty() {
            parts.push(format!("MM:{}", marketmaking.len()));
        }
        if !crossplatform.is_empty() {
            parts.push(format!("CP:{}", crossplatform.len()));
        }
        if !newsarb.is_empty() {
            parts.push(format!("NA:{}", newsarb.len()));
        }

        let msg = format!("{total} opportunities · {}", parts.join(" · "));

        *self.latest_data.write().await = PolymarketData {
            negrisk,
            endgame,
            correlation,
            marketmaking,
            crossplatform,
            newsarb,
        };

        Ok(Some(msg))
    }
}

fn unwrap_scan<T: std::fmt::Debug>(
    name: &str,
    result: Result<Result<Vec<T>, crate::polymarket::PolymarketError>, tokio::time::error::Elapsed>,
) -> Vec<T> {
    match result {
        Ok(Ok(v)) => {
            tracing::info!("polymarket {name}: {} opportunities", v.len());
            v
        }
        Ok(Err(e)) => {
            tracing::warn!("polymarket {name} error: {e}");
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("polymarket {name} timed out");
            Vec::new()
        }
    }
}
