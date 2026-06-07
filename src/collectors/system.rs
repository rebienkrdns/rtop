use sysinfo::{Disks, System};

use crate::models::{CpuData, DiskData, MemoryData};

pub struct SystemSnapshot {
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
}

pub struct SystemCollector {
    sys: System,
    disks: Disks,
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
        Self { sys, disks }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.disks.refresh_list();
    }

    pub fn cpu_data(&self) -> CpuData {
        let per_core: Vec<f64> = self.sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect();
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

    pub fn disk_data(&self) -> Vec<DiskData> {
        self.disks
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
                DiskData {
                    device: disk.name().to_string_lossy().into_owned(),
                    mount_point: disk.mount_point().to_string_lossy().into_owned(),
                    total_bytes: total,
                    used_bytes: used,
                    usage_pct,
                }
            })
            .collect()
    }

    pub fn snapshot(&self) -> SystemSnapshot {
        SystemSnapshot {
            cpu: self.cpu_data(),
            memory: self.memory_data(),
            disks: self.disk_data(),
        }
    }
}
