//! Execution outcomes reported by other organs (for example agent-muscle), stored next to heart's usage data.

use std::path::Path;

use anyhow::{bail, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const MAX_LABEL_CHARS: usize = 64;

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS execution_outcomes (
    id TEXT PRIMARY KEY,
    timestamp INTEGER NOT NULL,
    organ TEXT NOT NULL,
    kind TEXT NOT NULL,
    success INTEGER NOT NULL,
    exit_code INTEGER,
    duration_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_execution_outcomes_ts ON execution_outcomes(timestamp);";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub id: String,
    pub organ: String,
    pub kind: String,
    pub success: bool,
    pub exit_code: Option<i64>,
    pub duration_ms: u64,
}

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.chars().count() <= MAX_LABEL_CHARS
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

pub fn record(path: &Path, outcome: &ExecutionOutcome) -> Result<()> {
    if !valid_label(&outcome.id) || !valid_label(&outcome.organ) || !valid_label(&outcome.kind) {
        bail!(
            "outcome id, organ and kind must be short labels of letters, digits, '-', '_' or '.'"
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.execute_batch(SCHEMA)?;
    let ts = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO execution_outcomes (id, timestamp, organ, kind, success, exit_code, duration_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            outcome.id,
            ts,
            outcome.organ,
            outcome.kind,
            outcome.success,
            outcome.exit_code,
            i64::try_from(outcome.duration_ms).unwrap_or(i64::MAX),
        ],
    )?;
    Ok(())
}

pub fn count(path: &Path) -> Result<i64> {
    if !path.exists() {
        return Ok(0);
    }
    let conn = Connection::open(path)?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'execution_outcomes'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(0);
    }
    Ok(
        conn.query_row("SELECT COUNT(*) FROM execution_outcomes", [], |row| {
            row.get(0)
        })?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(id: &str) -> ExecutionOutcome {
        ExecutionOutcome {
            id: id.into(),
            organ: "agent-muscle".into(),
            kind: "bash".into(),
            success: true,
            exit_code: Some(0),
            duration_ms: 12,
        }
    }

    fn fresh_db(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("heart-outcomes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("heart.db")
    }

    #[test]
    fn recorded_outcome_is_counted() {
        let path = fresh_db("record");
        record(&path, &outcome("job-1")).unwrap();
        assert_eq!(count(&path).unwrap(), 1);
    }

    #[test]
    fn same_job_id_is_not_double_counted() {
        let path = fresh_db("dedupe");
        record(&path, &outcome("job-1")).unwrap();
        record(&path, &outcome("job-1")).unwrap();
        assert_eq!(count(&path).unwrap(), 1);
    }

    #[test]
    fn missing_database_counts_as_zero() {
        assert_eq!(count(&fresh_db("missing")).unwrap(), 0);
    }

    #[test]
    fn labels_with_spaces_or_slashes_are_rejected() {
        let path = fresh_db("labels");
        let mut bad = outcome("job 1");
        assert!(record(&path, &bad).is_err());
        bad = outcome("../escape");
        assert!(record(&path, &bad).is_err());
        assert_eq!(count(&path).unwrap(), 0);
    }
}
