#[derive(Clone, Default)]
pub struct DiskData {
    pub device: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_pct: f64,
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}
