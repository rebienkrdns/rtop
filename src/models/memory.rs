#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct MemoryData {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total: u64,
    pub swap_used: u64,
    pub usage_pct: f64,
}
