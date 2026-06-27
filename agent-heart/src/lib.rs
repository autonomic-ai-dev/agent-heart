pub mod brain_client;
pub mod config;
pub mod finetune;
pub mod lint;
pub mod log;
pub mod mcp_server;
pub mod scheduler;
pub mod serve;
pub mod spine;
pub mod token_budget;
pub mod update;

use anyhow::Result;
use config::Config;

pub async fn serve_mcp(config: Config) -> Result<()> {
    let brain_handle = brain_client::BrainHandle::start(&config).await?;
    mcp_server::HeartMcp::run(brain_handle, config).await?;
    Ok(())
}

pub async fn serve(config: Config) -> Result<()> {
    let brain_handle = brain_client::BrainHandle::start(&config).await?;
    let config_clone = config.clone();

    // Start HTTP server in background
    let http_handle = tokio::spawn(async move {
        serve::start_http(config_clone).await.ok();
    });

    // Start MCP server in background
    let mcp_brain = brain_handle.clone();
    let mcp_config = config.clone();
    let mcp_handle = tokio::spawn(async move {
        mcp_server::HeartMcp::run(mcp_brain, mcp_config).await.ok();
    });

    // Start cron scheduler (non-fatal — daemon works without it)
    if config.schedule.enabled {
        match scheduler::start(&config, brain_handle.clone()).await {
            Ok(mut sched) => {
                tokio::signal::ctrl_c().await?;
                sched.shutdown().await?;
            }
            Err(e) => {
                tracing::warn!("Scheduler failed to start: {e}");
                tokio::signal::ctrl_c().await?;
            }
        }
    } else {
        tokio::signal::ctrl_c().await?;
    }

    http_handle.abort();
    mcp_handle.abort();
    Ok(())
}

pub async fn run_gc_once(config: Config) -> Result<()> {
    let handle = brain_client::BrainHandle::start(&config).await?;
    let stats = handle.call_gc(0.3, 90).await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    handle.shutdown().await;
    Ok(())
}

pub async fn run_distill_once(config: Config) -> Result<()> {
    let handle = brain_client::BrainHandle::start(&config).await?;
    let pipeline = handle.call_dataset_pipeline(Some(&config.finetune)).await?;
    println!("{}", serde_json::to_string_pretty(&pipeline)?);
    let stats = handle.call_distill(0.95, false).await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    handle.shutdown().await;
    Ok(())
}

pub async fn run_finetune_once(config: Config) -> Result<()> {
    let handle = brain_client::BrainHandle::start(&config).await?;
    finetune::run_nightly_finetune(&handle, &config.finetune).await?;
    handle.shutdown().await;
    Ok(())
}
