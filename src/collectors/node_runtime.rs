use std::collections::VecDeque;
use std::time::Duration;
use tokio::time::timeout;

pub const NODE_HISTORY_LEN: usize = 60;

/// Telemetría del Event Loop de Node.js
#[derive(Clone, Debug, Default)]
pub struct EventLoopMetrics {
    /// Latencia del event loop en milisegundos
    pub delay_ms: f64,
    /// Event Loop Utilization (0.0 – 100.0 %)
    pub utilization_pct: f64,
}

/// Métricas del heap de la V8
#[derive(Clone, Debug, Default)]
pub struct HeapMetrics {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub new_space_bytes: u64,
    pub old_space_bytes: u64,
    pub code_space_bytes: u64,
    pub map_space_bytes: u64,
    /// Frecuencia de Minor GC por segundo (scavenge)
    pub minor_gc_rate: f64,
    /// Latencia promedio de Minor GC en ms
    pub minor_gc_avg_ms: f64,
    /// Frecuencia de Major GC por segundo (mark-sweep)
    pub major_gc_rate: f64,
    /// Latencia promedio de Major GC en ms
    pub major_gc_avg_ms: f64,
}

impl HeapMetrics {
    pub fn used_pct(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes as f64 / self.total_bytes as f64 * 100.0
        }
    }
}

/// Métricas del subsistema I/O de Libuv
#[derive(Clone, Debug, Default)]
pub struct LibuvMetrics {
    pub active_handles: u32,
    pub active_requests: u32,
    pub threadpool_queue: u32,
}

/// Snapshot completo de métricas de un proceso/contenedor Node.js
#[derive(Clone, Debug, Default)]
pub struct NodeRuntimeMetrics {
    pub event_loop: EventLoopMetrics,
    pub heap: HeapMetrics,
    pub libuv: LibuvMetrics,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Unavailable,
}

#[allow(clippy::derivable_impls)]
impl Default for NodeConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

#[derive(Clone, Debug)]
pub struct NodeMonitorData {
    pub pid: Option<u32>,
    pub container_id: Option<String>,
    pub status: NodeConnectionStatus,
    pub metrics: NodeRuntimeMetrics,
    /// Historial de ELU (%)
    pub elu_history: VecDeque<u64>,
    /// Historial de delay (ms × 10 para preservar décimas)
    pub delay_history: VecDeque<u64>,
    /// Historial de heap used (KB)
    pub heap_used_history: VecDeque<u64>,
    /// Historial de minor GC rate (× 10)
    pub minor_gc_history: VecDeque<u64>,
    /// Historial de major GC rate (× 10)
    pub major_gc_history: VecDeque<u64>,
}

impl NodeMonitorData {
    pub fn new(pid: u32) -> Self {
        Self {
            pid: Some(pid),
            container_id: None,
            status: NodeConnectionStatus::Disconnected,
            metrics: NodeRuntimeMetrics::default(),
            elu_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            delay_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            heap_used_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            minor_gc_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            major_gc_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
        }
    }

    pub fn new_container(container_id: String) -> Self {
        Self {
            pid: None,
            container_id: Some(container_id),
            status: NodeConnectionStatus::Disconnected,
            metrics: NodeRuntimeMetrics::default(),
            elu_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            delay_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            heap_used_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            minor_gc_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
            major_gc_history: VecDeque::with_capacity(NODE_HISTORY_LEN),
        }
    }

    pub fn push_history(&mut self) {
        let push = |dq: &mut VecDeque<u64>, v: u64| {
            if dq.len() >= NODE_HISTORY_LEN {
                dq.pop_front();
            }
            dq.push_back(v);
        };
        push(&mut self.elu_history, self.metrics.event_loop.utilization_pct as u64);
        push(&mut self.delay_history, (self.metrics.event_loop.delay_ms * 10.0) as u64);
        push(&mut self.heap_used_history, self.metrics.heap.used_bytes / 1024);
        push(&mut self.minor_gc_history, (self.metrics.event_loop.delay_ms * 10.0) as u64);
        push(&mut self.major_gc_history, (self.metrics.heap.major_gc_rate * 10.0) as u64);
    }
}

// ─── V8 Inspector WebSocket client ──────────────────────────────────────────

