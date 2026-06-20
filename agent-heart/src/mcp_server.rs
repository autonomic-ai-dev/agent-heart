use rmcp::model::{CallToolResult, Content, ErrorData as McpError, ServerInfo};
use rmcp::serve_server;
use rmcp::tool;
use rmcp::ServerHandler;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::brain_client::BrainHandle;

#[derive(Clone)]
pub struct HeartMcp {
    brain: BrainHandle,
}

impl HeartMcp {
    pub fn new(brain: BrainHandle) -> Self {
        Self { brain }
    }

    pub async fn run(brain: BrainHandle) -> anyhow::Result<()> {
        let server = Self::new(brain);
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

#[tool(tool_box)]
impl HeartMcp {
    #[tool(description = "Trigger garbage collection on agent-brain")]
    async fn heart_trigger_gc(
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

    #[tool(description = "Show agent-heart daemon status")]
    async fn heart_status(&self) -> Result<CallToolResult, McpError> {
        let now = chrono::Utc::now().to_rfc3339();
        let state_dir = dirs::state_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join("agent-heart");
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
                "Background distillation daemon for agent-brain. Use heart_trigger_gc to run GC."
                    .into(),
            ),
            ..Default::default()
        }
    }
}
