//! Predictive token budgeting from agent-brain retrieval_log history.

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config::TokenBudgetConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseTokenStats {
    pub phase: String,
    pub sample_count: u64,
    pub avg_tokens: u64,
    pub p95_tokens: u64,
    pub max_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetCheckRequest {
    pub phase: String,
    #[serde(default)]
    pub estimated_tokens: u64,
    #[serde(default)]
    pub task_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetCheckResponse {
    pub allowed: bool,
    pub frozen: bool,
    pub predicted_p95: u64,
    pub predicted_avg: u64,
    pub sample_count: u64,
    pub ceiling: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatsReport {
    pub brain_db: String,
    pub lookback_days: u32,
    pub phases: Vec<PhaseTokenStats>,
    pub global_avg_tokens: u64,
    pub global_p95_tokens: u64,
}

pub fn brain_db_path(config: &TokenBudgetConfig) -> PathBuf {
    config
        .brain_db_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| agent_body_core::memory_dir().join("brain.db"))
}

pub fn load_stats(config: &TokenBudgetConfig) -> Result<BudgetStatsReport> {
    let db_path = brain_db_path(config);
    let since_ms = lookback_since_ms(config.lookback_days);
    let phases = phase_stats(&db_path, since_ms)?;
    let global_samples = load_token_samples(&db_path, since_ms, None)?;
    let global_avg = average(&global_samples);
    let global_p95 = percentile(&global_samples, 95);

    Ok(BudgetStatsReport {
        brain_db: db_path.display().to_string(),
        lookback_days: config.lookback_days,
        phases,
        global_avg_tokens: global_avg,
        global_p95_tokens: global_p95,
    })
}

pub fn check_budget(
    config: &TokenBudgetConfig,
    req: &BudgetCheckRequest,
) -> Result<BudgetCheckResponse> {
    if !config.enabled {
        return Ok(BudgetCheckResponse {
            allowed: true,
            frozen: false,
            predicted_p95: 0,
            predicted_avg: 0,
            sample_count: 0,
            ceiling: config.max_tokens_per_route,
            reason: "token budget disabled".into(),
        });
    }

    let db_path = brain_db_path(config);
    if !db_path.exists() {
        return Ok(BudgetCheckResponse {
            allowed: true,
            frozen: false,
            predicted_p95: 0,
            predicted_avg: 0,
            sample_count: 0,
            ceiling: config.max_tokens_per_route,
            reason: format!(
                "brain.db not found at {} — allowing cold start",
                db_path.display()
            ),
        });
    }

    let since_ms = lookback_since_ms(config.lookback_days);
    let phase = normalize_phase(&req.phase);
    let samples = load_token_samples(&db_path, since_ms, Some(&phase))?;
    let sample_count = samples.len() as u64;
    let predicted_avg = average(&samples);
    let predicted_p95 = percentile(&samples, 95);
    let ceiling = config.max_tokens_per_route;
    let estimated = req.estimated_tokens.max(1);

    if estimated > ceiling {
        return Ok(deny(
            predicted_p95,
            predicted_avg,
            sample_count,
            ceiling,
            format!("estimated {estimated} tokens exceeds hard ceiling {ceiling}",),
        ));
    }

    if sample_count >= config.min_samples {
        let anomaly_threshold = ((predicted_p95 as f64) * config.anomaly_multiplier).ceil() as u64;
        if estimated > anomaly_threshold && anomaly_threshold > 0 {
            return Ok(deny(
                predicted_p95,
                predicted_avg,
                sample_count,
                ceiling,
                format!(
                    "estimated {estimated} tokens exceeds {:.1}x p95 ({predicted_p95}) → threshold {anomaly_threshold} — possible runaway LLM usage",
                    config.anomaly_multiplier
                ),
            ));
        }
    } else {
        return Ok(BudgetCheckResponse {
            allowed: true,
            frozen: false,
            predicted_p95,
            predicted_avg,
            sample_count,
            ceiling,
            reason: format!(
                "insufficient history ({sample_count}/{} samples) — allowing",
                config.min_samples
            ),
        });
    }

    Ok(BudgetCheckResponse {
        allowed: true,
        frozen: false,
        predicted_p95,
        predicted_avg,
        sample_count,
        ceiling,
        reason: "within predicted token budget".into(),
    })
}

fn deny(
    predicted_p95: u64,
    predicted_avg: u64,
    sample_count: u64,
    ceiling: u64,
    reason: String,
) -> BudgetCheckResponse {
    BudgetCheckResponse {
        allowed: false,
        frozen: true,
        predicted_p95,
        predicted_avg,
        sample_count,
        ceiling,
        reason,
    }
}

fn lookback_since_ms(days: u32) -> i64 {
    let now = chrono::Utc::now().timestamp_millis();
    now - i64::from(days.max(1)) * 24 * 3600 * 1000
}

fn normalize_phase(phase: &str) -> String {
    let p = phase.trim().to_lowercase();
    if p.is_empty() {
        "implementing".into()
    } else {
        p
    }
}

fn open_readonly(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open brain.db at {}", path.display()))
}

fn load_token_samples(path: &Path, since_ms: i64, phase: Option<&str>) -> Result<Vec<u64>> {
    let conn = open_readonly(path)?;
    let mut out = Vec::new();

    if let Some(phase) = phase {
        let mut stmt = conn.prepare(
            "SELECT tokens_used FROM retrieval_log WHERE timestamp >= ?1 AND phase != 'upstream_call' AND phase = ?2 ORDER BY tokens_used ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![since_ms, phase], |row| {
            row.get::<_, i64>(0)
        })?;
        for row in rows {
            out.push(row?.max(0) as u64);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT tokens_used FROM retrieval_log WHERE timestamp >= ?1 AND phase != 'upstream_call' ORDER BY tokens_used ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![since_ms], |row| row.get::<_, i64>(0))?;
        for row in rows {
            out.push(row?.max(0) as u64);
        }
    }

    Ok(out)
}

fn phase_stats(path: &Path, since_ms: i64) -> Result<Vec<PhaseTokenStats>> {
    let conn = open_readonly(path)?;
    let mut stmt = conn.prepare(
        "SELECT phase, tokens_used FROM retrieval_log WHERE timestamp >= ?1 AND phase != 'upstream_call'",
    )?;
    let rows = stmt.query_map(rusqlite::params![since_ms], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    let mut grouped: std::collections::HashMap<String, Vec<u64>> = std::collections::HashMap::new();
    for row in rows {
        let (phase, tokens) = row?;
        grouped.entry(phase).or_default().push(tokens.max(0) as u64);
    }

    let mut out: Vec<PhaseTokenStats> = grouped
        .into_iter()
        .map(|(phase, mut samples)| {
            samples.sort_unstable();
            PhaseTokenStats {
                sample_count: samples.len() as u64,
                avg_tokens: average(&samples),
                p95_tokens: percentile(&samples, 95),
                max_tokens: *samples.last().unwrap_or(&0),
                phase,
            }
        })
        .collect();
    out.sort_by(|a, b| a.phase.cmp(&b.phase));
    Ok(out)
}

fn average(samples: &[u64]) -> u64 {
    if samples.is_empty() {
        0
    } else {
        samples.iter().sum::<u64>() / samples.len() as u64
    }
}

fn percentile(sorted_or_any: &[u64], pct: u8) -> u64 {
    if sorted_or_any.is_empty() {
        return 0;
    }
    let mut v = sorted_or_any.to_vec();
    v.sort_unstable();
    let idx = ((v.len() as f64 * f64::from(pct) / 100.0).ceil() as usize)
        .saturating_sub(1)
        .min(v.len() - 1);
    v[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn seed_db(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE retrieval_log (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                query_hash TEXT,
                phase TEXT NOT NULL,
                items_returned TEXT,
                tokens_used INTEGER NOT NULL,
                truncated INTEGER NOT NULL,
                cache_hit INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        for (i, tokens) in [(100, 120), (101, 150), (102, 900), (103, 130)] {
            conn.execute(
                "INSERT INTO retrieval_log (id, timestamp, query_hash, phase, items_returned, tokens_used, truncated, cache_hit, latency_ms) VALUES (?1, ?2, 'q', 'implementing', '[]', ?3, 0, 0, 1)",
                rusqlite::params![i.to_string(), now, tokens],
            )
            .unwrap();
        }
    }

    #[test]
    fn blocks_anomaly_spike() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("brain.db");
        seed_db(&db);
        let config = TokenBudgetConfig {
            enabled: true,
            max_tokens_per_route: 5000,
            anomaly_multiplier: 2.0,
            lookback_days: 7,
            min_samples: 2,
            brain_db_path: Some(db.display().to_string()),
        };
        let resp = check_budget(
            &config,
            &BudgetCheckRequest {
                phase: "implementing".into(),
                estimated_tokens: 5000,
                task_kind: None,
            },
        )
        .unwrap();
        assert!(!resp.allowed);
        assert!(resp.frozen);
    }

    #[test]
    fn allows_normal_request() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("brain.db");
        seed_db(&db);
        let config = TokenBudgetConfig::default();
        let mut config = config;
        config.brain_db_path = Some(db.display().to_string());
        let resp = check_budget(
            &config,
            &BudgetCheckRequest {
                phase: "implementing".into(),
                estimated_tokens: 140,
                task_kind: None,
            },
        )
        .unwrap();
        assert!(resp.allowed);
    }
}
