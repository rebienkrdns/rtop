use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use bollard::container::{InspectContainerOptions, ListContainersOptions, StatsOptions};
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
    pub fn docker_client(&self) -> Option<Docker> {
        self.docker.clone()
    }

    pub async fn new() -> Self {
        let docker = Self::connect().await;
        let state = if docker.is_some() {
            ContainerBackendState {
                available: true,
                message: None,
            }
        } else {
            ContainerBackendState {
                available: false,
                message: Some("Docker/Podman no disponible".to_string()),
            }
        };
        Self {
            docker,
            prev: HashMap::new(),
            state,
        }
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
                if let Ok(docker) =
                    Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
                {
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

        let mut stats_futures = Vec::new();
        for c in containers {
            let docker_clone = docker.clone();
            let id = c.id.clone().unwrap_or_default();
            stats_futures.push(async move {
                let stats_stream = docker_clone.stats(
                    &id,
                    Some(StatsOptions {
                        stream: false,
                        ..Default::default()
                    }),
                );
                let mut stream = stats_stream.take(1);
                let stats = match stream.next().await {
                    Some(Ok(s)) => Some(s),
                    _ => None,
                };
                let inspect = docker_clone
                    .inspect_container(&id, Some(InspectContainerOptions { size: false }))
                    .await
                    .ok();
                (c, stats, inspect)
            });
        }

        let stats_results = futures_util::future::join_all(stats_futures).await;

        let mut result = Vec::new();
        for (c, stats_opt, inspect_opt) in stats_results {
            let Some(stats) = stats_opt else {
                continue;
            };

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

            let (
                cpu_pct,
                memory_bytes,
                memory_limit_bytes,
                net_recv_per_sec,
                net_recv_total,
                net_sent_per_sec,
                net_sent_total,
                disk_read_per_sec,
                disk_read_total,
                disk_write_per_sec,
                disk_write_total,
            ) = derive_metrics(&id, &stats, self.prev.get(&id));

            self.prev.insert(
                id.clone(),
                ContainerSnapshot {
                    timestamp: Instant::now(),
                    cpu_total: stats.cpu_stats.cpu_usage.total_usage,
                    system_cpu: stats.cpu_stats.system_cpu_usage.unwrap_or(0),
                    net_recv: net_recv_total as f64,
                    net_sent: net_sent_total as f64,
                    blk_read: disk_read_total as f64,
                    blk_write: disk_write_total as f64,
                },
            );

            let compose_project = c
                .labels
                .as_ref()
                .and_then(|labels| labels.get("com.docker.compose.project").cloned());

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
                net_recv_per_sec,
                net_recv_total,
                net_sent_per_sec,
                net_sent_total,
                disk_read_per_sec,
                disk_read_total,
                disk_write_per_sec,
                disk_write_total,
                ports: extract_ports(&inspect_opt),
                volumes: extract_volumes(&inspect_opt),
                networks: extract_networks(&inspect_opt),
                env_vars: extract_env_vars(&inspect_opt),
                compose_project,
            });
        }

        result.sort_by(|a, b| a.name.cmp(&b.name));
        result
    }
}

fn extract_ports(inspect: &Option<bollard::models::ContainerInspectResponse>) -> Vec<String> {
    let Some(resp) = inspect else {
        return vec![];
    };
    let Some(host_config) = resp.host_config.as_ref() else {
        return vec![];
    };
    let Some(bindings) = host_config.port_bindings.as_ref() else {
        return vec![];
    };
    let mut ports = Vec::new();
    for (container_port, host_binds) in bindings {
        if let Some(binds) = host_binds {
            for b in binds {
                let host_ip = b.host_ip.as_deref().unwrap_or("0.0.0.0");
                let host_port = b.host_port.as_deref().unwrap_or("?");
                ports.push(format!("{}:{}->{}", host_ip, host_port, container_port));
            }
        } else {
            ports.push(container_port.clone());
        }
    }
    ports
}

fn extract_volumes(inspect: &Option<bollard::models::ContainerInspectResponse>) -> Vec<String> {
    let Some(resp) = inspect else {
        return vec![];
    };
    let Some(mounts) = resp.mounts.as_ref() else {
        return vec![];
    };
    mounts
        .iter()
        .map(|m| {
            let src = m.source.as_deref().unwrap_or("?");
            let dst = m.destination.as_deref().unwrap_or("?");
            let rw = if m.rw.unwrap_or(true) { "rw" } else { "ro" };
            format!("{}:{} ({})", src, dst, rw)
        })
        .collect()
}

fn extract_networks(inspect: &Option<bollard::models::ContainerInspectResponse>) -> Vec<String> {
    let Some(resp) = inspect else {
        return vec![];
    };
    let Some(ns) = resp.network_settings.as_ref() else {
        return vec![];
    };
    let Some(networks) = ns.networks.as_ref() else {
        return vec![];
    };
    networks
        .iter()
        .map(|(name, net)| {
            let ip = net.ip_address.as_deref().unwrap_or("—");
            format!("{} ({})", name, ip)
        })
        .collect()
}

