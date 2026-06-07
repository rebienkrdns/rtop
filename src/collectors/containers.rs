use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use bollard::container::{ListContainersOptions, StatsOptions};
use bollard::Docker;
use futures_util::StreamExt;

use crate::models::{ContainerData, ContainerStatus};

#[derive(Clone)]
#[allow(dead_code)]
struct ContainerSnapshot {
    timestamp: Instant,
    cpu_total: u64,
    system_cpu: u64,
    net_recv: f64,
    net_sent: f64,
    blk_read: f64,
    blk_write: f64,
}

#[derive(Clone, Debug, Default)]
pub struct ContainerBackendState {
    pub available: bool,
    pub message: Option<String>,
}

pub struct ContainerCollector {
    docker: Option<Docker>,
    prev: HashMap<String, ContainerSnapshot>,
    pub state: ContainerBackendState,
}

impl ContainerCollector {
    pub async fn new() -> Self {
        let docker = Self::connect().await;
        let state = if docker.is_some() {
            ContainerBackendState { available: true, message: None }
        } else {
            ContainerBackendState {
                available: false,
                message: Some("Docker/Podman no disponible".to_string()),
            }
        };
        Self { docker, prev: HashMap::new(), state }
    }

    async fn connect() -> Option<Docker> {
        #[cfg(unix)]
        {
            let candidates = [
                "/var/run/docker.sock",
                "/run/podman/podman.sock",
                "/run/user/1000/podman/podman.sock",
            ];
            for path in candidates {
                if !Path::new(path).exists() {
                    continue;
                }
                if let Ok(docker) = Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION) {
                    if docker.version().await.is_ok() {
                        return Some(docker);
                    }
                }
            }
        }
        None
    }

    pub async fn refresh(&mut self) -> Vec<ContainerData> {
        let Some(docker) = &self.docker else {
            return vec![];
        };

        let containers = match docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
        {
            Ok(items) => items,
            Err(e) => {
                self.state.available = false;
                self.state.message = Some(format!("Error leyendo contenedores: {e}"));
                return vec![];
            }
        };

        let mut result = Vec::new();
        for c in containers {
            let id = c.id.clone().unwrap_or_default();
            let full_id = id.clone();
            let name = c
                .names
                .clone()
                .and_then(|mut names| names.drain(..).next())
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string();
            let image = c.image.clone().unwrap_or_default();
            let status = map_status(
                c.state.as_deref().unwrap_or_default(),
                c.status.as_deref().unwrap_or_default(),
            );

            let stats_stream = docker.stats(&id, Some(StatsOptions { stream: false, ..Default::default() }));
            let mut stream = stats_stream.take(1);
            let stats = match stream.next().await {
                Some(Ok(s)) => s,
                _ => continue,
            };

            let (cpu_pct, memory_bytes, memory_limit_bytes, net_recv, net_sent, disk_read, disk_write) =
                derive_metrics(&id, &stats, self.prev.get(&id));

            self.prev.insert(
                id.clone(),
                ContainerSnapshot {
                    timestamp: Instant::now(),
                    cpu_total: stats.cpu_stats.cpu_usage.total_usage,
                    system_cpu: stats.cpu_stats.system_cpu_usage.unwrap_or(0),
                    net_recv,
                    net_sent,
                    blk_read: disk_read,
                    blk_write: disk_write,
                },
            );

            result.push(ContainerData {
                id: full_id.chars().take(12).collect(),
                name,
                image,
                status,
                uptime_secs: None,
                cpu_pct,
                memory_bytes,
                memory_limit_bytes,
                memory_pct: if memory_limit_bytes > 0 {
                    (memory_bytes as f64 / memory_limit_bytes as f64) * 100.0
                } else {
                    0.0
                },
                net_recv_per_sec: net_recv,
                net_sent_per_sec: net_sent,
                disk_read_per_sec: disk_read,
                disk_write_per_sec: disk_write,
                ports: vec![],
                volumes: vec![],
            });
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}

fn map_status(state: &str, status: &str) -> ContainerStatus {
    let value = format!("{} {}", state.to_lowercase(), status.to_lowercase());
    if value.contains("running") {
        ContainerStatus::Running
    } else if value.contains("paused") {
        ContainerStatus::Paused
    } else if value.contains("restarting") {
        ContainerStatus::Restarting
    } else if value.contains("exited") {
        ContainerStatus::Exited
    } else if value.contains("dead") {
        ContainerStatus::Dead
    } else {
        ContainerStatus::Unknown
    }
}

fn derive_metrics(
    _id: &str,
    stats: &bollard::container::Stats,
    prev: Option<&ContainerSnapshot>,
) -> (f64, u64, u64, f64, f64, f64, f64) {
    let cpu_total = stats.cpu_stats.cpu_usage.total_usage;
    let system_cpu = stats.cpu_stats.system_cpu_usage.unwrap_or(0);
    let online_cpus = stats.cpu_stats.online_cpus.unwrap_or(1) as f64;

    let cpu_pct = if let Some(prev) = prev {
        let cpu_delta = cpu_total.saturating_sub(prev.cpu_total) as f64;
        let system_delta = system_cpu.saturating_sub(prev.system_cpu) as f64;
        if cpu_delta > 0.0 && system_delta > 0.0 {
            (cpu_delta / system_delta) * online_cpus * 100.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    let memory_bytes = stats.memory_stats.usage.unwrap_or(0);
    let memory_limit_bytes = stats.memory_stats.limit.unwrap_or(0);

    let (mut net_recv, mut net_sent) = (0.0, 0.0);
    if let Some(networks) = &stats.networks {
        for data in networks.values() {
            net_recv += data.rx_bytes as f64;
            net_sent += data.tx_bytes as f64;
        }
    }

    let blk_read = stats
        .blkio_stats
        .io_service_bytes_recursive
        .as_ref()
        .map(sum_blkio("Read"))
        .unwrap_or(0) as f64;
    let blk_write = stats
        .blkio_stats
        .io_service_bytes_recursive
        .as_ref()
        .map(sum_blkio("Write"))
        .unwrap_or(0) as f64;

    let (read_bps, write_bps) = if let Some(prev) = prev {
        let elapsed = prev.timestamp.elapsed().as_secs_f64().max(0.001);
        ((blk_read - prev.blk_read).max(0.0) / elapsed, (blk_write - prev.blk_write).max(0.0) / elapsed)
    } else {
        (0.0, 0.0)
    };

    (
        cpu_pct,
        memory_bytes,
        memory_limit_bytes,
        net_recv,
        net_sent,
        read_bps,
        write_bps,
    )
}

fn sum_blkio(op: &'static str) -> impl FnOnce(&Vec<bollard::container::BlkioStatsEntry>) -> u64 {
    move |entries| {
        entries
            .iter()
            .filter(|e| e.op.eq_ignore_ascii_case(op))
            .map(|e| e.value)
            .sum()
    }
}
