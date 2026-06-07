#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct NetworkData {
    pub interface: String,
    pub recv_bytes_per_sec: f64,
    pub sent_bytes_per_sec: f64,
    pub total_recv_bytes: u64,
    pub total_sent_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub is_up: bool,
    pub is_loopback: bool,
    pub ip_address: Option<String>,
}
