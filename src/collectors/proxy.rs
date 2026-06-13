use crate::models::{ContainerData, HttpProxyType, ProcessData};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

pub const PROXY_HISTORY_LEN: usize = 3600; // 1 hour at 1 sample/s — matches max HistoryRange

/// Snapshot of Traefik's cumulative histogram buckets used to compute per-interval percentiles.
#[derive(Clone, Debug, Default)]
pub struct TraefikHistogramState {
    /// (bound_ms, cumulative_count) sorted by bound ascending
    pub buckets: Vec<(f64, u64)>,
    pub total: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ProxyMetrics {
    pub active_connections: u32,
    pub requests_total: u64,
    pub rps: f64,
    pub status_1xx: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub error_rate: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
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
    pub rps_history: VecDeque<u64>,
    pub conn_history: VecDeque<u64>,
    pub s1xx_history: VecDeque<u64>,
    pub s2xx_history: VecDeque<u64>,
    pub s3xx_history: VecDeque<u64>,
    pub s4xx_history: VecDeque<u64>,
    pub s5xx_history: VecDeque<u64>,
    pub p50_history: VecDeque<u64>,
    pub p95_history: VecDeque<u64>,
    pub p99_history: VecDeque<u64>,
}

impl ProxyMonitorData {
    fn new_inner(proxy_type: HttpProxyType) -> Self {
        Self {
            pid: None,
            container_id: None,
            proxy_type,
            status: ProxyConnectionStatus::Disconnected,
            metrics: ProxyMetrics::default(),
            rps_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            conn_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            s1xx_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            s2xx_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            s3xx_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            s4xx_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            s5xx_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            p50_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            p95_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
            p99_history: VecDeque::with_capacity(PROXY_HISTORY_LEN),
        }
    }

    pub fn push_history(&mut self) {
        let push = |dq: &mut VecDeque<u64>, v: u64| {
            if dq.len() >= PROXY_HISTORY_LEN {
                dq.pop_front();
            }
            dq.push_back(v);
        };
        push(&mut self.rps_history, self.metrics.rps as u64);
        push(
            &mut self.conn_history,
            self.metrics.active_connections as u64,
        );
        // Store percentage (0–100) so each chart tracks share-of-requests per interval
        let total_delta = self.metrics.status_1xx
            + self.metrics.status_2xx
            + self.metrics.status_3xx
            + self.metrics.status_4xx
            + self.metrics.status_5xx;
        let pct = |n: u64| -> u64 { n.checked_mul(100).map_or(0, |v| v / total_delta.max(1)) };
        push(&mut self.s1xx_history, pct(self.metrics.status_1xx));
        push(&mut self.s2xx_history, pct(self.metrics.status_2xx));
        push(&mut self.s3xx_history, pct(self.metrics.status_3xx));
        push(&mut self.s4xx_history, pct(self.metrics.status_4xx));
        push(&mut self.s5xx_history, pct(self.metrics.status_5xx));
        push(&mut self.p50_history, self.metrics.p50_ms as u64);
        push(&mut self.p95_history, self.metrics.p95_ms as u64);
        push(&mut self.p99_history, self.metrics.p99_ms as u64);
    }

    pub fn new(pid: u32, proxy_type: HttpProxyType) -> Self {
        let mut s = Self::new_inner(proxy_type);
        s.pid = Some(pid);
        s
    }

