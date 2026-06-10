#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProcessSortColumn {
    #[default]
    Cpu,
    Memory,
    DiskRead,
    DiskWrite,
    NetRx,
    NetTx,
    Name,
}

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct ProcessIoData {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProcessStatus {
    Running,
    Sleeping,
    Stopped,
    Zombie,
    #[default]
    Other,
}

#[allow(dead_code)]
impl ProcessStatus {
    pub fn to_localized_str(self, lang: crate::localization::Language) -> &'static str {
        match lang {
            crate::localization::Language::Spanish => match self {
                ProcessStatus::Running => "ejecutando",
                ProcessStatus::Sleeping => "durmiendo",
                ProcessStatus::Stopped => "parado",
                ProcessStatus::Zombie => "zombi",
                ProcessStatus::Other => "otro",
            },
            crate::localization::Language::English => match self {
                ProcessStatus::Running => "running",
                ProcessStatus::Sleeping => "sleeping",
                ProcessStatus::Stopped => "stopped",
                ProcessStatus::Zombie => "zombie",
                ProcessStatus::Other => "other",
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatabaseType {
    PostgreSQL,
    MySqlMariaDb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum HttpProxyType {
    Traefik,
    Nginx,
    Apache,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeRuntimeType {
    Node,
    Bun,
    Deno,
}

impl NodeRuntimeType {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeRuntimeType::Node => "Node.js",
            NodeRuntimeType::Bun => "Bun",
            NodeRuntimeType::Deno => "Deno",
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ProcessData {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub cpu_pct: f64,
    pub memory_bytes: u64,
    pub memory_pct: f64,
    pub disk_read_per_sec: Option<f64>,
    pub disk_write_per_sec: Option<f64>,
    pub net_rx_per_sec: Option<f64>,
    pub net_tx_per_sec: Option<f64>,
    pub net_rx_total: Option<u64>,
    pub net_tx_total: Option<u64>,
    pub status: ProcessStatus,
    pub uptime_secs: u64,
    pub threads: u32,
    pub exe_path: String,
    pub cmd: String,
    pub cwd: String,
    pub database_type: Option<DatabaseType>,
    pub proxy_type: Option<HttpProxyType>,
    pub node_runtime_type: Option<NodeRuntimeType>,
}
