use crate::usage_schema::{BudgetPolicy, ModelUsageEvent, ProviderQuotaSnapshot, UsageSummary};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};

pub struct UsageStore;

impl UsageStore {
    pub fn open_default() -> Result<Connection> {
        let dir = crate::config::state_dir();
        std::fs::create_dir_all(&dir)?;
        let db_path = dir.join("heart.db");
        let conn = Connection::open(db_path)?;
        Self::init(&conn)?;
        Ok(conn)
    }

    pub fn init(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS model_usage_events (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                session_id TEXT,
                organ TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                billing_type TEXT NOT NULL,
                prompt_tokens INTEGER NOT NULL,
                completion_tokens INTEGER NOT NULL,
                cache_read_tokens INTEGER NOT NULL,
                cache_write_tokens INTEGER NOT NULL,
                estimated_cost_usd REAL NOT NULL,
                latency_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_usage_events_timestamp ON model_usage_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_usage_events_organ ON model_usage_events(organ);

            CREATE TABLE IF NOT EXISTS provider_quotas (
                provider TEXT NOT NULL,
                window_type TEXT NOT NULL,
                account_or_tier TEXT NOT NULL,
                utilization_pct REAL NOT NULL,
                reset_at INTEGER,
                rate_limit_remaining INTEGER,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (provider, window_type)
            );

            CREATE TABLE IF NOT EXISTS budget_policies (
                id TEXT PRIMARY KEY,
                monthly_ceiling_usd REAL NOT NULL,
                soft_alert_threshold_pct REAL NOT NULL,
                hard_stop_threshold_pct REAL NOT NULL,
                auto_downgrade_model TEXT
            );

            CREATE TABLE IF NOT EXISTS daily_usage_rollups (
                date TEXT PRIMARY KEY,
                total_prompt_tokens INTEGER NOT NULL,
                total_completion_tokens INTEGER NOT NULL,
                total_cache_read_tokens INTEGER NOT NULL,
                total_cost_usd REAL NOT NULL,
                total_events INTEGER NOT NULL
            );
            ",
        )
        .context("Failed to initialize usage tables in SQLite")?;
        Ok(())
    }

