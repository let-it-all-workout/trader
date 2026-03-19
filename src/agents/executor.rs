use super::Agent;

pub struct ExecutorAgent {
    // Add fields here: order queue, wallet, position tracker, etc.
}

impl ExecutorAgent {
    pub fn new() -> Self {
        Self {}
    }
}

impl Agent for ExecutorAgent {
    fn id(&self) -> &str {
        "executor"
    }

    fn name(&self) -> &str {
        "Executor"
    }

    fn icon(&self) -> &str {
        "⚡"
    }

    fn description(&self) -> &str {
        "Places and manages trade orders"
    }

    async fn tick(&mut self) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        // TODO: check pending orders, manage positions
        Ok(None)
    }
}
