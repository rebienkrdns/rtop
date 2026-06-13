use std::time::Instant;

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

#[derive(Debug, Clone)]
pub struct TcpStats {
    pub tcp_retransmissions: u64,
    pub tcp_retransmission_rate: f64,
    pub tcp_failed_connections: u64,
    pub tcp_resets: u64,
    pub tcp_retrans_fail: u64,
    #[allow(dead_code)]
    pub timestamp: Instant,
}
