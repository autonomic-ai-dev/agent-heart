//! Predictive token budgeting from agent-brain retrieval_log history.

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::budget_vector::BudgetVector;
use crate::config::TokenBudgetConfig;
use std::collections::HashMap;
use std::sync::Mutex;

static PID: Mutex<Option<HashMap<String, crate::pid_throttle::PidState>>> = Mutex::new(None);

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
    #[serde(default)]
    pub estimated_wasm_fuel: Option<u64>,
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
    pub wasm_p95: u64,
    pub wasm_ceiling: u64,
    #[serde(default = "default_throttle_factor")]
    pub throttle_factor: f64,
    #[serde(default)]
    pub budget_vector: BudgetVector,
}

fn default_throttle_factor() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatsReport {
    pub brain_db: String,
    pub lookback_days: u32,
    pub phases: Vec<PhaseTokenStats>,
    pub global_avg_tokens: u64,
    pub global_p95_tokens: u64,
    pub global_wasm_fuel_p95: u64,
    pub global_wasm_fuel_avg: u64,
    pub wasm_sample_count: u64,
}

pub fn brain_db_path(config: &TokenBudgetConfig) -> PathBuf {
    config
        .brain_db_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| agent_body_core::memory_dir().join("brain.db"))
}

pub fn heart_db_path(config: &TokenBudgetConfig) -> PathBuf {
    config
        .heart_db_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::config::state_dir().join("heart.db"))
}

pub fn ensure_wasm_schema(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS wasm_executions (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            job_id TEXT,
            organ TEXT NOT NULL,
            fuel_consumed INTEGER NOT NULL,
            memory_peak_bytes INTEGER,
            success INTEGER NOT NULL,
            latency_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_wasm_executions_ts ON wasm_executions(timestamp);",
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmExecutionRecord {
    pub id: String,
    pub job_id: Option<String>,
    pub organ: String,
    pub fuel_consumed: u64,
    pub memory_peak_bytes: Option<u64>,
    pub success: bool,
    pub latency_ms: u64,
}

pub fn record_wasm_execution(config: &TokenBudgetConfig, rec: &WasmExecutionRecord) -> Result<()> {
    let path = heart_db_path(config);
    ensure_wasm_schema(&path)?;
    let conn = Connection::open(&path)?;
    let ts = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO wasm_executions (id, timestamp, job_id, organ, fuel_consumed, memory_peak_bytes, success, latency_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            rec.id,
            ts,
            rec.job_id,
            rec.organ,
            i64::try_from(rec.fuel_consumed).unwrap_or(i64::MAX),
            rec.memory_peak_bytes.map(|v| i64::try_from(v).unwrap_or(i64::MAX)),
            i64::from(rec.success),
            i64::try_from(rec.latency_ms).unwrap_or(0),
        ],
    )?;
    Ok(())
}

fn load_wasm_fuel_samples(path: &Path, since_ms: i64) -> Result<Vec<u64>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    ensure_wasm_schema(path)?;
    let conn = Connection::open(path)?;
    let mut stmt = conn.prepare(
        "SELECT fuel_consumed FROM wasm_executions WHERE timestamp >= ?1 AND success = 1 ORDER BY fuel_consumed ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![since_ms], |row| row.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?.max(0) as u64);
    }
    Ok(out)
}

