use crate::models::{ContainerData, HttpProxyType, ProcessData};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Clone, Debug, Default)]
pub struct ProxyMetrics {
    pub active_connections: u32,
    pub requests_total: u64,
    pub rps: f64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub reading: u32,
    pub writing: u32,
    pub waiting: u32,
    pub busy_workers: u32,
    pub idle_workers: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxyConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct ProxyMonitorData {
    pub pid: Option<u32>,
    pub container_id: Option<String>,
    pub proxy_type: HttpProxyType,
    pub status: ProxyConnectionStatus,
    pub metrics: ProxyMetrics,
}

impl ProxyMonitorData {
    pub fn new(pid: u32, proxy_type: HttpProxyType) -> Self {
        Self {
            pid: Some(pid),
            container_id: None,
            proxy_type,
            status: ProxyConnectionStatus::Disconnected,
            metrics: ProxyMetrics::default(),
        }
    }

    pub fn new_container(container_id: String, proxy_type: HttpProxyType) -> Self {
        Self {
            pid: None,
            container_id: Some(container_id),
            proxy_type,
            status: ProxyConnectionStatus::Disconnected,
            metrics: ProxyMetrics::default(),
        }
    }
}

pub fn extract_proxy_port(cmd: &str, proxy_type: HttpProxyType) -> u16 {
    let default_port = match proxy_type {
        HttpProxyType::Traefik => 8080,
        HttpProxyType::Nginx => 80,
        HttpProxyType::Apache => 80,
    };

    let words: Vec<&str> = cmd.split_whitespace().collect();
    for i in 0..words.len() {
        if words[i] == "--port" || words[i] == "-p" {
            if let Some(w) = words.get(i + 1) {
                if let Ok(p) = w.parse::<u16>() {
                    return p;
                }
            }
        }
    }
    default_port
}

pub fn extract_proxy_container_port(ports: &[String], proxy_type: HttpProxyType) -> u16 {
    let candidates: &[u16] = match proxy_type {
        HttpProxyType::Traefik => &[8080, 80, 443],
        HttpProxyType::Nginx => &[80, 8080, 443],
        HttpProxyType::Apache => &[80, 8080, 443],
    };

    for &candidate in candidates {
        for p in ports {
            if let Some(right) = p.split("->").nth(1) {
                let clean_right = right.split('/').next().unwrap_or(right).trim();
                if let Ok(container_port) = clean_right.parse::<u16>() {
                    if container_port == candidate {
                        if let Some(left) = p.split("->").next() {
                            let port_str = left.split(':').next_back().unwrap_or(left).trim();
                            if let Ok(port) = port_str.parse::<u16>() {
                                return port;
                            }
                        }
                    }
                }
            }
        }
    }

    match proxy_type {
        HttpProxyType::Traefik => 8080,
        HttpProxyType::Nginx => 80,
        HttpProxyType::Apache => 80,
    }
}

async fn http_get(host: &str, port: u16, path: &str) -> Result<String, String> {
    let addr = format!("{}:{}", host, port);
    let stream = timeout(Duration::from_millis(1500), TcpStream::connect(&addr))
        .await
        .map_err(|_| "connect timeout".to_string())?
        .map_err(|e| e.to_string())?;

    let (mut reader, mut writer) = stream.into_split();
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );
    timeout(
        Duration::from_millis(1000),
        writer.write_all(request.as_bytes()),
    )
    .await
    .map_err(|_| "write timeout".to_string())?
    .map_err(|e| e.to_string())?;

    let mut buf = Vec::new();
    timeout(
        Duration::from_millis(1500),
        reader.read_to_end(&mut buf),
    )
    .await
    .map_err(|_| "read timeout".to_string())?
    .map_err(|e| e.to_string())?;

    let resp = String::from_utf8_lossy(&buf).into_owned();
    if resp.starts_with("HTTP/") {
        let status_line = resp.lines().next().unwrap_or("");
        let code: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if code != 200 {
            return Err(format!("HTTP {}", code));
        }
        // Return body after \r\n\r\n
        if let Some(body_start) = resp.find("\r\n\r\n") {
            return Ok(resp[body_start + 4..].to_string());
        }
    }
    Ok(resp)
}

fn parse_nginx_status(body: &str, metrics: &mut ProxyMetrics) {
    // Active connections: 42
    // server accepts handled requests
    //  100 100 250
    // Reading: 1 Writing: 3 Waiting: 38
    for line in body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Active connections:") {
            metrics.active_connections = rest.trim().parse().unwrap_or(0);
        } else if line.starts_with("Reading:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            // Reading: N Writing: M Waiting: K
            if parts.len() >= 6 {
                metrics.reading = parts[1].parse().unwrap_or(0);
                metrics.writing = parts[3].parse().unwrap_or(0);
                metrics.waiting = parts[5].parse().unwrap_or(0);
            }
        } else {
            // Line with 3 numbers: accepts handled requests
            let nums: Vec<u64> = line
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() == 3 {
                metrics.requests_total = nums[2];
            }
        }
    }
}

fn parse_apache_status(body: &str, metrics: &mut ProxyMetrics) {
    // Auto-format:
    // Total Accesses: 5234
    // BusyWorkers: 3
    // IdleWorkers: 247
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("Total Accesses:") {
            metrics.requests_total = rest.trim().parse().unwrap_or(0);
        } else if let Some(rest) = line.strip_prefix("BusyWorkers:") {
            metrics.busy_workers = rest.trim().parse().unwrap_or(0);
            metrics.active_connections = metrics.busy_workers;
        } else if let Some(rest) = line.strip_prefix("IdleWorkers:") {
            metrics.idle_workers = rest.trim().parse().unwrap_or(0);
        }
    }
}

