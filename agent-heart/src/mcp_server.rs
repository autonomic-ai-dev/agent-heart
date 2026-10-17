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

#[derive(Debug, Deserialize, JsonSchema)]
struct UsageSummaryParams {
    #[serde(default = "default_lookback_days")]
    lookback_days: u32,
}

fn default_lookback_days() -> u32 {
    30
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RecordUsageParams {
    organ: String,
    provider: String,
    model: String,
    #[serde(default = "default_billing_type")]
    billing_type: String,
    prompt_tokens: u64,
    completion_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
    #[serde(default)]
    latency_ms: u64,
    #[serde(default)]
    session_id: Option<String>,
}

fn default_billing_type() -> String {
    "metered_api".into()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RecordQuotaParams {
    provider: String,
    account_or_tier: String,
    window_type: String,
    utilization_pct: f64,
    #[serde(default)]
    reset_at: Option<i64>,
    #[serde(default)]
    rate_limit_remaining: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ParseHeadersQuotaParams {
    headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    account_or_tier: Option<String>,
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

    #[tool(
        description = "Run a background self-learning cycle analyzing recent verify outcomes and synthesizing memory facts"
    )]
    async fn heart_self_learn(&self) -> Result<CallToolResult, McpError> {
        match crate::self_learning::run_self_learning(&self.config) {
            Ok(report) => {
                let text =
                    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Return multi-day token usage, cost burn rate, and month-end spend projections"
    )]
    async fn heart_usage_summary(
        &self,
        #[tool(aggr)] params: UsageSummaryParams,
    ) -> Result<CallToolResult, McpError> {
        match crate::usage_store::UsageStore::open_default() {
            Ok(conn) => {
                match crate::usage_store::UsageStore::query_usage_summary(
                    &conn,
                    params.lookback_days,
                ) {
                    Ok(summary) => {
                        let text = serde_json::to_string_pretty(&summary)
                            .unwrap_or_else(|_| "{}".to_string());
                        Ok(CallToolResult::success(vec![Content::text(text)]))
                    }
                    Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
                }
            }
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Return active subscription quota utilization percentages and reset timestamps"
    )]
    async fn heart_quota_status(&self) -> Result<CallToolResult, McpError> {
        match crate::usage_store::UsageStore::open_default() {
            Ok(conn) => match crate::usage_store::UsageStore::get_quota_snapshots(&conn) {
                Ok(snapshots) => {
                    let text = serde_json::to_string_pretty(&snapshots)
                        .unwrap_or_else(|_| "[]".to_string());
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
            },
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Record a model turn usage event and automatically calculate cost and rollups"
    )]
    async fn heart_record_usage(
        &self,
        #[tool(aggr)] params: RecordUsageParams,
    ) -> Result<CallToolResult, McpError> {
        let billing = params
            .billing_type
            .parse::<crate::usage_schema::BillingType>()
            .unwrap_or(crate::usage_schema::BillingType::MeteredApi);
        let catalog = crate::pricing_catalog::PricingCatalog::new();
        let tokens = crate::pricing_catalog::TokenUsageCounts::new(
            params.prompt_tokens,
            params.completion_tokens,
            params.cache_read_tokens,
            params.cache_write_tokens,
        );
        let cost = catalog.calculate_cost(&params.provider, &params.model, billing, tokens);
        let event = crate::usage_schema::ModelUsageEvent {
            id: format!("evt-{}", uuid::Uuid::new_v4()),
            timestamp: chrono::Utc::now().timestamp_millis(),
            session_id: params.session_id,
            organ: params.organ,
            provider: params.provider,
            model: params.model,
            billing_type: billing,
            prompt_tokens: params.prompt_tokens,
            completion_tokens: params.completion_tokens,
            cache_read_tokens: params.cache_read_tokens,
            cache_write_tokens: params.cache_write_tokens,
            estimated_cost_usd: cost,
            latency_ms: params.latency_ms,
        };

        match crate::usage_store::UsageStore::open_default() {
            Ok(conn) => match crate::usage_store::UsageStore::record_event(&conn, &event) {
                Ok(_) => {
                    let text =
                        serde_json::to_string_pretty(&event).unwrap_or_else(|_| "{}".to_string());
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
            },
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Record or update a provider subscription quota or rate limit window snapshot"
    )]
    async fn heart_record_quota(
        &self,
        #[tool(aggr)] params: RecordQuotaParams,
    ) -> Result<CallToolResult, McpError> {
        let snapshot = crate::usage_schema::ProviderQuotaSnapshot {
            provider: params.provider,
            account_or_tier: params.account_or_tier,
            window_type: params.window_type,
            utilization_pct: params.utilization_pct.clamp(0.0, 100.0),
            reset_at: params.reset_at,
            rate_limit_remaining: params.rate_limit_remaining,
            updated_at: chrono::Utc::now().timestamp_millis(),
        };

        match crate::usage_store::UsageStore::open_default() {
            Ok(conn) => match crate::usage_store::UsageStore::record_quota(&conn, &snapshot) {
                Ok(_) => {
                    let text = serde_json::to_string_pretty(&snapshot)
                        .unwrap_or_else(|_| "{}".to_string());
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
            },
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }

    #[tool(
        description = "Parse LLM response headers (Anthropic, OpenAI, Gemini) and record quota snapshots"
    )]
    async fn heart_parse_and_record_headers(
        &self,
        #[tool(aggr)] params: ParseHeadersQuotaParams,
    ) -> Result<CallToolResult, McpError> {
        let snapshots = crate::quota_parser::parse_provider_headers(
            params.provider.as_deref(),
            &params.headers,
            params.account_or_tier.as_deref(),
        );

        match crate::usage_store::UsageStore::open_default() {
            Ok(conn) => {
                for snap in &snapshots {
                    if let Err(e) = crate::usage_store::UsageStore::record_quota(&conn, snap) {
                        return Err(McpError::internal_error(format!("{e}"), None));
                    }
                }
                let text =
                    serde_json::to_string_pretty(&snapshots).unwrap_or_else(|_| "[]".to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(format!("{e}"), None)),
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for HeartMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Metric and governance daemon for Autonomic AI. Tools: heart_gc (run GC), heart_budget_status (token/cost usage), heart_usage_summary (multi-day usage & burn rate), heart_quota_status (subscription quotas), heart_record_quota (record quota snapshot), heart_parse_and_record_headers (parse provider headers), heart_record_usage (record turn telemetry), heart_memory_distill (cluster distillation), heart_status (daemon status), heart_self_learn (offline critic)."
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

    #[test]
    fn default_lookback_days_is_30() {
        assert_eq!(default_lookback_days(), 30);
    }

    #[test]
    fn default_billing_type_is_metered_api() {
        assert_eq!(default_billing_type(), "metered_api");
    }

    #[test]
    fn test_record_quota_params_deserialization() {
        let json = r#"{
            "provider": "anthropic",
            "account_or_tier": "Claude Code (5h rolling)",
            "window_type": "5h_rolling",
            "utilization_pct": 58.0,
            "reset_at": 1776451200000,
            "rate_limit_remaining": 42000
        }"#;
        let params: RecordQuotaParams = serde_json::from_str(json).expect("deserialize RecordQuotaParams");
        assert_eq!(params.provider, "anthropic");
        assert_eq!(params.account_or_tier, "Claude Code (5h rolling)");
        assert_eq!(params.window_type, "5h_rolling");
        assert_eq!(params.utilization_pct, 58.0);
        assert_eq!(params.reset_at, Some(1776451200000));
        assert_eq!(params.rate_limit_remaining, Some(42000));
    }

    #[test]
    fn test_parse_headers_quota_params_deserialization() {
        let json = r#"{
            "headers": {
                "x-ratelimit-limit-tokens": "1000000",
                "x-ratelimit-remaining-tokens": "250000"
            },
            "provider": "openai"
        }"#;
        let params: ParseHeadersQuotaParams = serde_json::from_str(json).expect("deserialize ParseHeadersQuotaParams");
        assert_eq!(params.provider, Some("openai".to_string()));
        assert_eq!(params.headers.get("x-ratelimit-limit-tokens"), Some(&"1000000".to_string()));
    }
}
