use std::collections::HashMap;
use std::time::Instant;

use crate::models::DiskData;

const SECTOR_SIZE: u64 = 512;

struct DiskIoSnapshot {
    timestamp: Instant,
    sectors_read: u64,
    sectors_written: u64,
}

#[derive(Default, Clone)]
pub struct DiskIoRate {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Clone, Debug)]
pub struct DiskSelectorEntry {
    pub device_short: String,
    pub device_full: String,
    pub mount_point: String,
    pub total_bytes: u64,
}

pub struct DiskIoCollector {
    prev: HashMap<String, DiskIoSnapshot>,
}

impl Default for DiskIoCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskIoCollector {
    pub fn new() -> Self {
        Self { prev: HashMap::new() }
    }

    #[cfg(target_os = "linux")]
    fn read_diskstats_raw() -> HashMap<String, (u64, u64)> {
        let content = match std::fs::read_to_string("/proc/diskstats") {
            Ok(c) => c,
            Err(_) => return HashMap::new(),
        };
        let mut map = HashMap::new();
        for line in content.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 10 {
                continue;
            }
            let sr: u64 = f[5].parse().unwrap_or(0);
            let sw: u64 = f[9].parse().unwrap_or(0);
            map.insert(f[2].to_string(), (sr, sw));
        }
        map
    }

    #[cfg(not(target_os = "linux"))]
    fn read_diskstats_raw() -> HashMap<String, (u64, u64)> {
        HashMap::new()
    }

    pub fn io_rates_batch(&mut self, device_shorts: &[String]) -> HashMap<String, DiskIoRate> {
        let stats = Self::read_diskstats_raw();
        let now = Instant::now();
        let mut result = HashMap::new();

        for name in device_shorts {
            if let Some(&(sr, sw)) = stats.get(name.as_str()) {
                let rate = if let Some(prev) = self.prev.get(name) {
                    let elapsed = now.duration_since(prev.timestamp).as_secs_f64();
                    if elapsed > 0.0 {
                        let dr = sr.saturating_sub(prev.sectors_read);
                        let dw = sw.saturating_sub(prev.sectors_written);
                        DiskIoRate {
                            read_bytes_per_sec: (dr * SECTOR_SIZE) as f64 / elapsed,
                            write_bytes_per_sec: (dw * SECTOR_SIZE) as f64 / elapsed,
                        }
                    } else {
                        DiskIoRate::default()
                    }
                } else {
                    DiskIoRate::default()
                };
                self.prev.insert(
                    name.clone(),
                    DiskIoSnapshot { timestamp: now, sectors_read: sr, sectors_written: sw },
                );
                result.insert(name.clone(), rate);
            }
        }

        result
    }

    #[cfg(target_os = "linux")]
    fn raw_block_size(name: &str) -> u64 {
        let path = format!("/sys/block/{}/size", name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(sectors) = content.trim().parse::<u64>() {
                return sectors * SECTOR_SIZE;
            }
        }
        0
    }

    #[cfg(target_os = "linux")]
    fn diskstats_names() -> Vec<String> {
        let content = match std::fs::read_to_string("/proc/diskstats") {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut names: Vec<String> = content
            .lines()
            .filter_map(|line| {
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() < 3 {
                    return None;
                }
                let n = f[2];
                if n.starts_with("loop") || n.starts_with("ram") {
                    return None;
                }
                Some(n.to_string())
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    pub fn build_selector_entries(known: &[DiskData]) -> Vec<DiskSelectorEntry> {
        let mut entries: Vec<DiskSelectorEntry> = known
            .iter()
            .map(|d| DiskSelectorEntry {
                device_short: device_short_name(&d.device),
                device_full: d.device.clone(),
                mount_point: d.mount_point.clone(),
                total_bytes: d.total_bytes,
            })
            .collect();

        #[cfg(target_os = "linux")]
        {
            use std::collections::HashSet;
            let known_short: HashSet<String> =
                entries.iter().map(|e| e.device_short.clone()).collect();
            for name in Self::diskstats_names() {
                if !known_short.contains(&name) {
                    entries.push(DiskSelectorEntry {
                        device_full: format!("/dev/{}", name),
                        total_bytes: Self::raw_block_size(&name),
                        mount_point: String::new(),
                        device_short: name,
                    });
                }
            }
        }

        entries.sort_by(|a, b| a.device_short.cmp(&b.device_short));
        entries
    }
}

pub fn device_short_name(full: &str) -> String {
    full.strip_prefix("/dev/").unwrap_or(full).to_string()
}
