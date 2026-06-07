use std::collections::HashMap;
use std::time::Instant;

use sysinfo::Disk;

use crate::models::DiskData;

#[allow(dead_code)]
const SECTOR_SIZE: u64 = 512;

#[allow(dead_code)]
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
#[allow(dead_code)]
pub struct DiskSelectorEntry {
    pub device_short: String,
    pub device_full: String,
    pub mount_point: String,
    pub total_bytes: u64,
}

pub struct DiskIoCollector {
    #[allow(dead_code)]
    prev: HashMap<String, DiskIoSnapshot>,
    #[cfg(target_os = "linux")]
    proc_prev: HashMap<u32, DiskIoSnapshot>,
}

impl Default for DiskIoCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskIoCollector {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            #[cfg(target_os = "linux")]
            proc_prev: HashMap::new(),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn io_rates_from_disks(&mut self, disks: &[Disk]) -> HashMap<String, DiskIoRate> {
        let mut result = HashMap::new();
        for disk in disks {
            let name = device_short_name(&disk.name().to_string_lossy());
            result.insert(name, DiskIoRate::default());
        }
        result
    }

    #[cfg(not(target_os = "macos"))]
    pub fn io_rates_from_disks(&mut self, _disks: &[Disk]) -> HashMap<String, DiskIoRate> {
        HashMap::new()
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

    #[allow(dead_code)]
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
    pub fn raw_block_size(name: &str) -> u64 {
        let path = format!("/sys/block/{}/size", name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(sectors) = content.trim().parse::<u64>() {
                return sectors * SECTOR_SIZE;
            }
        }
        0
    }

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    pub fn diskstats_names() -> Vec<String> {
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

    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    pub fn diskstats_names() -> Vec<String> {
        vec![]
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

    #[cfg(target_os = "linux")]
    pub fn process_io_rates(&mut self, pids: &[u32]) -> (HashMap<u32, crate::models::process::ProcessIoData>, bool) {
        use std::fs;
        let now = Instant::now();
        let mut result = HashMap::new();
        let mut permission_denied = false;

        for &pid in pids {
            let path = format!("/proc/{}/io", pid);
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let mut read_bytes = 0;
                    let mut write_bytes = 0;
                    for line in content.lines() {
                        if line.starts_with("read_bytes:") {
                            if let Some(val_str) = line.split_whitespace().nth(1) {
                                read_bytes = val_str.parse().unwrap_or(0);
                            }
                        } else if line.starts_with("write_bytes:") {
                            if let Some(val_str) = line.split_whitespace().nth(1) {
                                write_bytes = val_str.parse().unwrap_or(0);
                            }
                        }
                    }

                    let rate = if let Some(prev) = self.proc_prev.get(&pid) {
                        let elapsed = now.duration_since(prev.timestamp).as_secs_f64();
                        if elapsed > 0.0 {
                            let dr = read_bytes.saturating_sub(prev.sectors_read);
                            let dw = write_bytes.saturating_sub(prev.sectors_written);
                            crate::models::process::ProcessIoData {
                                read_bytes_per_sec: dr as f64 / elapsed,
                                write_bytes_per_sec: dw as f64 / elapsed,
                            }
                        } else {
                            crate::models::process::ProcessIoData::default()
                        }
                    } else {
                        crate::models::process::ProcessIoData::default()
                    };

                    self.proc_prev.insert(
                        pid,
                        DiskIoSnapshot {
                            timestamp: now,
                            sectors_read: read_bytes,
                            sectors_written: write_bytes,
                        },
                    );
                    result.insert(pid, rate);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        permission_denied = true;
                    }
                }
            }
        }

        // Clean up stale PIDs to avoid memory leaks
        self.proc_prev.retain(|pid, _| pids.contains(pid));

        (result, permission_denied)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn process_io_rates(&mut self, _pids: &[u32]) -> (HashMap<u32, crate::models::process::ProcessIoData>, bool) {
        (HashMap::new(), false)
    }
}

pub fn device_short_name(full: &str) -> String {
    full.strip_prefix("/dev/").unwrap_or(full).to_string()
}
