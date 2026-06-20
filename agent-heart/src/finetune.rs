use anyhow::Result;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;

use crate::brain_client::BrainHandle;
use crate::config::FinetuneConfig;

pub async fn run_nightly_finetune(brain: &BrainHandle, config: &FinetuneConfig) -> Result<()> {
    info!("Nightly finetune: building dataset via agent-brain");
    let pipeline = brain.call_dataset_pipeline().await?;
    info!("Dataset pipeline: {}", pipeline);

    let merged = pipeline
        .get("merged_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            agent_body_core::memory_dir()
                .join("datasets")
                .join("spine.merged.jsonl")
        });

    let muscle = find_muscle_binary(config).await;
    let Some(muscle) = muscle else {
        anyhow::bail!("agent-muscle binary not found for nightly finetune");
    };

    info!(
        "Nightly finetune: training with {} on {}",
        muscle.display(),
        merged.display()
    );

    let child = Command::new(&muscle)
        .arg("train")
        .arg("--data")
        .arg(&merged)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to run agent-muscle train: {}", e))?;

    let status = child.wait_with_output().await?;
    if !status.status.success() {
        anyhow::bail!("agent-muscle train failed");
    }

    let state_dir = crate::config::state_dir();
    std::fs::create_dir_all(&state_dir).ok();
    let _ = std::fs::write(
        state_dir.join("last_finetune.txt"),
        chrono::Utc::now().to_rfc3339(),
    );

    Ok(())
}

async fn find_muscle_binary(config: &FinetuneConfig) -> Option<PathBuf> {
    if let Some(ref path) = config.muscle_binary {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    for path in [
        "/usr/local/bin/agent-muscle",
        "/opt/homebrew/bin/agent-muscle",
    ] {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Ok(output) = Command::new("which").arg("agent-muscle").output().await {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}
