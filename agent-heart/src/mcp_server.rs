use rmcp::model::{CallToolResult, Content, ErrorData as McpError, ServerInfo};
use rmcp::serve_server;
use rmcp::tool;
use rmcp::ServerHandler;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::brain_client::BrainHandle;
use crate::config::Config;
use crate::token_budget;

#[derive(Clone)]
pub struct HeartMcp {
    brain: BrainHandle,
    config: Config,
}

impl HeartMcp {
    pub fn new(brain: BrainHandle, config: Config) -> Self {
        Self { brain, config }
    }

    pub async fn run(brain: BrainHandle, config: Config) -> anyhow::Result<()> {
        let server = Self::new(brain, config);
        let service = serve_server(server, rmcp::transport::io::stdio()).await?;
        service.waiting().await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TriggerGcParams {
    #[serde(default = "default_min_confidence")]
    min_confidence: f64,
    #[serde(default = "default_max_age_days")]
    max_age_days: u64,
}

fn default_min_confidence() -> f64 {
    0.3
}
fn default_max_age_days() -> u64 {
    90
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DistillParams {
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default)]
    dry_run: bool,
}

fn default_threshold() -> f64 {
    0.75
}

#[tool(tool_box)]
impl HeartMcp {
    #[tool(description = "Run memory garbage collection on agent-brain")]
    async fn heart_gc(
        &self,
        #[tool(aggr)] params: TriggerGcParams,
    ) -> Result<CallToolResult, McpError> {
        match self
            .brain
            .call_gc(params.min_confidence, params.max_age_days)
            .await
        {
            Ok(stats) => {
                let text =
                    serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Return current token/cost consumption across the session from agent-brain retrieval_log"
    )]
    async fn heart_budget_status(&self) -> Result<CallToolResult, McpError> {
        match token_budget::load_stats(&self.config.token_budget) {
            Ok(report) => {
                let text =
                    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Summarize related memory facts into higher-level concepts via cluster distillation"
    )]
    async fn heart_memory_distill(
        &self,
        #[tool(aggr)] params: DistillParams,
    ) -> Result<CallToolResult, McpError> {
        match self
            .brain
            .call_distill(params.threshold, params.dry_run)
            .await
        {
            Ok(stats) => {
                let text =
                    serde_json::to_string_pretty(&stats).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(description = "Show agent-heart daemon status")]
    async fn heart_status(&self) -> Result<CallToolResult, McpError> {
        let now = chrono::Utc::now().to_rfc3339();
        let state_dir = crate::config::state_dir();
        let last_gc = std::fs::read_to_string(state_dir.join("last_gc.txt"))
            .unwrap_or_else(|_| "never".to_string());

        let status = serde_json::json!({
            "status": "running",
            "version": env!("CARGO_PKG_VERSION"),
            "last_gc": last_gc,
            "checked_at": now,
        });

        let text = serde_json::to_string_pretty(&status).unwrap_or_else(|_| "{}".to_string());
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[tool(tool_box)]
impl ServerHandler for HeartMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Background distillation daemon for agent-brain. Tools: heart_gc (run GC), heart_budget_status (token/cost usage), heart_memory_distill (cluster distillation), heart_status (daemon status)."
                    .into(),
            ),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_min_confidence_is_0_3() {
        assert!((default_min_confidence() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn default_max_age_days_is_90() {
        assert_eq!(default_max_age_days(), 90);
    }

    #[test]
    fn default_threshold_is_0_75() {
        assert!((default_threshold() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn heart_mcp_implements_server_handler() {
        fn assert_handler<T: rmcp::ServerHandler>() {}
        assert_handler::<HeartMcp>();
    }
}