fn parse_traefik_metrics(body: &str, metrics: &mut ProxyMetrics) {
    // Prometheus format:
    // traefik_entrypoint_requests_total{code="200",...} 42.0
    // traefik_entrypoint_open_connections{...} 5.0
    let mut total_2xx: u64 = 0;
    let mut total_3xx: u64 = 0;
    let mut total_4xx: u64 = 0;
    let mut total_5xx: u64 = 0;
    let mut open_conns: u32 = 0;

    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.contains("traefik_entrypoint_requests_total") {
            // Extract code label
            let code = extract_label(line, "code").unwrap_or_default();
            let value = parse_prometheus_value(line);
            match code.as_str() {
                c if c.starts_with('2') => total_2xx += value,
                c if c.starts_with('3') => total_3xx += value,
                c if c.starts_with('4') => total_4xx += value,
                c if c.starts_with('5') => total_5xx += value,
                _ => {}
            }
        } else if line.contains("traefik_entrypoint_open_connections") {
            open_conns += parse_prometheus_value(line) as u32;
        }
    }

    metrics.status_2xx = total_2xx;
    metrics.status_3xx = total_3xx;
    metrics.status_4xx = total_4xx;
    metrics.status_5xx = total_5xx;
    metrics.requests_total = total_2xx + total_3xx + total_4xx + total_5xx;
    metrics.active_connections = open_conns;
}

fn extract_label(line: &str, label: &str) -> Option<String> {
    let search = format!("{}=\"", label);
    let start = line.find(&search)? + search.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_prometheus_value(line: &str) -> u64 {
    line.split_whitespace()
        .last()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0) as u64
}

pub async fn poll_proxy_at_port(
    proxy_type: HttpProxyType,
    port: u16,
    metrics: &mut ProxyMetrics,
) -> ProxyConnectionStatus {
    let (path, parse_fn): (
        &str,
        fn(&str, &mut ProxyMetrics),
    ) = match proxy_type {
        HttpProxyType::Nginx => ("/nginx_status", parse_nginx_status),
        HttpProxyType::Apache => ("/server-status?auto", parse_apache_status),
        HttpProxyType::Traefik => ("/metrics", parse_traefik_metrics),
    };

    match http_get("127.0.0.1", port, path).await {
        Ok(body) => {
            parse_fn(&body, metrics);
            ProxyConnectionStatus::Connected
        }
        Err(e) => ProxyConnectionStatus::Error(e),
    }
}

pub async fn poll_proxy(process: ProcessData) -> ProxyMonitorData {
    let proxy_type = match process.proxy_type {
        Some(t) => t,
        None => return ProxyMonitorData::new(process.pid, HttpProxyType::Nginx),
    };

    let mut data = ProxyMonitorData::new(process.pid, proxy_type);
    data.status = ProxyConnectionStatus::Connecting;
    let port = extract_proxy_port(&process.cmd, proxy_type);
    data.status = poll_proxy_at_port(proxy_type, port, &mut data.metrics).await;
    data
}

pub async fn poll_proxy_container(container: ContainerData) -> ProxyMonitorData {
    let proxy_type = match container.proxy_type {
        Some(t) => t,
        None => return ProxyMonitorData::new_container(container.id, HttpProxyType::Nginx),
    };

    let mut data = ProxyMonitorData::new_container(container.id.clone(), proxy_type);
    data.status = ProxyConnectionStatus::Connecting;
    let port = extract_proxy_container_port(&container.ports, proxy_type);
    data.status = poll_proxy_at_port(proxy_type, port, &mut data.metrics).await;
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nginx_status() {
        let body = "Active connections: 42 \nserver accepts handled requests\n 100 100 250\nReading: 1 Writing: 3 Waiting: 38\n";
        let mut m = ProxyMetrics::default();
        parse_nginx_status(body, &mut m);
        assert_eq!(m.active_connections, 42);
        assert_eq!(m.requests_total, 250);
        assert_eq!(m.reading, 1);
        assert_eq!(m.writing, 3);
        assert_eq!(m.waiting, 38);
    }

    #[test]
    fn test_parse_apache_status() {
        let body = "Total Accesses: 5234\nBusyWorkers: 3\nIdleWorkers: 247\n";
        let mut m = ProxyMetrics::default();
        parse_apache_status(body, &mut m);
        assert_eq!(m.requests_total, 5234);
        assert_eq!(m.busy_workers, 3);
        assert_eq!(m.idle_workers, 247);
    }

    #[test]
    fn test_parse_traefik_metrics() {
        let body = r#"# HELP traefik_entrypoint_requests_total
# TYPE traefik_entrypoint_requests_total counter
traefik_entrypoint_requests_total{code="200",entrypoint="web",method="GET",protocol="http"} 42.0
traefik_entrypoint_requests_total{code="404",entrypoint="web",method="GET",protocol="http"} 5.0
traefik_entrypoint_open_connections{entrypoint="web",protocol="http"} 3.0
"#;
        let mut m = ProxyMetrics::default();
        parse_traefik_metrics(body, &mut m);
        assert_eq!(m.status_2xx, 42);
        assert_eq!(m.status_4xx, 5);
        assert_eq!(m.requests_total, 47);
        assert_eq!(m.active_connections, 3);
    }

    #[test]
    fn test_extract_proxy_container_port() {
        let ports = vec![
            "0.0.0.0:8080->8080/tcp".to_string(),
            "0.0.0.0:80->80/tcp".to_string(),
        ];
        assert_eq!(extract_proxy_container_port(&ports, HttpProxyType::Traefik), 8080);
        assert_eq!(extract_proxy_container_port(&ports, HttpProxyType::Nginx), 80);
    }
}
