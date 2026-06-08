#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContainerSortColumn {
    #[default]
    Name,
    Cpu,
    Memory,
    NetRecv,
    NetSent,
    DiskRead,
    DiskWrite,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ContainerStatus {
    Running,
    Paused,
    Restarting,
    Exited,
    Dead,
    #[default]
    Unknown,
}

impl ContainerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContainerStatus::Running => "activo",
            ContainerStatus::Paused => "pausado",
            ContainerStatus::Restarting => "reiniciando",
            ContainerStatus::Exited => "salido",
            ContainerStatus::Dead => "muerto",
            ContainerStatus::Unknown => "desconocido",
        }
    }
}

#[derive(Clone, Default, Debug)]
#[allow(dead_code)]
pub struct ContainerData {
    pub id: String,            // primeros 12 chars del ID
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub uptime_secs: Option<u64>,
    pub cpu_pct: f64,
    pub memory_bytes: u64,
    pub memory_limit_bytes: u64,
    pub memory_pct: f64,
    pub net_recv_per_sec: f64,
    pub net_recv_total: u64,
    pub net_sent_per_sec: f64,
    pub net_sent_total: u64,
    pub disk_read_per_sec: f64,
    pub disk_read_total: u64,
    pub disk_write_per_sec: f64,
    pub disk_write_total: u64,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub networks: Vec<String>,
    pub env_vars: Vec<String>,
}
