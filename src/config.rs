use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub const INTERVALS: &[f64] = &[0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0];
pub const DEFAULT_INTERVAL_IDX: usize = 2; // 2s

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    #[default]
    Processes,
    Containers,
    Network,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SortColumn {
    #[default]
    Cpu,
    Memory,
    Pid,
    Name,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub refresh_interval_secs: f64,
    pub selected_disk: Option<String>,
    pub selected_nic: Option<String>,
    pub default_tab: Tab,
    pub process_sort_column: SortColumn,
    pub show_swap: bool,
    pub docker_socket_path: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            refresh_interval_secs: INTERVALS[DEFAULT_INTERVAL_IDX],
            selected_disk: None,
            selected_nic: None,
            default_tab: Tab::Processes,
            process_sort_column: SortColumn::Cpu,
            show_swap: true,
            docker_socket_path: None,
        }
    }
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("rtop")
        .join("config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    if !path.exists() {
        let cfg = Config::default();
        if let Err(e) = save(&cfg) {
            tracing::warn!("No se pudo crear config.toml: {}", e);
        }
        return cfg;
    }
    match fs::read_to_string(&path) {
        Err(e) => {
            tracing::warn!("No se pudo leer config.toml: {}", e);
            Config::default()
        }
        Ok(content) => match toml::from_str::<Config>(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                // Archivo corrupto: loggear y usar defaults sin sobreescribir
                tracing::warn!(
                    "config.toml tiene errores de parseo (usando defaults, archivo preservado): {}",
                    e
                );
                Config::default()
            }
        },
    }
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

/// Guarda la config en un hilo bloqueante para no interrumpir el render loop.
pub fn save_non_blocking(cfg: Config) {
    tokio::task::spawn_blocking(move || {
        if let Err(e) = save(&cfg) {
            tracing::warn!("Error guardando config: {}", e);
        }
    });
}

pub fn interval_label(idx: usize) -> String {
    let secs = INTERVALS[idx];
    if secs < 1.0 {
        format!("{:.1}s", secs)
    } else {
        format!("{}s", secs as u64)
    }
}
