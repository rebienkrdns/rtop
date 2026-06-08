use std::collections::HashMap;

use sysinfo::{Disks, System};

use crate::collectors::disk::{device_short_name, DiskIoCollector};
use crate::collectors::network::NetworkCollector;
use crate::collectors::psi::PsiCollector;
use crate::models::{
    CpuData, DiskData, MemoryData, NetworkData, NetworkInterface, ProcessData, ProcessStatus,
    PsiData,
};

pub struct SystemSnapshot {
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    pub network_by_nic: HashMap<String, NetworkData>,
    pub available_nics: Vec<NetworkInterface>,
    pub suggested_nic: Option<String>,
    pub proc_permission_denied: bool,
    pub processes: Vec<ProcessData>,
    pub psi: Option<PsiData>,
}

pub struct SystemCollector {
    sys: System,
    disks: Disks,
    disk_io: DiskIoCollector,
    network: NetworkCollector,
    psi: PsiCollector,
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
        Self {
            sys,
            disks,
            disk_io: DiskIoCollector::new(),
            network: NetworkCollector::new(),
            psi: PsiCollector::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.disks.refresh();
        self.network.refresh();
    }

    pub fn cpu_data(&self) -> CpuData {
        let per_core: Vec<f64> = self
            .sys
            .cpus()
            .iter()
            .map(|c| c.cpu_usage() as f64)
            .collect();
        let core_count = per_core.len();
        let la = sysinfo::System::load_average();
        CpuData {
            global_usage_pct: self.sys.global_cpu_info().cpu_usage() as f64,
            per_core,
            core_count,
            load_avg: [la.one, la.five, la.fifteen],
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
        #[cfg(not(target_os = "linux"))]
        let rates = self.disk_io.io_rates_from_disks(self.disks.list());

        #[cfg(target_os = "linux")]
        let rates = {
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
            rates
        };

        #[allow(unused_mut)]
        let mut result: Vec<DiskData> = self
            .disks
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
                    read_bytes_per_sec: Some(rate.read_bytes_per_sec),
                    write_bytes_per_sec: Some(rate.write_bytes_per_sec),
                }
            })
            .collect();

        // Add dummy DiskData for unmounted disk stats
        #[cfg(target_os = "linux")]
        {
            use std::collections::HashSet;
            let mounted_shorts: HashSet<String> = self
                .disks
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
                        read_bytes_per_sec: Some(rate.read_bytes_per_sec),
                        write_bytes_per_sec: Some(rate.write_bytes_per_sec),
                    });
                }
            }
        }

        result
    }

    pub fn snapshot(&mut self) -> SystemSnapshot {
        let pids: Vec<u32> = self
            .sys
            .processes()
            .keys()
            .map(|pid| pid.as_u32())
            .collect();
        let (rates, permission_denied) = self.disk_io.process_io_rates(&pids);
        let users = sysinfo::Users::new_with_refreshed_list();
        let total_mem = self.sys.total_memory();

        let processes: Vec<ProcessData> = self
            .sys
            .processes()
            .iter()
            .map(|(pid_val, proc_val)| {
                let pid = pid_val.as_u32();
                let name = proc_val.name().to_string();
                let user = proc_val
                    .user_id()
                    .and_then(|uid| users.get_user_by_id(uid))
                    .map(|u| u.name().to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                let cpu_pct = proc_val.cpu_usage() as f64;
                let memory_bytes = proc_val.memory();
                let memory_pct = if total_mem > 0 {
                    (memory_bytes as f64 / total_mem as f64) * 100.0
                } else {
                    0.0
                };

                let io_rate = rates.get(&pid);
                let disk_read_per_sec = io_rate.map(|r| r.read_bytes_per_sec);
                let disk_write_per_sec = io_rate.map(|r| r.write_bytes_per_sec);

                let status = match proc_val.status() {
                    sysinfo::ProcessStatus::Run => ProcessStatus::Running,
                    sysinfo::ProcessStatus::Sleep => ProcessStatus::Sleeping,
                    sysinfo::ProcessStatus::Stop => ProcessStatus::Stopped,
                    sysinfo::ProcessStatus::Zombie => ProcessStatus::Zombie,
                    _ => ProcessStatus::Other,
                };

                let uptime_secs = proc_val.run_time();
                let threads = proc_val.tasks().map(|t| t.len() as u32).unwrap_or(1);

                let exe_path = proc_val
                    .exe()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "—".to_string());
                let cmd = proc_val.cmd().join(" ");
                let cwd = proc_val
                    .cwd()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "—".to_string());

                ProcessData {
                    pid,
                    name,
                    user,
                    cpu_pct,
                    memory_bytes,
                    memory_pct,
                    disk_read_per_sec,
                    disk_write_per_sec,
                    status,
                    uptime_secs,
                    threads,
                    exe_path,
                    cmd,
                    cwd,
                }
            })
            .collect();
        let cpu = self.cpu_data();
        let memory = self.memory_data();
        let disks = self.disk_data();
        let network_by_nic = self.network.all_data();
        let available_nics = self.network.interfaces();
        let suggested_nic = self.network.autodetect();
        let psi = self.psi.collect();

        SystemSnapshot {
            cpu,
            memory,
            disks,
            network_by_nic,
            available_nics,
            suggested_nic,
            proc_permission_denied: permission_denied,
            processes,
            psi,
        }
    }
}
