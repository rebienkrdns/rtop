use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ui::theme::ThemeMode;

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
    #[serde(default)]
    pub theme: ThemeMode,
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
            theme: ThemeMode::Dark,
        }
    }
}

fn config_path() -> PathBuf {
    if let Ok(val) = std::env::var("RTOP_CONFIG_PATH") {
        return PathBuf::from(val);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_config_lifecycle() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        let temp_file_path =
            std::env::temp_dir().join(format!("rtop_config_test_{}.toml", now.as_nanos()));

        // Set path to temp file
        std::env::set_var("RTOP_CONFIG_PATH", &temp_file_path);

        // Ensure temp file is cleaned up if it existed
        if temp_file_path.exists() {
            fs::remove_file(&temp_file_path).unwrap();
        }

        // 1. First load: file does not exist.
        // It should return default config and automatically create the config file.
        let cfg = load();
        assert_eq!(cfg.refresh_interval_secs, 2.0);
        assert!(cfg.selected_disk.is_none());
        assert!(cfg.selected_nic.is_none());
        assert_eq!(cfg.default_tab, Tab::Processes);
        assert_eq!(cfg.process_sort_column, SortColumn::Cpu);
        assert!(cfg.show_swap);
        assert!(cfg.docker_socket_path.is_none());
        assert_eq!(cfg.theme, ThemeMode::Dark);

        assert!(
            temp_file_path.exists(),
            "Config file should have been created automatically"
        );

        // 2. Save modified configuration and load again
        let mut modified = cfg;
        modified.refresh_interval_secs = 5.0;
        modified.selected_disk = Some("sda1".to_string());
        modified.selected_nic = Some("eth0".to_string());
        modified.default_tab = Tab::Network;
        modified.process_sort_column = SortColumn::Memory;
        modified.show_swap = false;
        modified.docker_socket_path = Some("/var/run/docker.sock".to_string());
        modified.theme = ThemeMode::Light;

        save(&modified).expect("Failed to save config");

        let loaded = load();
        assert_eq!(loaded.refresh_interval_secs, 5.0);
        assert_eq!(loaded.selected_disk.as_deref(), Some("sda1"));
        assert_eq!(loaded.selected_nic.as_deref(), Some("eth0"));
        assert_eq!(loaded.default_tab, Tab::Network);
        assert_eq!(loaded.process_sort_column, SortColumn::Memory);
        assert!(!loaded.show_swap);
        assert_eq!(
            loaded.docker_socket_path.as_deref(),
            Some("/var/run/docker.sock")
        );
        assert_eq!(loaded.theme, ThemeMode::Light);

        // 3. Corrupt configuration file
        let corrupt_content = "this is invalid toml = [ {";
        fs::write(&temp_file_path, corrupt_content).expect("Failed to write corrupt config");

        // Load corrupt config
        let fallback_cfg = load();
        // Should use defaults
        assert_eq!(fallback_cfg.refresh_interval_secs, 2.0);

        // File should not have been overwritten or deleted
        let current_content =
            fs::read_to_string(&temp_file_path).expect("Failed to read config file");
        assert_eq!(
            current_content, corrupt_content,
            "Corrupt file should be preserved"
        );

        // Clean up
        fs::remove_file(&temp_file_path).ok();
        std::env::remove_var("RTOP_CONFIG_PATH");
    }
}
