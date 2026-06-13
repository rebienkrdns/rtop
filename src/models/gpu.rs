#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GpuProcessData {
    pub pid: u32,
    pub used_vram_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct GpuData {
    pub index: u32,
    pub name: String,
    pub utilization_pct: u32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub temperature_c: u32,
    pub processes: Vec<GpuProcessData>,
}

impl GpuData {
    pub fn memory_usage_pct(&self) -> f64 {
        if self.memory_total_bytes == 0 {
            return 0.0;
        }
        (self.memory_used_bytes as f64 / self.memory_total_bytes as f64) * 100.0
    }
}
