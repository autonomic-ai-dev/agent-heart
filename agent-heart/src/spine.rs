use std::collections::HashMap;

#[derive(Clone)]
pub struct SpineClient {
    url: String,
    name: String,
    version: String,
    client: reqwest::Client,
}

impl SpineClient {
    pub fn new(url: &str, name: &str, version: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            name: name.to_string(),
            version: version.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn register(&self) -> anyhow::Result<()> {
        let info = serde_json::json!({
            "name": self.name,
            "version": self.version,
            "capabilities": ["maintenance:gc", "maintenance:distill"],
            "last_seen": chrono::Utc::now().to_rfc3339(),
            "metadata": HashMap::<String, String>::new(),
        });
        let resp = self
            .client
            .post(format!("{}/api/v1/agents", self.url))
            .json(&info)
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::warn!("Agent registration returned {}", resp.status());
        } else {
            tracing::info!("Registered with agent-spine as '{}'", self.name);
        }
        Ok(())
    }

    pub async fn heartbeat(&self) -> anyhow::Result<()> {
        let resp = self
            .client
            .post(format!("{}/api/v1/agents/{}", self.url, self.name))
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::warn!("Heartbeat returned {}", resp.status());
        }
        Ok(())
    }

    pub async fn publish(&self, subject: &str, payload: &serde_json::Value) -> anyhow::Result<()> {
        let body = serde_json::json!({
            "source": self.name,
            "subject": subject,
            "payload": payload,
            "metadata": {},
        });
        let resp = self
            .client
            .post(format!("{}/api/v1/events", self.url))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::warn!("Event publish returned {}", resp.status());
        }
        Ok(())
    }
}