    pub fn new_container(container_id: String, proxy_type: HttpProxyType) -> Self {
        let mut s = Self::new_inner(proxy_type);
        s.container_id = Some(container_id);
        s
    }
}

pub fn extract_proxy_port(cmd: &str, proxy_type: HttpProxyType) -> u16 {
    let default_port = match proxy_type {
        HttpProxyType::Traefik => 9090,
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
        HttpProxyType::Traefik => &[9090, 8082, 8080, 80, 443],
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
        HttpProxyType::Traefik => 9090,
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
    timeout(Duration::from_millis(1500), reader.read_to_end(&mut buf))
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

fn parse_traefik_metrics(
    body: &str,
    metrics: &mut ProxyMetrics,
    prev_hist: Option<&TraefikHistogramState>,
) -> TraefikHistogramState {
    let mut total_1xx: u64 = 0;
    let mut total_2xx: u64 = 0;
    let mut total_3xx: u64 = 0;
    let mut total_4xx: u64 = 0;
    let mut total_5xx: u64 = 0;
    let mut open_conns: u32 = 0;

    // histogram buckets: (le_bound_ms, cumulative_count)
    let mut hist_buckets: Vec<(f64, u64)> = Vec::new();
    let mut hist_count: u64 = 0;

    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        if line.contains("traefik_entrypoint_requests_total") {
            let code = extract_label(line, "code").unwrap_or_default();
            let value = parse_prometheus_value(line);
            match code.as_str() {
                c if c.starts_with('1') => total_1xx += value,
                c if c.starts_with('2') => total_2xx += value,
                c if c.starts_with('3') => total_3xx += value,
                c if c.starts_with('4') => total_4xx += value,
                c if c.starts_with('5') => total_5xx += value,
                _ => {}
            }
        } else if line.contains("traefik_entrypoint_open_connections") {
            open_conns += parse_prometheus_value(line) as u32;
        } else if line.contains("traefik_entrypoint_request_duration_seconds_bucket") {
            if let Some(le) = extract_label(line, "le") {
                if le == "+Inf" {
                    hist_count += parse_prometheus_value(line);
                } else if let Ok(bound) = le.parse::<f64>() {
                    let count = parse_prometheus_value(line);
                    hist_buckets.push((bound * 1000.0, count)); // convert to ms
                }
            }
        }
    }

    hist_buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Compute p50/p95/p99 from delta histogram when prev state is available,
    // falling back to cumulative on the first poll.
    if hist_count > 0 && !hist_buckets.is_empty() {
        let (effective_buckets, effective_total): (Vec<(f64, u64)>, u64) =
            if let Some(prev) = prev_hist {
                let delta_total = hist_count.saturating_sub(prev.total);
                let delta_buckets: Vec<(f64, u64)> = hist_buckets
                    .iter()
                    .map(|&(bound, count)| {
                        let prev_count = prev
                            .buckets
                            .iter()
                            .find(|&&(b, _)| (b - bound).abs() < 0.0001)
                            .map(|&(_, c)| c)
                            .unwrap_or(0);
                        (bound, count.saturating_sub(prev_count))
                    })
                    .collect();
                (delta_buckets, delta_total)
            } else {
                (hist_buckets.clone(), hist_count)
            };

        if effective_total > 0 {
            metrics.p50_ms = percentile_from_buckets(&effective_buckets, effective_total, 0.50);
            metrics.p95_ms = percentile_from_buckets(&effective_buckets, effective_total, 0.95);
            metrics.p99_ms = percentile_from_buckets(&effective_buckets, effective_total, 0.99);
        }
    }

    let total = total_1xx + total_2xx + total_3xx + total_4xx + total_5xx;
    metrics.error_rate = if total > 0 {
        (total_5xx as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    metrics.status_1xx = total_1xx;
    metrics.status_2xx = total_2xx;
    metrics.status_3xx = total_3xx;
    metrics.status_4xx = total_4xx;
    metrics.status_5xx = total_5xx;
    metrics.requests_total = total;
    metrics.active_connections = open_conns;

    TraefikHistogramState {
        buckets: hist_buckets,
        total: hist_count,
    }
}

fn percentile_from_buckets(buckets: &[(f64, u64)], total: u64, pct: f64) -> f64 {
    // Use ceil so target is never 0 (floor would give 0 for total=1 and pct<1.0,
    // causing every percentile to return 0ms when there is only one request/interval).
    let target = ((total as f64 * pct).ceil() as u64).max(1);
    let mut prev_bound = 0.0_f64;
    let mut prev_count = 0_u64;
    for &(bound, count) in buckets {
        if count >= target {
            if count == prev_count {
                return bound;
            }
            let frac = (target - prev_count) as f64 / (count - prev_count) as f64;
            return prev_bound + frac * (bound - prev_bound);
        }
        prev_bound = bound;
        prev_count = count;
    }
    prev_bound
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
    prev_hist: Option<&TraefikHistogramState>,
) -> (ProxyConnectionStatus, Option<TraefikHistogramState>) {
    match proxy_type {
        HttpProxyType::Traefik => match http_get("127.0.0.1", port, "/metrics").await {
            Ok(body) => {
                let hist = parse_traefik_metrics(&body, metrics, prev_hist);
                (ProxyConnectionStatus::Connected, Some(hist))
            }
            Err(e) => (ProxyConnectionStatus::Error(e), None),
        },
        _ => {
            let (path, parse_fn): (&str, fn(&str, &mut ProxyMetrics)) = match proxy_type {
                HttpProxyType::Nginx => ("/nginx_status", parse_nginx_status),
                HttpProxyType::Apache => ("/server-status?auto", parse_apache_status),
                HttpProxyType::Traefik => unreachable!(),
            };
            match http_get("127.0.0.1", port, path).await {
                Ok(body) => {
                    parse_fn(&body, metrics);
                    (ProxyConnectionStatus::Connected, None)
                }
                Err(e) => (ProxyConnectionStatus::Error(e), None),
            }
        }
    }
}

pub async fn poll_proxy(
    process: ProcessData,
    prev_hist: Option<&TraefikHistogramState>,
) -> (ProxyMonitorData, Option<TraefikHistogramState>) {
    let proxy_type = match process.proxy_type {
        Some(t) => t,
        None => return (ProxyMonitorData::new(process.pid, HttpProxyType::Nginx), None),
    };

    let mut data = ProxyMonitorData::new(process.pid, proxy_type);
    data.status = ProxyConnectionStatus::Connecting;
    let port = extract_proxy_port(&process.cmd, proxy_type);
    let (status, hist) = poll_proxy_at_port(proxy_type, port, &mut data.metrics, prev_hist).await;
    data.status = status;
    (data, hist)
}

pub async fn poll_proxy_container(
    container: ContainerData,
    prev_hist: Option<&TraefikHistogramState>,
) -> (ProxyMonitorData, Option<TraefikHistogramState>) {
    let proxy_type = match container.proxy_type {
        Some(t) => t,
        None => {
            return (
                ProxyMonitorData::new_container(container.id, HttpProxyType::Nginx),
                None,
            )
        }
    };

    let mut data = ProxyMonitorData::new_container(container.id.clone(), proxy_type);
    data.status = ProxyConnectionStatus::Connecting;
    let port = extract_proxy_container_port(&container.ports, proxy_type);
    let (status, hist) = poll_proxy_at_port(proxy_type, port, &mut data.metrics, prev_hist).await;
    data.status = status;
    (data, hist)
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
        parse_traefik_metrics(body, &mut m, None);
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
        assert_eq!(
            extract_proxy_container_port(&ports, HttpProxyType::Traefik),
            8080
        );
        assert_eq!(
            extract_proxy_container_port(&ports, HttpProxyType::Nginx),
            80
        );
    }
}
