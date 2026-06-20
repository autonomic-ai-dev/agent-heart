pub mod brain_client;
pub mod config;
pub mod mcp_server;
pub mod scheduler;

use anyhow::Result;
use config::Config;

pub async fn serve(config: Config) -> Result<()> {
    let brain_handle = brain_client::BrainHandle::start(&config).await?;

    // Start MCP server in background
    let mcp_brain = brain_handle.clone();
    let mcp_handle = tokio::spawn(async move {
        mcp_server::HeartMcp::run(mcp_brain).await.ok();
    });

    // Start cron scheduler
    if config.schedule.enabled {
        let mut sched = scheduler::start(&config, brain_handle.clone()).await?;
        tokio::signal::ctrl_c().await?;
        sched.shutdown().await?;
    } else {
        tokio::signal::ctrl_c().await?;
    }

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
    let stats = handle.call_distill(0.95, false).await?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    handle.shutdown().await;
    Ok(())
}
