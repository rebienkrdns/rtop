use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::enums::device::UsedGpuMemory;
use nvml_wrapper::Nvml;

use crate::models::gpu::{GpuData, GpuProcessData};

pub struct GpuCollector {
    nvml: Option<Nvml>,
}

impl GpuCollector {
    pub fn new() -> Self {
        let nvml = Nvml::init().ok();
        Self { nvml }
    }

    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        self.nvml.is_some()
    }

    pub fn collect(&self) -> Vec<GpuData> {
        let nvml = match &self.nvml {
            Some(n) => n,
            None => return vec![],
        };

        let count = nvml.device_count().unwrap_or(0);
        let mut gpus = Vec::with_capacity(count as usize);

        for i in 0..count {
            let Ok(device) = nvml.device_by_index(i) else {
                continue;
            };

            let name = device.name().unwrap_or_else(|_| format!("GPU {i}"));

            let utilization_pct = device.utilization_rates().map(|u| u.gpu).unwrap_or(0);

            let (memory_used_bytes, memory_total_bytes) = device
                .memory_info()
                .map(|m| (m.used, m.total))
                .unwrap_or((0, 0));

            let temperature_c = device.temperature(TemperatureSensor::Gpu).unwrap_or(0);

            let processes = device
                .running_compute_processes()
                .unwrap_or_default()
                .into_iter()
                .map(|p| GpuProcessData {
                    pid: p.pid,
                    used_vram_bytes: match p.used_gpu_memory {
                        UsedGpuMemory::Used(b) => b,
                        UsedGpuMemory::Unavailable => 0,
                    },
                })
                .collect();

            gpus.push(GpuData {
                index: i,
                name,
                utilization_pct,
                memory_used_bytes,
                memory_total_bytes,
                temperature_c,
                processes,
            });
        }

        gpus
    }
}

impl Default for GpuCollector {
    fn default() -> Self {
        Self::new()
    }
}
