pub mod scanner;
pub mod executor;
pub mod analytics;
pub mod signals;
pub mod polymarket;

use crate::config::AgentConfig;
use crate::telegram::TelegramBot;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Offline,
    Running,
    Error,
    Stopped,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offline => write!(f, "offline"),
            Self::Running => write!(f, "running"),
            Self::Error => write!(f, "error"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub status: AgentStatus,
    pub last_tick: Option<DateTime<Utc>>,
    pub last_message: Option<String>,
    pub tick_count: u64,
}

pub trait Agent: Send + Sync + 'static {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn icon(&self) -> &str;
    fn description(&self) -> &str;

    /// Called once per interval. Return a message string if there's something
    /// worth reporting (gets sent to Telegram). Return None for quiet ticks.
    fn tick(
        &mut self,
    ) -> impl std::future::Future<Output = Result<Option<String>, Box<dyn std::error::Error + Send + Sync>>> + Send;
}

struct RunningAgent {
    handle: JoinHandle<()>,
}

pub struct AgentManager {
    infos: Arc<RwLock<HashMap<String, AgentInfo>>>,
    running: Arc<Mutex<HashMap<String, RunningAgent>>>,
    telegram: TelegramBot,
    configs: HashMap<String, AgentConfig>,
    default_channel: Option<String>,
}

impl AgentManager {
    pub fn new(
        telegram: TelegramBot,
        configs: HashMap<String, AgentConfig>,
        default_channel: Option<String>,
    ) -> Self {
        Self {
            infos: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(Mutex::new(HashMap::new())),
            telegram,
            configs,
            default_channel,
        }
    }

    pub fn infos(&self) -> Arc<RwLock<HashMap<String, AgentInfo>>> {
        self.infos.clone()
    }

    pub async fn register<A: Agent>(&self, agent: A) {
        let info = AgentInfo {
            id: agent.id().to_string(),
            name: agent.name().to_string(),
            icon: agent.icon().to_string(),
            description: agent.description().to_string(),
            status: AgentStatus::Offline,
            last_tick: None,
            last_message: None,
            tick_count: 0,
        };
        self.infos.write().await.insert(agent.id().to_string(), info);
    }

    pub async fn start<A: Agent>(&self, mut agent: A) {
        let id = agent.id().to_string();
        let config = self.configs.get(&id).cloned();
        let interval_secs = config.as_ref().map(|c| c.interval_secs).unwrap_or(60);
        let channel = config
            .as_ref()
            .and_then(|c| c.telegram_channel.clone())
            .or_else(|| self.default_channel.clone());

        let infos = self.infos.clone();
        let telegram = self.telegram.clone();

        {
            let mut map = infos.write().await;
            if let Some(info) = map.get_mut(&id) {
                info.status = AgentStatus::Running;
            }
        }

        let agent_id = id.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval_secs),
            );

            loop {
                interval.tick().await;

                let result = agent.tick().await;

                let mut map = infos.write().await;
                if let Some(info) = map.get_mut(&agent_id) {
                    info.last_tick = Some(Utc::now());
                    info.tick_count += 1;

                    match result {
                        Ok(Some(msg)) => {
                            info.last_message = Some(msg.clone());
                            if let Some(ref ch) = channel {
                                let _ = telegram.send(ch, &msg).await;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let err_msg = format!("[{}] error: {e}", agent_id);
                            info.status = AgentStatus::Error;
                            info.last_message = Some(err_msg.clone());
                            tracing::error!("{err_msg}");
                        }
                    }
                }
            }
        });

        self.running.lock().await.insert(id, RunningAgent { handle });
    }

    pub async fn stop(&self, id: &str) {
        if let Some(running) = self.running.lock().await.remove(id) {
            running.handle.abort();
        }
        if let Some(info) = self.infos.write().await.get_mut(id) {
            info.status = AgentStatus::Stopped;
        }
    }

    pub async fn agent_list(&self) -> Vec<AgentInfo> {
        let map = self.infos.read().await;
        let mut list: Vec<AgentInfo> = map.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }
}
