use reqwest::Client;
use serde_json::json;

#[derive(Clone)]
pub struct TelegramBot {
    client: Client,
    token: String,
}

impl TelegramBot {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
        }
    }

    pub async fn send(&self, chat_id: &str, message: &str) -> Result<(), TelegramError> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);

        let resp = self
            .client
            .post(&url)
            .json(&json!({
                "chat_id": chat_id,
                "text": message,
                "parse_mode": "HTML"
            }))
            .send()
            .await
            .map_err(TelegramError::Http)?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(TelegramError::Api(body));
        }

        Ok(())
    }

    pub async fn send_urgent(&self, chat_id: &str, message: &str) -> Result<(), TelegramError> {
        let formatted = format!("🚨 <b>URGENT</b>\n\n{message}");
        self.send(chat_id, &formatted).await
    }

    pub async fn send_info(&self, chat_id: &str, message: &str) -> Result<(), TelegramError> {
        let formatted = format!("ℹ️ {message}");
        self.send(chat_id, &formatted).await
    }

    pub async fn send_opportunity(
        &self,
        chat_id: &str,
        message: &str,
    ) -> Result<(), TelegramError> {
        let formatted = format!("💰 <b>OPPORTUNITY</b>\n\n{message}");
        self.send(chat_id, &formatted).await
    }
}

#[derive(Debug)]
pub enum TelegramError {
    Http(reqwest::Error),
    Api(String),
}

impl std::fmt::Display for TelegramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "telegram http error: {e}"),
            Self::Api(body) => write!(f, "telegram api error: {body}"),
        }
    }
}

impl std::error::Error for TelegramError {}
