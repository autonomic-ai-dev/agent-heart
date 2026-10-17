use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use tracing::info;
use uuid::Uuid;

use crate::config::Config;
use crate::token_budget::brain_db_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfLearningReport {
    pub analyzed_outcomes: usize,
    pub successful_outcomes: usize,
    pub failed_outcomes: usize,
    pub synthesized_facts: usize,
    pub timestamp: String,
}

impl SelfLearningReport {
    pub fn empty() -> Self {
        Self {
            analyzed_outcomes: 0,
            successful_outcomes: 0,
            failed_outcomes: 0,
            synthesized_facts: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub fn is_enabled(config: &Config) -> bool {
    if let Ok(val) = std::env::var("AUTONOMIC_SELF_LEARNING") {
        let v = val.trim().to_ascii_lowercase();
        return v == "1" || v == "true" || v == "yes";
    }
    config.self_learning.enabled
}

pub fn run_self_learning(config: &Config) -> Result<SelfLearningReport> {
    let db_path = brain_db_path(&config.token_budget);
    if !db_path.exists() {
        return Ok(SelfLearningReport::empty());
    }

    let conn = Connection::open(&db_path)
        .with_context(|| format!("open brain.db at {}", db_path.display()))?;

    let state_dir = crate::config::state_dir();
    fs::create_dir_all(&state_dir).ok();
    let state_file = state_dir.join("last_learning_ms.txt");
    let last_ms: i64 = fs::read_to_string(&state_file)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        r#"SELECT id, exit_code, cause_tag, cmd, recorded_at
           FROM verify_outcomes
           WHERE recorded_at > ?1
           ORDER BY recorded_at ASC
           LIMIT 100"#,
    )?;

    struct OutcomeRow {
        _id: String,
        exit_code: i32,
        cause_tag: Option<String>,
        cmd: String,
        recorded_at: i64,
    }

    let rows = stmt
        .query_map(params![last_ms], |row| {
            Ok(OutcomeRow {
                _id: row.get(0)?,
                exit_code: row.get(1)?,
                cause_tag: row.get(2)?,
                cmd: row.get(3)?,
                recorded_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(SelfLearningReport::empty());
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut success_count = 0;
    let mut failure_count = 0;
    let mut synthesized_count = 0;
    let mut max_recorded_at = last_ms;

    for row in &rows {
        if row.recorded_at > max_recorded_at {
            max_recorded_at = row.recorded_at;
        }

        let is_success = row.exit_code == 0;
        if is_success {
            success_count += 1;
        } else {
            failure_count += 1;
        }

        let (topic, fact_text) = if is_success {
            (
                "autonomic/reinforcement",
                format!(
                    "Command `{}` verified successfully with clean exit code 0.",
                    truncate_str(&row.cmd, 120)
                ),
            )
        } else {
            let cause = row.cause_tag.as_deref().unwrap_or("unknown");
            (
                "autonomic/anti-pattern",
                format!(
                    "Command `{}` failed verification with exit code {} (cause: {}). Ensure flags, arguments, and dependencies are verified before invocation.",
                    truncate_str(&row.cmd, 120),
                    row.exit_code,
                    cause
                ),
            )
        };

        let mut hasher = Sha256::new();
        hasher.update(topic.as_bytes());
        hasher.update(fact_text.as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        let id = Uuid::new_v4().to_string();
        let inserted = conn.execute(
            r#"INSERT OR IGNORE INTO facts
               (id, topic, fact, scope, source, confidence, created_at, updated_at, content_hash)
               VALUES (?1, ?2, ?3, 'global', 'heart-critic', 0.85, ?4, ?4, ?5)"#,
            params![id, topic, fact_text, now_ms, content_hash],
        );

        if let Ok(n) = inserted {
            if n > 0 {
                synthesized_count += 1;
            }
        }
    }

    if max_recorded_at > last_ms {
        let _ = fs::write(&state_file, max_recorded_at.to_string());
    }

    let report = SelfLearningReport {
        analyzed_outcomes: rows.len(),
        successful_outcomes: success_count,
        failed_outcomes: failure_count,
        synthesized_facts: synthesized_count,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    info!(
        "Self-learning cycle: analyzed={}, successes={}, failures={}, synthesized_facts={}",
        report.analyzed_outcomes,
        report.successful_outcomes,
        report.failed_outcomes,
        report.synthesized_facts
    );

    Ok(report)
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello");
    }

    #[test]
    fn test_empty_report() {
        let r = SelfLearningReport::empty();
        assert_eq!(r.analyzed_outcomes, 0);
        assert_eq!(r.synthesized_facts, 0);
    }
}
