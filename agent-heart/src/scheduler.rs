use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

use crate::brain_client::BrainHandle;
use crate::config::Config;

pub async fn start(config: &Config, brain: BrainHandle) -> Result<JobScheduler> {
    let sched = JobScheduler::new().await?;
    let cron_expr = config.schedule.cron.clone();

    let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
        let brain = brain.clone();
        Box::pin(async move {
            info!("Cron tick: running brain_gc");
            match brain.call_gc(0.3, 90).await {
                Ok(stats) => {
                    info!("GC complete: {}", stats);
                    let state_dir = dirs::state_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                        .join("agent-heart");
                    std::fs::create_dir_all(&state_dir).ok();
                    let _ = std::fs::write(
                        state_dir.join("last_gc.txt"),
                        chrono::Utc::now().to_rfc3339(),
                    );
                }
                Err(e) => tracing::error!("GC failed: {}", e),
            }
        })
    })?;

    sched.add(job).await?;
    sched.start().await?;
    info!("Scheduler started: cron='{}'", cron_expr);
    Ok(sched)
}