/// Detecta el puerto de inspección V8 de un proceso Node.js leyendo /proc/<pid>/net/tcp
/// o netstat. En macOS usa lsof. Retorna el primer puerto en rango 9229–9239.
pub async fn discover_inspector_port(_pid: u32) -> Option<u16> {
    // Rango convencional de Node.js inspector
    for port in 9229u16..=9239 {
        if probe_tcp_port(port).await {
            return Some(port);
        }
    }
    // Intentar leer /proc/<pid>/cmdline para extraer --inspect-port=XXXX
    #[cfg(target_os = "linux")]
    {
        if let Ok(cmdline) = tokio::fs::read_to_string(format!("/proc/{}/cmdline", _pid)).await {
            let parts: Vec<&str> = cmdline.split('\0').collect();
            for part in &parts {
                if let Some(rest) = part.strip_prefix("--inspect-port=").or_else(|| part.strip_prefix("--inspect=")) {
                    if let Ok(p) = rest.trim_end_matches('\0').parse::<u16>() {
                        if probe_tcp_port(p).await {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

async fn probe_tcp_port(port: u16) -> bool {
    let addr = format!("127.0.0.1:{}", port);
    timeout(
        Duration::from_millis(150),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .is_some()
}

// ─── HTTP scraping del endpoint /json del V8 Inspector ───────────────────────

/// Obtiene las métricas del Node.js Inspector HTTP endpoint (/json/runtime)
/// Parsea la respuesta del inspector para extraer métricas básicas.
pub async fn poll_node_inspector(pid: u32) -> NodeMonitorData {
    let mut data = NodeMonitorData::new(pid);

    let Some(port) = discover_inspector_port(pid).await else {
        data.status = NodeConnectionStatus::Unavailable;
        return data;
    };

    data.status = NodeConnectionStatus::Connecting;

    // Intentar conectar al endpoint HTTP del inspector para obtener el websocket URL
    let ws_url = match get_inspector_ws_url(port).await {
        Some(url) => url,
        None => {
            data.status = NodeConnectionStatus::Unavailable;
            return data;
        }
    };

    // Conectar via WebSocket y ejecutar Runtime.evaluate para leer métricas
    match collect_metrics_via_ws(&ws_url).await {
        Some(metrics) => {
            data.status = NodeConnectionStatus::Connected;
            data.metrics = metrics;
        }
        None => {
            data.status = NodeConnectionStatus::Unavailable;
        }
    }

    data
}

pub async fn poll_node_inspector_container(container_id: &str) -> NodeMonitorData {
    let mut data = NodeMonitorData::new_container(container_id.to_string());

    // Para contenedores intentamos el mismo mecanismo: Node.js expone el inspector
    // en un puerto mapeado al host. Escaneamos puertos convencionales.
    let Some(port) = find_mapped_inspector_port(container_id).await else {
        data.status = NodeConnectionStatus::Unavailable;
        return data;
    };

    let ws_url = match get_inspector_ws_url(port).await {
        Some(url) => url,
        None => {
            data.status = NodeConnectionStatus::Unavailable;
            return data;
        }
    };

    match collect_metrics_via_ws(&ws_url).await {
        Some(metrics) => {
            data.status = NodeConnectionStatus::Connected;
            data.metrics = metrics;
        }
        None => {
            data.status = NodeConnectionStatus::Unavailable;
        }
    }

    data
}

async fn find_mapped_inspector_port(_container_id: &str) -> Option<u16> {
    // Probar puertos convencionales del inspector
    for port in [9229u16, 9230, 9231, 9232] {
        if probe_tcp_port(port).await {
            return Some(port);
        }
    }
    None
}

async fn get_inspector_ws_url(port: u16) -> Option<String> {
    let url = format!("http://127.0.0.1:{}/json", port);
    let Ok(resp) = timeout(Duration::from_millis(800), fetch_http(&url, port)).await else {
        return None;
    };
    let body = resp?;

    // Parsear JSON array de targets: [{"webSocketDebuggerUrl": "ws://..."}]
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body) else {
        return None;
    };

    let raw_url = parsed
        .as_array()?
        .first()?
        .get("webSocketDebuggerUrl")?
        .as_str()?;

    // Algunos runtimes devuelven la URL sin puerto (cuando usan HTTP/1.1 con Host sin puerto).
    // Nos aseguramos de que siempre tenga host:port correcto.
    let normalized = normalize_ws_url(raw_url, port);
    Some(normalized)
}

/// Garantiza que la URL WS tenga el puerto explícito. Por ejemplo:
/// "ws://127.0.0.1/abc" → "ws://127.0.0.1:9229/abc"
fn normalize_ws_url(url: &str, fallback_port: u16) -> String {
    // Si ya tiene puerto explícito, devolver tal cual
    let without_scheme = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))
        .unwrap_or(url);

    let has_explicit_port = without_scheme
        .split('/')
        .next()
        .map(|host_part| host_part.contains(':'))
        .unwrap_or(false);

    if has_explicit_port {
        return url.to_string();
    }

    // Inyectar el puerto antes del path
    if let Some(path_start) = without_scheme.find('/') {
        let host = &without_scheme[..path_start];
        let path = &without_scheme[path_start..];
        let scheme = if url.starts_with("wss://") { "wss" } else { "ws" };
        format!("{}://{}:{}{}", scheme, host, fallback_port, path)
    } else {
        // sin path
        let scheme = if url.starts_with("wss://") { "wss" } else { "ws" };
        format!("{}://{}:{}", scheme, without_scheme, fallback_port)
    }
}

/// Hace un GET HTTP/1.1 al endpoint del inspector.
/// `port` se necesita para construir el Host header correcto (Node.js lo usa para generar la wsURL).
async fn fetch_http(url: &str, port: u16) -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let url_parsed = url::parse_simple(url)?;
    let addr = format!("{}:{}", url_parsed.host, url_parsed.port);
    let path = url_parsed.path;

    let mut stream = timeout(Duration::from_millis(800), TcpStream::connect(&addr))
        .await
        .ok()?
        .ok()?;

    // HTTP/1.1 con Host incluyendo puerto — necesario para que el inspector
    // genere la webSocketDebuggerUrl con el puerto correcto.
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        path, url_parsed.host, port
    );
    stream.write_all(request.as_bytes()).await.ok()?;

    let mut buf = Vec::new();
    let _ = timeout(Duration::from_millis(800), stream.read_to_end(&mut buf)).await;
    let response = String::from_utf8_lossy(&buf);

    // Separar cabeceras del cuerpo (CRLF o LF)
    response
        .split_once("\r\n\r\n")
        .or_else(|| response.split_once("\n\n"))
        .map(|(_, body)| body.to_string())
}

/// Conecta via WebSocket al Inspector V8 y ejecuta comandos para obtener métricas de heap,
/// event loop y GC.
async fn collect_metrics_via_ws(ws_url: &str) -> Option<NodeRuntimeMetrics> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let parsed = url::parse_simple(ws_url)?;
    let addr = format!("{}:{}", parsed.host, parsed.port);
    // Construir el path sin `?` colgante cuando no hay query string
    let path = match parsed.query {
        Some(ref q) if !q.is_empty() => format!("{}?{}", parsed.path, q),
        _ => parsed.path.clone(),
    };

    let mut stream = timeout(Duration::from_millis(1500), TcpStream::connect(&addr))
        .await
        .ok()?
        .ok()?;

    // WebSocket handshake (RFC 6455) — Host debe incluir el puerto
    let key = "dEdJuAx5DkXel2KQQQYQJQ==";
    let handshake = format!(
        "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        if path.starts_with('/') { path.clone() } else { format!("/{}", path) },
        parsed.host,
        parsed.port,
        key
    );
    stream.write_all(handshake.as_bytes()).await.ok()?;

    // Leer hasta doble CRLF (fin de cabeceras del handshake)
    let mut header_buf = [0u8; 4096];
    let n = timeout(Duration::from_millis(800), stream.read(&mut header_buf))
        .await
        .ok()?
        .ok()?;
    let header_str = String::from_utf8_lossy(&header_buf[..n]);
    if !header_str.contains("101") {
        return None;
    }

    // Enviar Runtime.evaluate para obtener process.memoryUsage() y performance.eventLoopUtilization()
    let eval_cmd = serde_json::json!({
        "id": 1,
        "method": "Runtime.evaluate",
        "params": {
            "expression": "(function() { var m = process.memoryUsage(); var elu = (typeof performance !== 'undefined' && performance.eventLoopUtilization) ? performance.eventLoopUtilization() : {utilization: 0}; return JSON.stringify({heapUsed: m.heapUsed, heapTotal: m.heapTotal, rss: m.rss, external: m.external, elu: elu.utilization}); })()",
            "returnByValue": true
        }
    });

    let cmd_str = eval_cmd.to_string();
    let frame = encode_ws_frame(cmd_str.as_bytes());
    stream.write_all(&frame).await.ok()?;

    // Leer respuesta
    let mut resp_buf = vec![0u8; 8192];
    let n = timeout(Duration::from_secs(2), stream.read(&mut resp_buf))
        .await
        .ok()?
        .ok()?;

    let ws_payload = decode_ws_frame(&resp_buf[..n])?;
    let resp_json: serde_json::Value = serde_json::from_slice(&ws_payload).ok()?;

    let result_str = resp_json
        .pointer("/result/result/value")
        .and_then(|v| v.as_str())?;
    let result: serde_json::Value = serde_json::from_str(result_str).ok()?;

    let heap_used = result["heapUsed"].as_u64().unwrap_or(0);
    let heap_total = result["heapTotal"].as_u64().unwrap_or(0);
    let elu = result["elu"].as_f64().unwrap_or(0.0) * 100.0;

    // Segunda llamada: getHeapSpaceStatistics
    let heap_spaces_cmd = serde_json::json!({
        "id": 2,
        "method": "Runtime.evaluate",
        "params": {
            "expression": "JSON.stringify(require('v8').getHeapSpaceStatistics())",
            "returnByValue": true
        }
    });
    let cmd2 = heap_spaces_cmd.to_string();
    let frame2 = encode_ws_frame(cmd2.as_bytes());
    let _ = stream.write_all(&frame2).await;

    let mut resp2_buf = vec![0u8; 16384];
    let spaces_result = if let Ok(Ok(n2)) = timeout(Duration::from_secs(2), stream.read(&mut resp2_buf)).await {
        let payload2 = decode_ws_frame(&resp2_buf[..n2]).unwrap_or_default();
        let resp2: serde_json::Value = serde_json::from_slice(&payload2).unwrap_or_default();
        let spaces_str = resp2
            .pointer("/result/result/value")
            .and_then(|v| v.as_str())
            .unwrap_or("[]");
        serde_json::from_str::<serde_json::Value>(spaces_str).unwrap_or_default()
    } else {
        serde_json::Value::Null
    };

    let get_space = |name: &str| -> u64 {
        spaces_result.as_array().map_or(0, |arr| {
            arr.iter()
                .find(|s| s["space_name"].as_str() == Some(name))
                .and_then(|s| s["space_used_size"].as_u64())
                .unwrap_or(0)
        })
    };

    Some(NodeRuntimeMetrics {
        event_loop: EventLoopMetrics {
            delay_ms: 0.0, // no disponible sin perf_hooks activos; se puede añadir con otra eval
            utilization_pct: elu,
        },
        heap: HeapMetrics {
            used_bytes: heap_used,
            total_bytes: heap_total,
            new_space_bytes: get_space("new_space"),
            old_space_bytes: get_space("old_space"),
            code_space_bytes: get_space("code_space"),
            map_space_bytes: get_space("map_space"),
            minor_gc_rate: 0.0,
            minor_gc_avg_ms: 0.0,
            major_gc_rate: 0.0,
            major_gc_avg_ms: 0.0,
        },
        libuv: LibuvMetrics::default(),
    })
}

/// Encodifica un frame WebSocket sin máscara (servidor → cliente usa sin máscara)
/// Para cliente → servidor la máscara es obligatoria.
fn encode_ws_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    // FIN=1, opcode=1 (text)
    frame.push(0x81);

    let len = payload.len();
    // MASK bit = 1 (cliente → servidor siempre enmascarado)
    let mask_key: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len < 65536 {
        frame.push(0x80 | 126u8);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127u8);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    frame.extend_from_slice(&mask_key);
    for (i, b) in payload.iter().enumerate() {
        frame.push(b ^ mask_key[i % 4]);
    }
    frame
}

/// Decodifica el payload de un frame WebSocket (sin máscara, desde servidor)
fn decode_ws_frame(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 2 {
        return None;
    }
    let masked = (data[1] & 0x80) != 0;
    let mut len = (data[1] & 0x7f) as usize;
    let mut offset = 2usize;

    if len == 126 {
        if data.len() < 4 { return None; }
        len = u16::from_be_bytes([data[2], data[3]]) as usize;
        offset = 4;
    } else if len == 127 {
        if data.len() < 10 { return None; }
        len = u64::from_be_bytes(data[2..10].try_into().ok()?) as usize;
        offset = 10;
    }

    if masked {
        if data.len() < offset + 4 { return None; }
        let mask = &data[offset..offset + 4];
        offset += 4;
        if data.len() < offset + len { return None; }
        let payload: Vec<u8> = data[offset..offset + len]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ mask[i % 4])
            .collect();
        Some(payload)
    } else {
        if data.len() < offset + len { return None; }
        Some(data[offset..offset + len].to_vec())
    }
}

/// Parser de URL mínimo para evitar dependencias extra
mod url {
    pub struct SimpleUrl {
        pub host: String,
        pub port: u16,
        pub path: String,
        pub query: Option<String>,
    }

    pub fn parse_simple(url: &str) -> Option<SimpleUrl> {
        let without_scheme = url
            .strip_prefix("ws://")
            .or_else(|| url.strip_prefix("http://"))
            .or_else(|| url.strip_prefix("wss://"))
            .or_else(|| url.strip_prefix("https://"))?;

        let (host_port, rest) = without_scheme.split_once('/').unwrap_or((without_scheme, ""));
        let path = format!("/{}", rest.split('?').next().unwrap_or(""));
        let query = rest.contains('?').then(|| rest.split_once('?').map(|(_, q)| q.to_string())).flatten();

        let (host, port_str) = if host_port.contains(':') {
            host_port.split_once(':')?
        } else {
            (host_port, "80")
        };
        let port = port_str.parse().ok()?;

        Some(SimpleUrl {
            host: host.to_string(),
            port,
            path,
            query,
        })
    }
}
