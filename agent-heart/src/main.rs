use clap::{Parser, Subcommand};
use std::path::PathBuf;

fn config_path() -> PathBuf {
    agent_heart::config::config_path()
}

#[derive(Parser)]
#[command(name = "agent-heart", about = "Background distillation daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon (MCP server + cron scheduler)
    Serve,
    /// Run GC once and exit
    Gc,
    /// Run cluster distillation once and exit
    Distill,
    /// Show daemon status
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = agent_heart::config::Config::load()?;

    match cli.command {
        Commands::Serve => agent_heart::serve(config).await?,
        Commands::Gc => agent_heart::run_gc_once(config).await?,
        Commands::Distill => agent_heart::run_distill_once(config).await?,
        Commands::Status => {
            let state_dir = dirs::state_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("agent-heart");
            let last_gc = std::fs::read_to_string(state_dir.join("last_gc.txt"))
                .unwrap_or_else(|_| "never".into());
            println!("agent-heart status");
            println!("  config: {}", config_path().display());
            println!(
                "  schedule: {} (enabled: {})",
                config.schedule.cron, config.schedule.enabled
            );
            println!("  last_gc: {}", last_gc);
        }
    }
    Ok(())
}
