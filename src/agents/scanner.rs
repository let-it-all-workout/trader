use super::Agent;
use crate::funding::{
    self, aster::Aster, backpack::Backpack, detect_arbs, detect_stable_arbs, dydx::DyDx,
    drift::Drift, ethereal::Ethereal, hibachi::Hibachi, hyperliquid::Hyperliquid,
    lighter::Lighter, orderly::Orderly, pacifica::Pacifica, paradex::Paradex,
    variational::Variational, vest::Vest, ArbOpportunity, ExchangeFilter, FundingRate,
    FundingSource, RateHistory, StableArbOpportunity,
};
use std::sync::Arc;
use tokio::sync::RwLock;

const PER_EXCHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const HISTORY_WINDOW: usize = 30;
const MIN_STABLE_SAMPLES: usize = 3;
const MIN_STABLE_CONSISTENCY: f64 = 0.7;

pub struct ScannerAgent {
    sources: Vec<Box<dyn FundingSourceBoxed>>,
    min_spread_pct: f64,
    pub latest_rates: Arc<RwLock<Vec<FundingRate>>>,
    pub latest_arbs: Arc<RwLock<Vec<ArbOpportunity>>>,
    pub latest_stable_arbs: Arc<RwLock<Vec<StableArbOpportunity>>>,
    pub exchange_filter: Arc<RwLock<ExchangeFilter>>,
    rate_history: RateHistory,
}

trait FundingSourceBoxed: Send + Sync {
    fn name(&self) -> &str;
    fn fetch_rates(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<FundingRate>, funding::FundingError>> + Send + '_>,
    >;
}

impl<T: FundingSource> FundingSourceBoxed for T {
    fn name(&self) -> &str {
        FundingSource::name(self)
    }

    fn fetch_rates(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<FundingRate>, funding::FundingError>> + Send + '_>,
    > {
        Box::pin(FundingSource::fetch_rates(self))
    }
}

impl ScannerAgent {
    pub fn new(
        latest_rates: Arc<RwLock<Vec<FundingRate>>>,
        latest_arbs: Arc<RwLock<Vec<ArbOpportunity>>>,
        latest_stable_arbs: Arc<RwLock<Vec<StableArbOpportunity>>>,
        exchange_filter: Arc<RwLock<ExchangeFilter>>,
    ) -> Self {
        let sources: Vec<Box<dyn FundingSourceBoxed>> = vec![
            Box::new(Hyperliquid::new()),
            Box::new(DyDx::new()),
            Box::new(Paradex::new()),
            Box::new(Drift::new()),
            Box::new(Backpack::new()),
            Box::new(Orderly::new()),
            Box::new(Ethereal::new()),
            Box::new(Aster::new()),
            Box::new(Variational::new()),
            Box::new(Pacifica::new()),
            Box::new(Hibachi::new()),
            Box::new(Vest::new()),
            Box::new(Lighter::new()),
        ];

        Self {
            sources,
            min_spread_pct: 0.005,
            latest_rates,
            latest_arbs,
            latest_stable_arbs,
            exchange_filter,
            rate_history: RateHistory::new(HISTORY_WINDOW),
        }
    }
}

impl Agent for ScannerAgent {
    fn id(&self) -> &str {
        "scanner"
    }

    fn name(&self) -> &str {
        "Scanner"
    }

    fn icon(&self) -> &str {
        "⊙"
    }

    fn description(&self) -> &str {
        "Scans DEX perp funding rates for arbitrage"
    }

    async fn tick(&mut self) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let futs: Vec<_> = self
            .sources
            .iter()
            .map(|source| {
                let name = source.name().to_string();
                async move {
                    match tokio::time::timeout(PER_EXCHANGE_TIMEOUT, source.fetch_rates()).await {
                        Ok(Ok(rates)) => {
                            tracing::info!("{}: fetched {} rates", name, rates.len());
                            rates
                        }
                        Ok(Err(e)) => {
                            tracing::warn!("{}: {}", name, e);
                            Vec::new()
                        }
                        Err(_) => {
                            tracing::warn!("{}: timed out after {}s", name, PER_EXCHANGE_TIMEOUT.as_secs());
                            Vec::new()
                        }
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(futs).await;
        let all_rates: Vec<FundingRate> = results.into_iter().flatten().collect();

        if all_rates.is_empty() {
            return Ok(None);
        }

        self.rate_history.push(&all_rates);

        let filter = self.exchange_filter.read().await;
        let arbs = detect_arbs(&all_rates, self.min_spread_pct, &filter.exchanges);

        let stable_arbs = detect_stable_arbs(
            &all_rates,
            &self.rate_history,
            self.min_spread_pct,
            MIN_STABLE_CONSISTENCY,
            MIN_STABLE_SAMPLES,
            &filter.exchanges,
        );

        *self.latest_rates.write().await = all_rates;
        *self.latest_arbs.write().await = arbs.clone();
        *self.latest_stable_arbs.write().await = stable_arbs;

        if arbs.is_empty() {
            return Ok(None);
        }

        let mut msg = format!("{} arbs found · ", arbs.len());
        for (i, arb) in arbs.iter().take(3).enumerate() {
            if i > 0 {
                msg.push_str(" · ");
            }
            msg.push_str(&format!(
                "{} {:.0}%",
                arb.symbol, arb.spread_annual,
            ));
        }
        if arbs.len() > 3 {
            msg.push_str(&format!(" +{} more", arbs.len() - 3));
        }

        Ok(Some(msg))
    }
}