    pub fn record_event(conn: &Connection, event: &ModelUsageEvent) -> Result<()> {
        let date_str = chrono::DateTime::from_timestamp_millis(event.timestamp)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());

        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "INSERT INTO model_usage_events (
                id, timestamp, session_id, organ, provider, model, billing_type,
                prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens,
                estimated_cost_usd, latency_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                event.id,
                event.timestamp,
                event.session_id,
                event.organ,
                event.provider,
                event.model,
                event.billing_type.to_string(),
                event.prompt_tokens as i64,
                event.completion_tokens as i64,
                event.cache_read_tokens as i64,
                event.cache_write_tokens as i64,
                event.estimated_cost_usd,
                event.latency_ms as i64,
            ],
        )?;

        tx.execute(
            "INSERT INTO daily_usage_rollups (
                date, total_prompt_tokens, total_completion_tokens, total_cache_read_tokens,
                total_cost_usd, total_events
            ) VALUES (?1, ?2, ?3, ?4, ?5, 1)
            ON CONFLICT(date) DO UPDATE SET
                total_prompt_tokens = total_prompt_tokens + excluded.total_prompt_tokens,
                total_completion_tokens = total_completion_tokens + excluded.total_completion_tokens,
                total_cache_read_tokens = total_cache_read_tokens + excluded.total_cache_read_tokens,
                total_cost_usd = total_cost_usd + excluded.total_cost_usd,
                total_events = total_events + 1",
            params![
                date_str,
                event.prompt_tokens as i64,
                event.completion_tokens as i64,
                event.cache_read_tokens as i64,
                event.estimated_cost_usd,
            ],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn record_quota(conn: &Connection, quota: &ProviderQuotaSnapshot) -> Result<()> {
        conn.execute(
            "INSERT INTO provider_quotas (
                provider, window_type, account_or_tier, utilization_pct, reset_at,
                rate_limit_remaining, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(provider, window_type) DO UPDATE SET
                account_or_tier = excluded.account_or_tier,
                utilization_pct = excluded.utilization_pct,
                reset_at = excluded.reset_at,
                rate_limit_remaining = excluded.rate_limit_remaining,
                updated_at = excluded.updated_at",
            params![
                quota.provider,
                quota.window_type,
                quota.account_or_tier,
                quota.utilization_pct,
                quota.reset_at,
                quota.rate_limit_remaining.map(|r| r as i64),
                quota.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_quota_snapshots(conn: &Connection) -> Result<Vec<ProviderQuotaSnapshot>> {
        let mut stmt = conn.prepare(
            "SELECT provider, window_type, account_or_tier, utilization_pct, reset_at,
                    rate_limit_remaining, updated_at
             FROM provider_quotas ORDER BY provider, window_type",
        )?;

        let rows = stmt.query_map([], |row| {
            let rate_limit: Option<i64> = row.get(5)?;
            Ok(ProviderQuotaSnapshot {
                provider: row.get(0)?,
                window_type: row.get(1)?,
                account_or_tier: row.get(2)?,
                utilization_pct: row.get(3)?,
                reset_at: row.get(4)?,
                rate_limit_remaining: rate_limit.map(|v| v as u64),
                updated_at: row.get(6)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn set_budget_policy(conn: &Connection, policy: &BudgetPolicy) -> Result<()> {
        conn.execute(
            "INSERT INTO budget_policies (
                id, monthly_ceiling_usd, soft_alert_threshold_pct, hard_stop_threshold_pct,
                auto_downgrade_model
            ) VALUES ('default', ?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                monthly_ceiling_usd = excluded.monthly_ceiling_usd,
                soft_alert_threshold_pct = excluded.soft_alert_threshold_pct,
                hard_stop_threshold_pct = excluded.hard_stop_threshold_pct,
                auto_downgrade_model = excluded.auto_downgrade_model",
            params![
                policy.monthly_ceiling_usd,
                policy.soft_alert_threshold_pct,
                policy.hard_stop_threshold_pct,
                policy.auto_downgrade_model,
            ],
        )?;
        Ok(())
    }

    pub fn get_budget_policy(conn: &Connection) -> Result<BudgetPolicy> {
        let mut stmt = conn.prepare(
            "SELECT monthly_ceiling_usd, soft_alert_threshold_pct, hard_stop_threshold_pct,
                    auto_downgrade_model
             FROM budget_policies WHERE id = 'default'",
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(BudgetPolicy {
                monthly_ceiling_usd: row.get(0)?,
                soft_alert_threshold_pct: row.get(1)?,
                hard_stop_threshold_pct: row.get(2)?,
                auto_downgrade_model: row.get(3)?,
            })
        } else {
            Ok(BudgetPolicy::default())
        }
    }

    pub fn query_usage_summary(conn: &Connection, lookback_days: u32) -> Result<UsageSummary> {
        let policy = Self::get_budget_policy(conn)?;
        let days = if lookback_days == 0 { 1 } else { lookback_days };

        let start_ts = Utc::now().timestamp_millis() - (days as i64 * 86_400_000);

        let mut stmt = conn.prepare(
            "SELECT
                COALESCE(SUM(prompt_tokens), 0),
                COALESCE(SUM(completion_tokens), 0),
                COALESCE(SUM(cache_read_tokens), 0),
                COALESCE(SUM(estimated_cost_usd), 0.0),
                COUNT(id)
             FROM model_usage_events
             WHERE timestamp >= ?1",
        )?;

        let mut rows = stmt.query(params![start_ts])?;
        let (prompt, compl, cache, cost, count): (i64, i64, i64, f64, i64) =
            if let Some(row) = rows.next()? {
                (
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                )
            } else {
                (0, 0, 0, 0.0, 0)
            };

        let daily_burn_rate_usd = cost / (days as f64);
        let days_in_month = 30.0;
        let day_of_month = (Utc::now()
            .format("%d")
            .to_string()
            .parse::<f64>()
            .unwrap_or(15.0))
        .min(30.0);
        let days_remaining = (days_in_month - day_of_month).max(0.0);
        let projected_month_end_usd = cost + (daily_burn_rate_usd * days_remaining);
        let remaining_budget_usd = (policy.monthly_ceiling_usd - cost).max(0.0);

        let soft_limit = policy.monthly_ceiling_usd * (policy.soft_alert_threshold_pct / 100.0);
        let hard_limit = policy.monthly_ceiling_usd * (policy.hard_stop_threshold_pct / 100.0);

        Ok(UsageSummary {
            lookback_days: days,
            total_prompt_tokens: prompt as u64,
            total_completion_tokens: compl as u64,
            total_cache_read_tokens: cache as u64,
            total_cost_usd: cost,
            total_events: count as u64,
            daily_burn_rate_usd,
            projected_month_end_usd,
            remaining_budget_usd,
            soft_alert_triggered: cost >= soft_limit,
            hard_stop_triggered: cost >= hard_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_schema::BillingType;

    #[test]
    fn test_store_init_and_record_event() {
        // Arrange
        let conn = Connection::open_in_memory().expect("in memory db");
        UsageStore::init(&conn).expect("init db");

        let event = ModelUsageEvent {
            id: "evt-001".to_string(),
            timestamp: Utc::now().timestamp_millis(),
            session_id: Some("session-123".to_string()),
            organ: "agent-brain".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-3-7-sonnet".to_string(),
            billing_type: BillingType::MeteredApi,
            prompt_tokens: 10_000,
            completion_tokens: 1_000,
            cache_read_tokens: 5_000,
            cache_write_tokens: 0,
            estimated_cost_usd: 0.0465,
            latency_ms: 850,
        };

        // Act
        UsageStore::record_event(&conn, &event).expect("record event");
        let summary = UsageStore::query_usage_summary(&conn, 7).expect("query summary");

        // Assert
        assert_eq!(summary.total_events, 1);
        assert_eq!(summary.total_prompt_tokens, 10_000);
        assert_eq!(summary.total_completion_tokens, 1_000);
        assert_eq!(summary.total_cache_read_tokens, 5_000);
        assert!((summary.total_cost_usd - 0.0465).abs() < 1e-4);
    }

    #[test]
    fn test_record_quota_and_budget_policy() {
        // Arrange
        let conn = Connection::open_in_memory().expect("in memory db");
        UsageStore::init(&conn).expect("init db");

        let quota = ProviderQuotaSnapshot {
            provider: "anthropic".to_string(),
            account_or_tier: "claude-code-pro".to_string(),
            window_type: "5h_rolling".to_string(),
            utilization_pct: 42.0,
            reset_at: Some(Utc::now().timestamp_millis() + 7_200_000),
            rate_limit_remaining: Some(250_000),
            updated_at: Utc::now().timestamp_millis(),
        };

        let policy = BudgetPolicy {
            monthly_ceiling_usd: 50.0,
            soft_alert_threshold_pct: 75.0,
            hard_stop_threshold_pct: 90.0,
            auto_downgrade_model: Some("claude-3-5-haiku".to_string()),
        };

        // Act
        UsageStore::record_quota(&conn, &quota).expect("record quota");
        UsageStore::set_budget_policy(&conn, &policy).expect("set policy");

        let quotas = UsageStore::get_quota_snapshots(&conn).expect("get quotas");
        let retrieved_policy = UsageStore::get_budget_policy(&conn).expect("get policy");

        // Assert
        assert_eq!(quotas.len(), 1);
        assert_eq!(quotas[0].provider, "anthropic");
        assert_eq!(quotas[0].utilization_pct, 42.0);
        assert_eq!(retrieved_policy.monthly_ceiling_usd, 50.0);
        assert_eq!(
            retrieved_policy.auto_downgrade_model,
            Some("claude-3-5-haiku".to_string())
        );
    }

    #[test]
    fn test_soft_and_hard_budget_alerts() {
        // Arrange
        let conn = Connection::open_in_memory().expect("in memory db");
        UsageStore::init(&conn).expect("init db");

        let policy = BudgetPolicy {
            monthly_ceiling_usd: 100.0,
            soft_alert_threshold_pct: 80.0,
            hard_stop_threshold_pct: 95.0,
            auto_downgrade_model: Some("claude-3-5-haiku".to_string()),
        };
        UsageStore::set_budget_policy(&conn, &policy).expect("set policy");

        // Act 1: Record event with $50 spend (50% - no alert)
        let event1 = ModelUsageEvent {
            id: "evt-001".to_string(),
            timestamp: Utc::now().timestamp_millis(),
            session_id: None,
            organ: "agent-brain".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-3-7-sonnet".to_string(),
            billing_type: BillingType::MeteredApi,
            prompt_tokens: 1_000_000,
            completion_tokens: 500_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 50.0,
            latency_ms: 1200,
        };
        UsageStore::record_event(&conn, &event1).expect("record event 1");
        let summary1 = UsageStore::query_usage_summary(&conn, 30).expect("query summary");

        // Assert 1
        assert!(!summary1.soft_alert_triggered);
        assert!(!summary1.hard_stop_triggered);

        // Act 2: Record event bringing total to $82 (82% - soft alert)
        let event2 = ModelUsageEvent {
            id: "evt-002".to_string(),
            timestamp: Utc::now().timestamp_millis(),
            session_id: None,
            organ: "agent-brain".to_string(),
            provider: "anthropic".to_string(),
            model: "claude-3-7-sonnet".to_string(),
            billing_type: BillingType::MeteredApi,
            prompt_tokens: 1_000_000,
            completion_tokens: 200_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 32.0,
            latency_ms: 1000,
        };
        UsageStore::record_event(&conn, &event2).expect("record event 2");
        let summary2 = UsageStore::query_usage_summary(&conn, 30).expect("query summary");

        // Assert 2
        assert!(summary2.soft_alert_triggered);
        assert!(!summary2.hard_stop_triggered);

        // Act 3: Record event bringing total to $96 (96% - hard stop)
        let event3 = ModelUsageEvent {
            id: "evt-003".to_string(),
            timestamp: Utc::now().timestamp_millis(),
            session_id: None,
            organ: "agent-brain".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            billing_type: BillingType::MeteredApi,
            prompt_tokens: 500_000,
            completion_tokens: 100_000,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated_cost_usd: 14.0,
            latency_ms: 800,
        };
        UsageStore::record_event(&conn, &event3).expect("record event 3");
        let summary3 = UsageStore::query_usage_summary(&conn, 30).expect("query summary");

        // Assert 3
        assert!(summary3.soft_alert_triggered);
        assert!(summary3.hard_stop_triggered);
    }

    #[test]
    fn test_multi_day_aggregation_and_burn_rate() {
        // Arrange
        let conn = Connection::open_in_memory().expect("in memory db");
        UsageStore::init(&conn).expect("init db");

        let now_ms = Utc::now().timestamp_millis();
        for i in 0..5 {
            let event = ModelUsageEvent {
                id: format!("evt-day-{i}"),
                timestamp: now_ms - (i as i64 * 86_400_000),
                session_id: Some("session-multi".to_string()),
                organ: "agent-body".to_string(),
                provider: "google".to_string(),
                model: "gemini-2.0-flash".to_string(),
                billing_type: BillingType::MeteredApi,
                prompt_tokens: 20_000,
                completion_tokens: 5_000,
                cache_read_tokens: 10_000,
                cache_write_tokens: 0,
                estimated_cost_usd: 2.0,
                latency_ms: 400,
            };
            UsageStore::record_event(&conn, &event).expect("record multi-day event");
        }

        // Act
        let summary = UsageStore::query_usage_summary(&conn, 7).expect("query 7 days summary");

        // Assert
        assert_eq!(summary.total_events, 5);
        assert_eq!(summary.total_prompt_tokens, 100_000);
        assert_eq!(summary.total_completion_tokens, 25_000);
        assert_eq!(summary.total_cache_read_tokens, 50_000);
        assert!((summary.total_cost_usd - 10.0).abs() < 1e-4);
        assert!(summary.daily_burn_rate_usd > 0.0);
    }
}
