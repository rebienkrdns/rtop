#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct CpuData {
    pub global_usage_pct: f64,
    pub per_core: Vec<f64>,
    pub core_count: usize,
    /// Load average: [1min, 5min, 15min]. Available on Linux and macOS.
    pub load_avg: [f64; 3],
}
