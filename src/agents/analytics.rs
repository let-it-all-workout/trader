use super::Agent;

pub struct AnalyticsAgent {
    // Add fields here: portfolio snapshots, P&L history, etc.
}

impl AnalyticsAgent {
    pub fn new() -> Self {
        Self {}
    }
}

impl Agent for AnalyticsAgent {
    fn id(&self) -> &str {
        "analytics"
    }

    fn name(&self) -> &str {
        "Analytics"
    }

    fn icon(&self) -> &str {
        "◈"
    }

    fn description(&self) -> &str {
        "Tracks P&L, portfolio stats, and performance"
    }

    async fn tick(&mut self) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: compute portfolio stats, check for alertable thresholds
        Ok(None)
    }
}
