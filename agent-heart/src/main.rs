use clap::{Parser, Subcommand};
use std::path::PathBuf;

fn config_path() -> PathBuf {
    agent_heart::config::config_path()
}

#[derive(Parser)]
#[command(name = "agent-heart", about = "Background distillation daemon")]
#[command(version)]
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
    /// Run dataset pipeline + MLX finetune once and exit
    Finetune,
    /// Show daemon status
    Status,
    /// Lint a bash script for common issues
    Lint {
        /// Path to the script to lint
        script: std::path::PathBuf,
    },
    /// View supervisor logs
    Log {
        /// Log name to view (omit to list all logs)
        name: Option<String>,
        /// Follow log output (tail -f style)
        #[arg(long)]
        follow: bool,
        /// List available logs
        #[arg(long)]
        list: bool,
    },
    /// Predictive token budget tools (reads agent-brain retrieval_log)
    Budget {
        #[command(subcommand)]
        command: BudgetCommands,
    },
}

#[derive(Subcommand)]
enum BudgetCommands {
    /// Show historical token stats from brain.db
    Stats,
    /// Check whether a route would be allowed
    Check {
        #[arg(long, default_value = "implementing")]
        phase: String,
        #[arg(long)]
        tokens: u64,
    },
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
        Commands::Finetune => agent_heart::run_finetune_once(config).await?,
        Commands::Status => {
            let state_dir = agent_heart::config::state_dir();
            let last_gc = std::fs::read_to_string(state_dir.join("last_gc.txt"))
                .unwrap_or_else(|_| "never".into());
            let last_finetune = std::fs::read_to_string(state_dir.join("last_finetune.txt"))
                .unwrap_or_else(|_| "never".into());
            println!("agent-heart status");
            println!("  config: {}", config_path().display());
            println!(
                "  schedule: {} (enabled: {})",
                config.schedule.cron, config.schedule.enabled
            );
            println!(
                "  finetune: {} (enabled: {}, min_entries: {})",
                config.finetune.cron, config.finetune.enabled, config.finetune.min_merged_entries
            );
            println!("  last_gc: {}", last_gc);
            println!("  last_finetune: {}", last_finetune);
            println!(
                "  token_budget: enabled={} ceiling={} anomaly_x={}",
                config.token_budget.enabled,
                config.token_budget.max_tokens_per_route,
                config.token_budget.anomaly_multiplier
            );
        }
        Commands::Lint { script } => {
            agent_heart::lint::lint_script(&script)?;
        }
        Commands::Log {
            name,
            follow,
            list,
        } => {
            if list {
                let logs = agent_heart::log::list_logs()?;
                for name in logs {
                    println!("{name}");
                }
            } else if let Some(name) = name {
                if follow {
                    agent_heart::log::follow_log(&name)?;
                } else {
                    agent_heart::log::print_log(&name)?;
                }
            } else {
                let logs = agent_heart::log::list_logs()?;
                for name in logs {
                    println!("{name}");
                }
            }
        }
        Commands::Budget { command } => match command {
            BudgetCommands::Stats => {
                let report = agent_heart::token_budget::load_stats(&config.token_budget)?;
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            BudgetCommands::Check { phase, tokens } => {
                let resp = agent_heart::token_budget::check_budget(
                    &config.token_budget,
                    &agent_heart::token_budget::BudgetCheckRequest {
                        phase,
                        estimated_tokens: tokens,
                        task_kind: None,
                    },
                )?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
                if !resp.allowed {
                    std::process::exit(1);
                }
            }
        },
    }
    Ok(())
}
