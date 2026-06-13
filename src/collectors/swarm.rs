use std::path::Path;

use bollard::service::ListServicesOptions;
use bollard::Docker;

use crate::models::swarm::{SwarmData, SwarmNodeData, SwarmServiceData};

pub struct SwarmCollector {
    docker: Option<Docker>,
}

impl SwarmCollector {
    pub async fn new() -> Self {
        Self {
            docker: connect_docker().await,
        }
    }

    pub async fn refresh(&mut self) -> SwarmData {
        if self.docker.is_none() {
            self.docker = connect_docker().await;
        }

        let Some(docker) = &self.docker else {
            return SwarmData {
                available: false,
                message: Some("Docker no disponible".to_string()),
                ..Default::default()
            };
        };

        let info = match docker.info().await {
            Ok(i) => i,
            Err(e) => {
                self.docker = None;
                return SwarmData {
                    available: false,
                    message: Some(format!("Error de Docker: {e}")),
                    ..Default::default()
                };
            }
        };

        use bollard::secret::LocalNodeState;
        let is_active = info
            .swarm
            .as_ref()
            .and_then(|s| s.local_node_state.as_ref())
            .map(|s| matches!(s, LocalNodeState::ACTIVE))
            .unwrap_or(false);

        let is_manager = info
            .swarm
            .as_ref()
            .and_then(|s| s.control_available)
            .unwrap_or(false);

        if !is_active || !is_manager {
            return SwarmData {
                available: true,
                is_manager: false,
                message: Some("Este nodo no es Swarm Manager".to_string()),
                ..Default::default()
            };
        }

        match collect_swarm(docker).await {
            Ok((services, nodes)) => SwarmData {
                available: true,
                is_manager: true,
                message: None,
                services,
                nodes,
            },
            Err(msg) => SwarmData {
                available: true,
                is_manager: true,
                message: Some(msg),
                ..Default::default()
            },
        }
    }
}

async fn connect_docker() -> Option<Docker> {
    let candidates = [
        "/var/run/docker.sock",
        "/run/podman/podman.sock",
        "/run/user/1000/podman/podman.sock",
    ];
    for path in candidates {
        if !Path::new(path).exists() {
            continue;
        }
        if let Ok(docker) =
            Docker::connect_with_unix(path, 120, bollard::API_DEFAULT_VERSION)
        {
            if docker.version().await.is_ok() {
                return Some(docker);
            }
        }
    }
    None
}

async fn collect_swarm(
    docker: &Docker,
) -> Result<(Vec<SwarmServiceData>, Vec<SwarmNodeData>), String> {
    let services_raw = docker
        .list_services(None::<ListServicesOptions<String>>)
        .await
        .map_err(|e| format!("Error listando servicios: {e}"))?;

    let mut services: Vec<SwarmServiceData> = services_raw
        .iter()
        .map(|svc| {
            let id = svc.id.as_deref().unwrap_or_default().to_string();
            let name = svc
                .spec
                .as_ref()
                .and_then(|s| s.name.as_deref())
                .unwrap_or_default()
                .to_string();

            let raw_image = svc
                .spec
                .as_ref()
                .and_then(|s| s.task_template.as_ref())
                .and_then(|t| t.container_spec.as_ref())
                .and_then(|c| c.image.as_deref())
                .unwrap_or_default();
            let image = raw_image
                .split('@')
                .next()
                .unwrap_or(raw_image)
                .to_string();

            let replicas_running = svc
                .service_status
                .as_ref()
                .and_then(|ss| ss.running_tasks)
                .unwrap_or(0);

            let replicas_desired = svc
                .service_status
                .as_ref()
                .and_then(|ss| ss.desired_tasks)
                .unwrap_or_else(|| {
                    svc.spec
                        .as_ref()
                        .and_then(|s| s.mode.as_ref())
                        .and_then(|m| m.replicated.as_ref())
                        .and_then(|r| r.replicas)
                        .unwrap_or(0) as u64
                });

            SwarmServiceData {
                id,
                name,
                replicas_running,
                replicas_desired,
                image,
                restart_count: 0,
            }
        })
        .collect();
    services.sort_by(|a, b| a.name.cmp(&b.name));

    // list_nodes not in bollard 0.16 — call Docker REST API directly
    let nodes = fetch_nodes().await;

    Ok((services, nodes))
}

/// Calls GET /nodes on the Docker Unix socket and deserializes with bollard models.
async fn fetch_nodes() -> Vec<SwarmNodeData> {
    let sock = [
        "/var/run/docker.sock",
        "/run/podman/podman.sock",
        "/run/user/1000/podman/podman.sock",
    ]
    .iter()
    .find(|p| Path::new(p).exists())
    .copied();

    let Some(sock) = sock else { return vec![] };

    let body = match docker_unix_get(sock, "/nodes").await {
        Some(b) => b,
        None => return vec![],
    };

    let raw_nodes: Vec<bollard::models::Node> =
        serde_json::from_slice(&body).unwrap_or_default();

    raw_nodes
        .iter()
        .map(|node| {
            let id = node
                .id
                .as_deref()
                .unwrap_or_default()
                .chars()
                .take(12)
                .collect();

            let hostname = node
                .description
                .as_ref()
                .and_then(|d| d.hostname.as_deref())
                .unwrap_or_default()
                .to_string();

            let status = node
                .status
                .as_ref()
                .and_then(|s| s.state.as_ref())
                .map(|s| format!("{s}"))
                .unwrap_or_else(|| "unknown".to_string());
            let status = capitalize(&status);

            let role = node
                .spec
                .as_ref()
                .and_then(|s| s.role.as_ref())
                .map(|r| format!("{r}"))
                .unwrap_or_else(|| "worker".to_string());
            let role = capitalize(&role);

            let availability = node
                .spec
                .as_ref()
                .and_then(|s| s.availability.as_ref())
                .map(|a| format!("{a}"))
                .unwrap_or_else(|| "active".to_string());
            let availability = capitalize(&availability);

            SwarmNodeData {
                id,
                hostname,
                status,
                role,
                availability,
            }
        })
        .collect()
}

/// Minimal HTTP GET over a Docker Unix socket, returns the response body.
async fn docker_unix_get(sock: &str, path: &str) -> Option<Vec<u8>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(sock).await.ok()?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        path
    );
    stream.write_all(req.as_bytes()).await.ok()?;
    stream.flush().await.ok()?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.ok()?;

    let sep = buf.windows(4).position(|w| w == b"\r\n\r\n")?;
    let header_str = std::str::from_utf8(&buf[..sep]).ok()?.to_lowercase();
    let body = &buf[sep + 4..];

    if header_str.contains("transfer-encoding: chunked") {
        decode_chunked(body)
    } else {
        Some(body.to_vec())
    }
}

fn decode_chunked(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let eol = data[pos..].windows(2).position(|w| w == b"\r\n")? + pos;
        let size_hex = std::str::from_utf8(&data[pos..eol]).ok()?.trim();
        let size_hex = size_hex.split(';').next().unwrap_or(size_hex);
        let chunk_size = usize::from_str_radix(size_hex, 16).ok()?;
        pos = eol + 2;
        if chunk_size == 0 {
            break;
        }
        let end = pos + chunk_size;
        if end > data.len() {
            break;
        }
        out.extend_from_slice(&data[pos..end]);
        pos = end + 2;
    }
    Some(out)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
