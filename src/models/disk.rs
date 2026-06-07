#[derive(Clone)]
#[allow(dead_code)]
pub struct DiskData {
    pub device: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub usage_pct: f64,
}
