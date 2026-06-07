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
        self.networks.refresh_list();
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
        self.networks
            .iter()
            .map(|(name, data)| {
                let recv_bps = data.received() as f64 / elapsed;
                let sent_bps = data.transmitted() as f64 / elapsed;
                (
                    name.clone(),
                    NetworkData {
                        interface: name.clone(),
                        recv_bytes_per_sec: recv_bps,
                        sent_bytes_per_sec: sent_bps,
                        total_recv_bytes: data.total_received(),
                        total_sent_bytes: data.total_transmitted(),
                    },
                )
            })
            .collect()
    }
}

fn is_docker(name: &str) -> bool {
    name.starts_with("docker") || name.starts_with("br-") || name.starts_with("veth")
}