pub fn load_stats(config: &TokenBudgetConfig) -> Result<BudgetStatsReport> {
    let db_path = brain_db_path(config);
    let since_ms = lookback_since_ms(config.lookback_days);
    let (phases, global_samples) = if db_path.exists() {
        (
            phase_stats(&db_path, since_ms)?,
            load_token_samples(&db_path, since_ms, None)?,
        )
    } else {
        (Vec::new(), Vec::new())
    };
    let global_avg = average(&global_samples);
    let global_p95 = percentile(&global_samples, 95);
    let heart_path = heart_db_path(config);
    let wasm_samples = load_wasm_fuel_samples(&heart_path, since_ms).unwrap_or_default();
    let wasm_avg = average(&wasm_samples);
    let wasm_p95 = percentile(&wasm_samples, 95);

    Ok(BudgetStatsReport {
        brain_db: db_path.display().to_string(),
        lookback_days: config.lookback_days,
        phases,
        global_avg_tokens: global_avg,
        global_p95_tokens: global_p95,
        global_wasm_fuel_p95: wasm_p95,
        global_wasm_fuel_avg: wasm_avg,
        wasm_sample_count: wasm_samples.len() as u64,
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
            wasm_p95: 0,
            wasm_ceiling: config.max_wasm_fuel_per_route,
            throttle_factor: 1.0,
            budget_vector: BudgetVector::default(),
        });
    }

    let db_path = brain_db_path(config);
    let since_ms = lookback_since_ms(config.lookback_days);
    let phase = normalize_phase(&req.phase);
    let samples = if db_path.exists() {
        load_token_samples(&db_path, since_ms, Some(&phase))?
    } else {
        Vec::new()
    };
    let sample_count = samples.len() as u64;
    let predicted_avg = average(&samples);
    let predicted_p95 = percentile(&samples, 95);
    let ceiling = config.max_tokens_per_route;
    let estimated = req.estimated_tokens.max(1);
    let heart_path = heart_db_path(config);
    let wasm_samples = load_wasm_fuel_samples(&heart_path, since_ms).unwrap_or_default();
    let wasm_p95 = percentile(&wasm_samples, 95);
    let wasm_ceiling = config.max_wasm_fuel_per_route;
    let estimated_wasm = req.estimated_wasm_fuel.unwrap_or(0);

    if estimated_wasm > wasm_ceiling {
        return Ok(deny(
            predicted_p95,
            predicted_avg,
            sample_count,
            ceiling,
            format!("estimated {estimated_wasm} WASM fuel exceeds ceiling {wasm_ceiling}"),
            wasm_p95,
            wasm_ceiling,
        ));
    }

    if estimated > ceiling {
        return Ok(deny(
            predicted_p95,
            predicted_avg,
            sample_count,
            ceiling,
            format!("estimated {estimated} tokens exceeds hard ceiling {ceiling}",),
            wasm_p95,
            wasm_ceiling,
        ));
    }

    if sample_count >= config.min_samples {
        let anomaly_threshold = ((predicted_p95 as f64) * config.anomaly_multiplier).ceil() as u64;
        if estimated > anomaly_threshold && anomaly_threshold > 0 {
            let mut pid_map = PID.lock().unwrap();
            if pid_map.is_none() {
                *pid_map = Some(HashMap::new());
            }
            let state = pid_map.as_mut().unwrap().entry(phase.clone()).or_default();
            let throttle_factor = crate::pid_throttle::throttle_factor(state, estimated, predicted_p95.max(1));
            if throttle_factor >= 0.35 {
                return Ok(BudgetCheckResponse {
                    allowed: true,
                    frozen: false,
                    predicted_p95,
                    predicted_avg,
                    sample_count,
                    ceiling,
                    reason: format!(
                        "PID throttle {:.2} instead of freeze (est {estimated} > {anomaly_threshold})",
                        throttle_factor
                    ),
                    wasm_p95,
                    wasm_ceiling,
                    throttle_factor,
                    budget_vector: budget_vector_for(req, ceiling, wasm_ceiling),
                });
            }
            return Ok(deny(
                predicted_p95,
                predicted_avg,
                sample_count,
                ceiling,
                format!(
                    "estimated {estimated} tokens exceeds {:.1}x p95 ({predicted_p95}) → threshold {anomaly_threshold} — possible runaway LLM usage",
                    config.anomaly_multiplier
                ),
                wasm_p95,
                wasm_ceiling,
            ));
        }
    }

    if estimated_wasm > 0 && wasm_samples.len() as u64 >= config.min_samples {
        let wasm_threshold = ((wasm_p95 as f64) * config.wasm_anomaly_multiplier).ceil() as u64;
        if estimated_wasm > wasm_threshold && wasm_threshold > 0 {
            return Ok(deny(
                predicted_p95,
                predicted_avg,
                sample_count,
                ceiling,
                format!(
                    "estimated {estimated_wasm} WASM fuel exceeds {:.1}x p95 ({wasm_p95}) → threshold {wasm_threshold}",
                    config.wasm_anomaly_multiplier
                ),
                wasm_p95,
                wasm_ceiling,
            ));
        }
    }

    let reason = if sample_count < config.min_samples {
        if db_path.exists() {
            format!(
                "insufficient token history ({sample_count}/{} samples) — allowing",
                config.min_samples
            )
        } else {
            format!(
                "brain.db not found at {} — allowing token cold start",
                db_path.display()
            )
        }
    } else {
        "within predicted token and WASM fuel budget".into()
    };

    let mut pid_map = PID.lock().unwrap();
    if pid_map.is_none() {
        *pid_map = Some(HashMap::new());
    }
    let state = pid_map.as_mut().unwrap().entry(phase.clone()).or_default();
    let throttle_factor = crate::pid_throttle::throttle_factor(state, estimated, predicted_p95.max(1));
    let (allowed, frozen, reason) = if throttle_factor < 0.35 {
        (
            false,
            true,
            format!(
                "PID throttle {:.2} below floor — gradual slowdown active (p95 {predicted_p95})",
                throttle_factor
            ),
        )
    } else if throttle_factor < 1.0 {
        (
            true,
            false,
            format!(
                "PID throttle {:.2} — within budget but throttled",
                throttle_factor
            ),
        )
    } else {
        (true, false, reason)
    };

    Ok(BudgetCheckResponse {
        allowed,
        frozen,
        predicted_p95,
        predicted_avg,
        sample_count,
        ceiling,
        reason,
        wasm_p95,
        wasm_ceiling,
        throttle_factor,
        budget_vector: budget_vector_for(req, ceiling, wasm_ceiling),
    })
}

