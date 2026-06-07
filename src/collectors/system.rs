use std::collections::HashMap;

use sysinfo::{Disks, System};

use crate::collectors::disk::{device_short_name, DiskIoCollector};
use crate::collectors::network::NetworkCollector;
use crate::models::{CpuData, DiskData, MemoryData, NetworkData, NetworkInterface};

pub struct SystemSnapshot {
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    pub network_by_nic: HashMap<String, NetworkData>,
    pub available_nics: Vec<NetworkInterface>,
    pub suggested_nic: Option<String>,
    pub proc_permission_denied: bool,
}

pub struct SystemCollector {
    sys: System,
    disks: Disks,
    disk_io: DiskIoCollector,
    network: NetworkCollector,
}

impl Default for SystemCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemCollector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        Self { sys, disks, disk_io: DiskIoCollector::new(), network: NetworkCollector::new() }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.disks.refresh_list();
        self.network.refresh();
    }

    pub fn cpu_data(&self) -> CpuData {
        let per_core: Vec<f64> =
            self.sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect();
        let core_count = per_core.len();
        CpuData {
            global_usage_pct: self.sys.global_cpu_info().cpu_usage() as f64,
            per_core,
            core_count,
        }
    }

    pub fn memory_data(&self) -> MemoryData {
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();
        let available = self.sys.available_memory();
        let usage_pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        MemoryData {
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            usage_pct,
        }
    }

    pub fn disk_data(&mut self) -> Vec<DiskData> {
        let mut disk_shorts: Vec<String> = self
            .disks
            .list()
            .iter()
            .map(|d| device_short_name(&d.name().to_string_lossy()))
            .collect();

        // Also query diskstats_names to support unmounted disks
        for name in DiskIoCollector::diskstats_names() {
            if !disk_shorts.contains(&name) {
                disk_shorts.push(name);
            }
        }

        let rates = self.disk_io.io_rates_batch(&disk_shorts);

        #[allow(unused_mut)]
        let mut result: Vec<DiskData> = self.disks
            .list()
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                let usage_pct = if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let short = device_short_name(&disk.name().to_string_lossy());
                let rate = rates.get(&short).cloned().unwrap_or_default();
                DiskData {
                    device: disk.name().to_string_lossy().into_owned(),
                    mount_point: disk.mount_point().to_string_lossy().into_owned(),
                    total_bytes: total,
                    used_bytes: used,
                    usage_pct,
                    read_bytes_per_sec: rate.read_bytes_per_sec,
                    write_bytes_per_sec: rate.write_bytes_per_sec,
                }
            })
            .collect();

        // Add dummy DiskData for unmounted disk stats
        #[cfg(target_os = "linux")]
        {
            use std::collections::HashSet;
            let mounted_shorts: HashSet<String> = self.disks
                .list()
                .iter()
                .map(|d| device_short_name(&d.name().to_string_lossy()))
                .collect();

            for name in DiskIoCollector::diskstats_names() {
                if !mounted_shorts.contains(&name) {
                    let total = DiskIoCollector::raw_block_size(&name);
                    let rate = rates.get(&name).cloned().unwrap_or_default();
                    result.push(DiskData {
                        device: format!("/dev/{}", name),
                        mount_point: String::new(),
                        total_bytes: total,
                        used_bytes: 0,
                        usage_pct: 0.0,
                        read_bytes_per_sec: rate.read_bytes_per_sec,
                        write_bytes_per_sec: rate.write_bytes_per_sec,
                    });
                }
            }
        }

        result
    }

    pub fn snapshot(&mut self) -> SystemSnapshot {
        let pids: Vec<u32> = self.sys.processes().keys().map(|pid| pid.as_u32()).collect();
        let (_rates, permission_denied) = self.disk_io.process_io_rates(&pids);

        SystemSnapshot {
            cpu: self.cpu_data(),
            memory: self.memory_data(),
            disks: self.disk_data(),
            network_by_nic: self.network.all_data(),
            available_nics: self.network.interfaces(),
            suggested_nic: self.network.autodetect(),
            proc_permission_denied: permission_denied,
        }
    }
}
