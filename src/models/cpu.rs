#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct CpuData {
    pub global_usage_pct: f64,
    pub per_core: Vec<f64>,
    pub core_count: usize,
}
