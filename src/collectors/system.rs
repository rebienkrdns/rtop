use std::collections::HashMap;

use sysinfo::{Disks, System};

use crate::collectors::cpu_times::CpuTimesCollector;
use crate::collectors::disk::{device_short_name, DiskIoCollector};
use crate::collectors::gpu::GpuCollector;
use crate::collectors::network::NetworkCollector;
use crate::collectors::process_net::ProcessNetCollector;
use crate::collectors::psi::PsiCollector;
use crate::collectors::tcp_stats::TcpStatsCollector;
use crate::models::{
    CoreType, CpuCoreData, CpuData, DiskData, GpuData, MemoryData, NetworkData,
    NetworkInterface, ProcessData, ProcessStatus, PsiData, TcpStats,
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
    pub gpus: Vec<GpuData>,
    pub tcp_stats: Option<TcpStats>,
    pub uptime_secs: u64,
}

pub struct SystemCollector {
    sys: System,
    disks: Disks,
    disk_io: DiskIoCollector,
    network: NetworkCollector,
    process_net: ProcessNetCollector,
    psi: PsiCollector,
    gpu: GpuCollector,
    tcp: TcpStatsCollector,
    cpu_times: CpuTimesCollector,
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
            process_net: ProcessNetCollector::new(),
            psi: PsiCollector::new(),
            gpu: GpuCollector::new(),
            tcp: TcpStatsCollector::new(),
            cpu_times: CpuTimesCollector::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.sys.refresh_all();
        self.disks.refresh();
        self.network.refresh();
    }

    pub fn cpu_data(&mut self) -> CpuData {
        let cpus = self.sys.cpus();
        let core_count = cpus.len();

        let per_core: Vec<CpuCoreData> = cpus
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let usage = c.cpu_usage() as f64;
                let freq = c.frequency();
                let vendor = c.vendor_id().to_string();
                let brand = c.brand().to_string();

                let core_type = detect_core_type(i, core_count, &vendor, &brand);
                let temp = read_core_temperature(i);

                CpuCoreData {
                    core_id: i,
                    usage_pct: usage,
                    frequency_mhz: freq,
                    temperature_celsius: temp,
                    core_type,
                    vendor_id: vendor,
                    brand,
                }
            })
            .collect();

        let global_usage = self.sys.global_cpu_info().cpu_usage() as f64;
        let times = self.cpu_times.collect();

        CpuData {
            global_usage_pct: global_usage,
            per_core,
            core_count,
            user_pct: times.user_pct,
            system_pct: times.system_pct,
            iowait_pct: times.iowait_pct,
            steal_pct: times.steal_pct,
            ctx_switches_per_sec: times.ctx_switches_per_sec,
            interrupts_per_sec: times.interrupts_per_sec,
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

            self.disk_io.io_rates_batch(&disk_shorts)
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
                    read_latency_ms: rate.read_latency_ms,
                    write_latency_ms: rate.write_latency_ms,
                    io_util_pct: rate.io_util_pct,
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

                    // Aggregate used/total from mounted child partitions (e.g. nvme0n1p1 → nvme0n1)
                    let (used_bytes, usage_pct) = {
                        let mut agg_used: u64 = 0;
                        let mut agg_total: u64 = 0;
                        for d in self.disks.list() {
                            let short = device_short_name(&d.name().to_string_lossy());
                            if short != name && short.starts_with(&name) {
                                agg_total += d.total_space();
                                agg_used += d.total_space().saturating_sub(d.available_space());
                            }
                        }
                        if agg_total > 0 {
                            (agg_used, (agg_used as f64 / agg_total as f64) * 100.0)
                        } else {
                            (0, 0.0)
                        }
                    };

                    result.push(DiskData {
                        device: format!("/dev/{}", name),
                        mount_point: String::new(),
                        total_bytes: total,
                        used_bytes,
                        usage_pct,
                        read_bytes_per_sec: Some(rate.read_bytes_per_sec),
                        write_bytes_per_sec: Some(rate.write_bytes_per_sec),
                        read_latency_ms: rate.read_latency_ms,
                        write_latency_ms: rate.write_latency_ms,
                        io_util_pct: rate.io_util_pct,
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
        let net_rates = self.process_net.collect(&pids);
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
                let net_rate = net_rates.get(&pid);
                let net_rx_per_sec = net_rate.map(|r| r.rx_bytes_per_sec);
                let net_tx_per_sec = net_rate.map(|r| r.tx_bytes_per_sec);
                let net_rx_total = net_rate.map(|r| r.rx_total);
                let net_tx_total = net_rate.map(|r| r.tx_total);

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

                let name_lower = name.to_lowercase();
                let database_type = if name_lower.contains("postgres") {
                    Some(crate::models::DatabaseType::PostgreSQL)
                } else if name_lower.contains("mysqld") || name_lower.contains("mariadbd") {
                    Some(crate::models::DatabaseType::MySqlMariaDb)
                } else {
                    None
                };

                let proxy_type = if name_lower.contains("traefik") {
                    Some(crate::models::HttpProxyType::Traefik)
                } else if name_lower.contains("nginx") {
                    Some(crate::models::HttpProxyType::Nginx)
                } else if name_lower.contains("httpd") || name_lower.contains("apache") {
                    Some(crate::models::HttpProxyType::Apache)
                } else {
                    None
                };

                let node_runtime_type = if name_lower == "bun" || exe_path.contains("/bun") {
                    Some(crate::models::NodeRuntimeType::Bun)
                } else if name_lower == "deno" || exe_path.contains("/deno") {
                    Some(crate::models::NodeRuntimeType::Deno)
                } else if matches!(
                    name_lower.as_str(),
                    "node"
                        | "npm"
                        | "npx"
                        | "pnpm"
                        | "pnpx"
                        | "yarn"
                        | "pm2"
                        | "nest"
                        | "tsx"
                        | "ts-node"
                        | "ts-node-esm"
                ) || exe_path.contains("/node")
                    || exe_path.contains("/.nvm/")
                    || exe_path.contains("/nvm/versions/")
                {
                    Some(crate::models::NodeRuntimeType::Node)
                } else {
                    None
                };
                let message_broker_type = if name_lower.contains("redpanda") {
                    Some(crate::models::MessageBrokerType::Redpanda)
                } else if name_lower.contains("kafka") || cmd.to_lowercase().contains("kafka") {
                    Some(crate::models::MessageBrokerType::Kafka)
                } else {
                    None
                };

                ProcessData {
                    pid,
                    name,
                    user,
                    cpu_pct,
                    memory_bytes,
                    memory_pct,
                    disk_read_per_sec,
                    disk_write_per_sec,
                    net_rx_per_sec,
                    net_tx_per_sec,
                    net_rx_total,
                    net_tx_total,
                    status,
                    uptime_secs,
                    threads,
                    exe_path,
                    cmd,
                    cwd,
                    database_type,
                    proxy_type,
                    node_runtime_type,
                    message_broker_type,
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
        let gpus = self.gpu.collect();
        let tcp_stats = self.tcp.collect();
        let uptime_secs = System::uptime();

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
            gpus,
            tcp_stats,
            uptime_secs,
        }
    }
}

fn detect_core_type(core_id: usize, total_cores: usize, vendor: &str, brand: &str) -> CoreType {
    let vendor_lower = vendor.to_lowercase();
    let brand_lower = brand.to_lowercase();

    // Apple Silicon detection
    if vendor_lower.contains("apple") || brand_lower.contains("apple") {
        // On Apple Silicon, first cores are typically P-cores, last are E-cores
        // M1/M2/M3: 4P+4E (8 cores), M1 Pro/Max: 6P+2E or 8P+2E, M1 Ultra: 16P+4E
        // Heuristic: if we have 8+ cores, assume first half are P, second half E
        if total_cores >= 8 {
            let p_core_count = match total_cores {
                8 => 4,  // M1/M2/M3 base
                10 => 6, // M1 Pro/Max 10-core
                12 => 8, // M1 Pro/Max 12-core
                16 => 12, // M1 Max 16-core? Actually M1 Ultra has 16P+4E
                _ => total_cores / 2,
            };
            if core_id < p_core_count {
                return CoreType::Performance;
            } else {
                return CoreType::Efficiency;
            }
        }
        return CoreType::Standard;
    }

    // Intel hybrid (Alder Lake, Raptor Lake) - 12th/13th/14th gen
    if vendor_lower.contains("intel") || vendor_lower.contains("genuineintel") {
        // Check for hybrid architecture in brand string
        if brand_lower.contains("hybrid") || brand_lower.contains("12th") || brand_lower.contains("13th") || brand_lower.contains("14th") {
            // On Linux, we'd need to read /proc/cpuinfo for core type
            // For now, use a heuristic: if > 8 cores, likely hybrid
            if total_cores > 8 {
                // Typical: 8P + 8E = 16 threads, or 6P + 4E = 10 cores
                // P-cores usually come first in numbering
                let p_core_estimate = (total_cores as f64 * 0.6) as usize;
                if core_id < p_core_estimate {
                    return CoreType::Performance;
                } else {
                    return CoreType::Efficiency;
                }
            }
        }
        return CoreType::Standard;
    }

    // AMD - typically all standard cores (Zen architecture)
    if vendor_lower.contains("amd") || vendor_lower.contains("authenticamd") {
        return CoreType::Standard;
    }

    CoreType::Unknown
}

#[cfg(target_os = "linux")]
fn read_core_temperature(core_id: usize) -> Option<f64> {
    use std::fs;
    // Try multiple thermal zone paths
    let paths = [
        format!("/sys/class/thermal/thermal_zone{}/temp", core_id),
        format!("/sys/class/hwmon/hwmon{}/temp{}_input", core_id / 4 + 1, (core_id % 4) + 1),
        format!("/sys/devices/platform/coretemp.{}/hwmon/hwmon*/temp{}_input", core_id, core_id + 1),
    ];

    for path in &paths {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(temp_millicelsius) = content.trim().parse::<i64>() {
                return Some(temp_millicelsius as f64 / 1000.0);
            }
        }
    }

    // Fallback: try to read from coretemp hwmon
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let name_path = entry.path().join("name");
            if let Ok(name) = fs::read_to_string(&name_path) {
                if name.trim().contains("coretemp") || name.trim().contains("k10temp") || name.trim().contains("zenpower") {
                    let temp_path = entry.path().join(format!("temp{}_input", core_id + 1));
                    if let Ok(content) = fs::read_to_string(&temp_path) {
                        if let Ok(temp_millicelsius) = content.trim().parse::<i64>() {
                            return Some(temp_millicelsius as f64 / 1000.0);
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(not(target_os = "linux"))]
fn read_core_temperature(_core_id: usize) -> Option<f64> {
    None
}
