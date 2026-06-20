use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::config::Config;

#[derive(Clone)]
pub struct BrainHandle {
    binary: Option<PathBuf>,
}

impl BrainHandle {
    pub async fn start(config: &Config) -> Result<Self> {
        let binary = find_brain_binary(config).await;
        Ok(Self { binary })
    }

    pub async fn call_gc(&self, min_confidence: f64, max_age_days: u64) -> Result<Value> {
        let binary = self
            .binary
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("agent-brain binary not found"))?;

        let child = Command::new(binary)
            .arg("gc")
            .arg("--min-confidence")
            .arg(min_confidence.to_string())
            .arg("--max-age-days")
            .arg(max_age_days.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to run agent-brain gc: {}", e))?;

        let output = child.wait_with_output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("agent-brain gc exited with {}: {}", output.status, stderr);
        }

        let stats: Value = serde_json::from_slice(&output.stdout)?;
        Ok(stats)
    }

    pub async fn shutdown(&self) {
        // Process exits on its own — nothing to clean up
    }
}

async fn find_brain_binary(config: &Config) -> Option<PathBuf> {
    if let Some(ref path) = config.brain.binary_path {
        let p = PathBuf::from(path);
        if p.exists() {
            info!("Using configured brain path: {}", p.display());
            return Some(p);
        }
    }

    let common = [
        "/usr/local/bin/agent-brain",
        "/opt/homebrew/bin/agent-brain",
    ];
    for path in &common {
        let p = PathBuf::from(path);
        if p.exists() {
            info!("Found agent-brain at {}", p.display());
            return Some(p);
        }
    }

    if let Ok(output) = Command::new("which").arg("agent-brain").output().await {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            let p = PathBuf::from(&path);
            if p.exists() {
                info!("Found agent-brain via which: {}", p.display());
                return Some(p);
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        for p in [".cargo/bin/agent-brain", ".local/bin/agent-brain"] {
            let pb = PathBuf::from(&home).join(p);
            if pb.exists() {
                info!("Found agent-brain at {}", pb.display());
                return Some(pb);
            }
        }
    }

    tracing::warn!("agent-brain binary not found — check config or PATH");
    None
}