fn extract_env_vars(inspect: &Option<bollard::models::ContainerInspectResponse>) -> Vec<String> {
    let Some(resp) = inspect else {
        return vec![];
    };
    let Some(config) = resp.config.as_ref() else {
        return vec![];
    };
    config.env.clone().unwrap_or_default()
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
) -> (f64, u64, u64, f64, u64, f64, u64, f64, u64, f64, u64) {
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

    let (net_recv_bps, net_sent_bps, read_bps, write_bps) = if let Some(prev) = prev {
        let elapsed = prev.timestamp.elapsed().as_secs_f64().max(0.001);
        (
            ((net_recv - prev.net_recv).max(0.0) / elapsed),
            ((net_sent - prev.net_sent).max(0.0) / elapsed),
            ((blk_read - prev.blk_read).max(0.0) / elapsed),
            ((blk_write - prev.blk_write).max(0.0) / elapsed),
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    (
        cpu_pct,
        memory_bytes,
        memory_limit_bytes,
        net_recv_bps,
        net_recv as u64,
        net_sent_bps,
        net_sent as u64,
        read_bps,
        blk_read as u64,
        write_bps,
        blk_write as u64,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[allow(clippy::too_many_arguments)]
    fn make_stats(
        total_usage: u64,
        system_cpu_usage: u64,
        online_cpus: u64,
        mem_usage: u64,
        mem_limit: u64,
        rx_bytes: u64,
        tx_bytes: u64,
        blk_read: u64,
        blk_write: u64,
    ) -> bollard::container::Stats {
        let json = format!(
            r#"{{
            "read": "",
            "preread": "",
            "num_procs": 0,
            "pids_stats": {{ "current": null, "limit": null }},
            "networks": {{
                "eth0": {{
                    "rx_bytes": {},
                    "rx_packets": 0,
                    "rx_errors": 0,
                    "rx_dropped": 0,
                    "tx_bytes": {},
                    "tx_packets": 0,
                    "tx_errors": 0,
                    "tx_dropped": 0
                }}
            }},
            "memory_stats": {{
                "usage": {},
                "limit": {}
            }},
            "blkio_stats": {{
                "io_service_bytes_recursive": [
                    {{ "major": 8, "minor": 0, "op": "Read", "value": {} }},
                    {{ "major": 8, "minor": 0, "op": "Write", "value": {} }}
                ]
            }},
            "cpu_stats": {{
                "cpu_usage": {{
                    "total_usage": {},
                    "usage_in_kernelmode": 0,
                    "usage_in_usermode": 0
                }},
                "system_cpu_usage": {},
                "online_cpus": {},
                "throttling_data": {{
                    "periods": 0,
                    "throttled_periods": 0,
                    "throttled_time": 0
                }}
            }},
            "precpu_stats": {{
                "cpu_usage": {{
                    "total_usage": 0,
                    "usage_in_kernelmode": 0,
                    "usage_in_usermode": 0
                }},
                "throttling_data": {{
                    "periods": 0,
                    "throttled_periods": 0,
                    "throttled_time": 0
                }}
            }},
            "storage_stats": {{}},
            "name": "",
            "id": ""
        }}"#,
            rx_bytes,
            tx_bytes,
            mem_usage,
            mem_limit,
            blk_read,
            blk_write,
            total_usage,
            system_cpu_usage,
            online_cpus
        );

        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn test_derive_metrics_first_snapshot() {
        let stats = make_stats(1000, 5000, 2, 50000, 100000, 100, 200, 300, 400);

        let res = derive_metrics("test_id", &stats, None);
        assert_eq!(res.0, 0.0); // cpu_pct is 0 without previous snapshot
        assert_eq!(res.1, 50000); // memory_bytes
        assert_eq!(res.2, 100000); // memory_limit_bytes
        assert_eq!(res.3, 0.0); // net_recv_per_sec rate is 0
        assert_eq!(res.4, 100); // net_recv_total
        assert_eq!(res.5, 0.0); // net_sent_per_sec rate is 0
        assert_eq!(res.6, 200); // net_sent_total
        assert_eq!(res.7, 0.0); // disk_read_per_sec rate is 0
        assert_eq!(res.8, 300); // disk_read_total
        assert_eq!(res.9, 0.0); // disk_write_per_sec rate is 0
        assert_eq!(res.10, 400); // disk_write_total
    }

    #[test]
    fn test_derive_metrics_second_snapshot() {
        // Initial setup
        let prev = ContainerSnapshot {
            timestamp: Instant::now() - Duration::from_secs(2),
            cpu_total: 1000,
            system_cpu: 5000,
            net_recv: 100.0,
            net_sent: 200.0,
            blk_read: 300.0,
            blk_write: 400.0,
        };

        let stats = make_stats(1200, 6000, 2, 50000, 100000, 300, 600, 500, 1200);

        let res = derive_metrics("test_id", &stats, Some(&prev));
        // cpu_pct: (200 / 1000) * 2 * 100 = 40%
        assert!((res.0 - 40.0).abs() < 1.0);
        assert_eq!(res.1, 50000); // memory_bytes
        assert_eq!(res.2, 100000); // memory_limit_bytes
                                   // rates should be approx (delta / 2.0s) = delta / 2.0
        assert!((res.3 - 100.0).abs() < 5.0); // net_recv_per_sec
        assert_eq!(res.4, 300); // net_recv_total
        assert!((res.5 - 200.0).abs() < 5.0); // net_sent_per_sec
        assert_eq!(res.6, 600); // net_sent_total
        assert!((res.7 - 100.0).abs() < 5.0); // disk_read_per_sec
        assert_eq!(res.8, 500); // disk_read_total
        assert!((res.9 - 400.0).abs() < 5.0); // disk_write_per_sec
        assert_eq!(res.10, 1200); // disk_write_total
    }
}
