use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const INTERVALS: &[f64] = &[0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0];
pub const DEFAULT_INTERVAL_IDX: usize = 2; // 2s

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub refresh_interval_secs: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_secs: INTERVALS[DEFAULT_INTERVAL_IDX],
        }
    }
}

fn config_path() -> PathBuf {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"));
    config_home.join("rtop").join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(cfg) = toml::from_str::<Config>(&content) {
            return cfg;
        }
    }
    Config::default()
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(cfg)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn interval_label(idx: usize) -> String {
    let secs = INTERVALS[idx];
    if secs < 1.0 {
        format!("{:.1}s", secs)
    } else {
        format!("{}s", secs as u64)
    }
}
