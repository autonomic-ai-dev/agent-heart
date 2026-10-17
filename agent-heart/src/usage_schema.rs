use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingType {
    MeteredApi,
    SubscriptionQuota,
    LocalFree,
}

impl std::fmt::Display for BillingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MeteredApi => write!(f, "metered_api"),
            Self::SubscriptionQuota => write!(f, "subscription_quota"),
            Self::LocalFree => write!(f, "local_free"),
        }
    }
}

impl std::str::FromStr for BillingType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "metered_api" | "metered" => Ok(Self::MeteredApi),
            "subscription_quota" | "subscription" => Ok(Self::SubscriptionQuota),
            "local_free" | "local" | "free" => Ok(Self::LocalFree),
            other => anyhow::bail!("unknown billing type: {other}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelUsageEvent {
    pub id: String,
    pub timestamp: i64,
    pub session_id: Option<String>,
    pub organ: String,
    pub provider: String,
    pub model: String,
    pub billing_type: BillingType,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub estimated_cost_usd: f64,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderQuotaSnapshot {
    pub provider: String,
    pub account_or_tier: String,
    pub window_type: String,
    pub utilization_pct: f64,
    pub reset_at: Option<i64>,
    pub rate_limit_remaining: Option<u64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetPolicy {
    pub monthly_ceiling_usd: f64,
    pub soft_alert_threshold_pct: f64,
    pub hard_stop_threshold_pct: f64,
    pub auto_downgrade_model: Option<String>,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            monthly_ceiling_usd: 100.0,
            soft_alert_threshold_pct: 80.0,
            hard_stop_threshold_pct: 95.0,
            auto_downgrade_model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DailyUsageRollup {
    pub date: String,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub total_events: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageSummary {
    pub lookback_days: u32,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cost_usd: f64,
    pub total_events: u64,
    pub daily_burn_rate_usd: f64,
    pub projected_month_end_usd: f64,
    pub remaining_budget_usd: f64,
    pub soft_alert_triggered: bool,
    pub hard_stop_triggered: bool,
}
