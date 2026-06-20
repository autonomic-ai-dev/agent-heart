use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

use crate::brain_client::BrainHandle;
use crate::config::Config;

pub async fn start(config: &Config, brain: BrainHandle) -> Result<JobScheduler> {
    let sched = JobScheduler::new().await?;
    let cron_expr = config.schedule.cron.clone();
    let gc_brain = brain.clone();

    let job = Job::new_async(cron_expr.as_str(), move |_uuid, _lock| {
        let brain = gc_brain.clone();
        Box::pin(async move {
            info!("Cron tick: running brain_gc");
            match brain.call_gc(0.3, 90).await {
                Ok(stats) => {
                    info!("GC complete: {}", stats);
                    let state_dir = crate::config::state_dir();
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

    if config.finetune.enabled {
        let finetune_cfg = config.finetune.clone();
        let finetune_cron = finetune_cfg.cron.clone();
        let finetune_brain = brain.clone();
        let finetune_job = Job::new_async(finetune_cron.as_str(), move |_uuid, _lock| {
            let cfg = finetune_cfg.clone();
            let brain = finetune_brain.clone();
            Box::pin(async move {
                info!("Cron tick: nightly finetune pipeline");
                if let Err(e) = crate::finetune::run_nightly_finetune(&brain, &cfg).await {
                    tracing::error!("Nightly finetune failed: {}", e);
                }
            })
        })?;
        sched.add(finetune_job).await?;
        info!(
            "Finetune scheduler started: cron='{}'",
            config.finetune.cron
        );
    }

    sched.start().await?;
    info!("Scheduler started: cron='{}'", cron_expr);
    Ok(sched)
}
