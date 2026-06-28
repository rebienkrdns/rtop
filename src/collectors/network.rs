use std::collections::HashMap;
use std::time::Instant;

use sysinfo::Networks;

use crate::models::network::{NetworkData, NetworkInterface};

pub struct NetworkCollector {
    networks: Networks,
    last_elapsed: f64,
    last_refresh: Instant,
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkCollector {
    pub fn new() -> Self {
        let networks = Networks::new_with_refreshed_list();
        Self {
            networks,
            last_elapsed: 1.0,
            last_refresh: Instant::now(),
        }
    }

    pub fn refresh(&mut self) {
        self.last_elapsed = self.last_refresh.elapsed().as_secs_f64().max(0.1);
        self.networks.refresh();
        self.last_refresh = Instant::now();
    }

    pub fn interfaces(&self) -> Vec<NetworkInterface> {
        let mut nics: Vec<NetworkInterface> = self
            .networks
            .keys()
            .map(|name| {
                let is_loopback = name == "lo" || name.starts_with("lo0");
                let is_docker = is_docker(name);
                NetworkInterface {
                    name: name.clone(),
                    // Mark non-loopback, non-docker interfaces as "up" by default;
                    // sysinfo 0.30 does not expose per-interface IP addresses.
                    is_up: !is_loopback && !is_docker,
                    is_loopback,
                    ip_address: None,
                }
            })
            .collect();
        nics.sort_by(|a, b| a.name.cmp(&b.name));
        nics
    }

    pub fn autodetect(&self) -> Option<String> {
        let mut candidates: Vec<NetworkInterface> = self
            .interfaces()
            .into_iter()
            .filter(|n| !n.is_loopback)
            .filter(|n| !is_docker(&n.name))
            .filter(|n| n.is_up)
            .collect();
        candidates.sort_by(|a, b| a.name.cmp(&b.name));
        candidates.into_iter().next().map(|n| n.name)
    }

    pub fn all_data(&self) -> HashMap<String, NetworkData> {
        let elapsed = self.last_elapsed;
        let error_stats = read_all_netdev_errors();
        self.networks
            .iter()
            .map(|(name, data)| {
                let recv_bps = data.received() as f64 / elapsed;
                let sent_bps = data.transmitted() as f64 / elapsed;
                let (rx_errors, tx_errors, rx_drops, tx_drops) =
                    error_stats.get(name.as_str()).copied().unwrap_or((0, 0, 0, 0));
                (
                    name.clone(),
                    NetworkData {
                        interface: name.clone(),
                        recv_bytes_per_sec: recv_bps,
                        sent_bytes_per_sec: sent_bps,
                        total_recv_bytes: data.total_received(),
                        total_sent_bytes: data.total_transmitted(),
                        rx_errors,
                        tx_errors,
                        rx_drops,
                        tx_drops,
                    },
                )
            })
            .collect()
    }
}

fn is_docker(name: &str) -> bool {
    name.starts_with("docker") || name.starts_with("br-") || name.starts_with("veth")
}

/// Reads rx/tx errors and drops from /proc/net/dev (Linux only).
/// Returns (rx_errors, tx_errors, rx_drops, tx_drops) per interface.
fn read_all_netdev_errors() -> std::collections::HashMap<String, (u64, u64, u64, u64)> {
    #[cfg(target_os = "linux")]
    {
        let content = match std::fs::read_to_string("/proc/net/dev") {
            Ok(c) => c,
            Err(_) => return std::collections::HashMap::new(),
        };
        let mut map = std::collections::HashMap::new();
        for line in content.lines().skip(2) {
            let (iface, rest) = match line.trim().split_once(':') {
                Some(p) => p,
                None => continue,
            };
            let fields: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            // /proc/net/dev columns after iface: rx_bytes rx_packets rx_errs rx_drop rx_fifo
            //   rx_frame rx_compressed rx_multicast tx_bytes tx_packets tx_errs tx_drop ...
            if fields.len() >= 12 {
                map.insert(iface.trim().to_string(), (fields[2], fields[10], fields[3], fields[11]));
            }
        }
        return map;
    }
    #[cfg(not(target_os = "linux"))]
    std::collections::HashMap::new()
}

/// Reads the link speed (in Mbps) from `/sys/class/net/<iface>/speed` and converts to bytes/s.
/// Falls back to 1 Gbps (125_000_000 B/s) when the file is absent or contains an invalid value.
pub fn read_interface_max_bps(_name: &str) -> f64 {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/sys/class/net/{}/speed", _name);
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(mbps) = contents.trim().parse::<u64>() {
                if mbps > 0 {
                    return mbps as f64 * 125_000.0; // Mbps → bytes/s
                }
            }
        }
    }
    // Fallback: 1 Gbps
    125_000_000.0
}