fn budget_vector_for(req: &BudgetCheckRequest, token_ceiling: u64, wasm_ceiling: u64) -> BudgetVector {
    BudgetVector::from_usage(
        req.estimated_tokens,
        req.estimated_wasm_fuel.unwrap_or(0),
        0,
        BudgetVector {
            tokens: token_ceiling.max(1) as f64,
            wasm_fuel: wasm_ceiling.max(1) as f64,
            latency_ms: 5_000.0,
        },
    )
}

fn deny(
    predicted_p95: u64,
    predicted_avg: u64,
    sample_count: u64,
    ceiling: u64,
    reason: String,
    wasm_p95: u64,
    wasm_ceiling: u64,
) -> BudgetCheckResponse {
    BudgetCheckResponse {
        allowed: false,
        frozen: true,
        predicted_p95,
        predicted_avg,
        sample_count,
        ceiling,
        reason,
        wasm_p95,
        wasm_ceiling,
        throttle_factor: 0.0,
        budget_vector: BudgetVector::default(),
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
            ..TokenBudgetConfig::default()
        };
        let resp = check_budget(
            &config,
            &BudgetCheckRequest {
                phase: "implementing".into(),
                estimated_tokens: 5000,
                task_kind: None,
                estimated_wasm_fuel: None,
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
                estimated_wasm_fuel: None,
            },
        )
        .unwrap();
        assert!(resp.allowed);
    }

    fn seed_wasm_db(path: &Path) {
        let config = TokenBudgetConfig {
            heart_db_path: Some(path.display().to_string()),
            ..TokenBudgetConfig::default()
        };
        for (i, fuel) in [(1, 1000u64), (2, 1200), (3, 9000), (4, 1100)] {
            record_wasm_execution(
                &config,
                &WasmExecutionRecord {
                    id: format!("w{i}"),
                    job_id: Some(format!("job-{i}")),
                    organ: "immune".into(),
                    fuel_consumed: fuel,
                    memory_peak_bytes: Some(4096),
                    success: true,
                    latency_ms: 5,
                },
            )
            .unwrap();
        }
    }

    #[test]
    fn wasm_fuel_p95_tracking() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("heart.db");
        seed_wasm_db(&db);
        let config = TokenBudgetConfig {
            enabled: true,
            max_wasm_fuel_per_route: 50_000,
            wasm_anomaly_multiplier: 2.0,
            min_samples: 2,
            heart_db_path: Some(db.display().to_string()),
            ..TokenBudgetConfig::default()
        };
        let stats = load_stats(&config).unwrap();
        assert!(stats.wasm_sample_count >= 4);
        assert!(stats.global_wasm_fuel_p95 >= 1100);
        let resp = check_budget(
            &config,
            &BudgetCheckRequest {
                phase: "implementing".into(),
                estimated_tokens: 100,
                task_kind: None,
                estimated_wasm_fuel: Some(20_000),
            },
        )
        .unwrap();
        assert!(!resp.allowed);
        assert!(resp.wasm_p95 > 0);
    }
}
