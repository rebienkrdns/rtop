#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SwarmServiceData {
    pub id: String,
    pub name: String,
    pub replicas_running: u64,
    pub replicas_desired: u64,
    pub image: String,
    pub restart_count: u64,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SwarmNodeData {
    pub id: String,
    pub hostname: String,
    pub status: String,
    pub role: String,
    pub availability: String,
}

#[derive(Debug, Clone, Default)]
pub struct SwarmData {
    pub is_manager: bool,
    pub available: bool,
    pub message: Option<String>,
    pub services: Vec<SwarmServiceData>,
    pub nodes: Vec<SwarmNodeData>,
}
