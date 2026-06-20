use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub schedule: ScheduleConfig,
    pub brain: BrainConfig,
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
            cron: "0 3 * * *".into(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainConfig {
    pub binary_path: Option<String>,
}

impl Default for BrainConfig {
    fn default() -> Self {
        Self { binary_path: None }
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
        let path = config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_yaml::from_str(&content)?)
        } else {
            let cfg = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, serde_yaml::to_string(&cfg)?)?;
            Ok(cfg)
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schedule: ScheduleConfig::default(),
            brain: BrainConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(&home).join(".config/agent-heart/config.yaml")
}
