use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub schedule: ScheduleConfig,
    pub finetune: FinetuneConfig,
    pub brain: BrainConfig,
    pub token_budget: TokenBudgetConfig,
    pub server: ServerConfig,
    pub spine: SpineConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub cron: String,
    pub enabled: bool,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            cron: "0 0 3 * * *".into(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinetuneConfig {
    pub enabled: bool,
    pub cron: String,
    pub dataset_dir: Option<String>,
    pub muscle_binary: Option<String>,
    pub verify_ui_url: Option<String>,
    pub verify_memory_script: Option<String>,
    pub ui_threshold: f64,
    pub memory_threshold_kb: u64,
    pub min_merged_entries: u64,
}

impl Default for FinetuneConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cron: "0 0 4 * * *".into(),
            dataset_dir: None,
            muscle_binary: None,
            verify_ui_url: None,
            verify_memory_script: None,
            ui_threshold: 1.0,
            memory_threshold_kb: 524_288,
            min_merged_entries: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrainConfig {
    pub binary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetConfig {
    pub enabled: bool,
    pub max_tokens_per_route: u64,
    pub anomaly_multiplier: f64,
    pub lookback_days: u32,
    pub min_samples: u64,
    pub brain_db_path: Option<String>,
}

impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tokens_per_route: 8000,
            anomaly_multiplier: 2.5,
            lookback_days: 7,
            min_samples: 5,
            brain_db_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { port: 3101 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpineConfig {
    pub url: String,
}

impl Default for SpineConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:3100".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            file: None,
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        agent_body_core::organ_config::load("heart")
    }
}

pub fn config_path() -> PathBuf {
    agent_body_core::config_path()
}

pub fn state_dir() -> PathBuf {
    agent_body_core::organ_state_dir("heart")
}
