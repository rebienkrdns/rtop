use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::timeout;

pub const NODE_HISTORY_LEN: usize = 60;

#[derive(Clone, Debug, Default)]
pub struct EventLoopMetrics {
    pub delay_ms: f64,
    pub utilization_pct: f64,
}

#[derive(Clone, Debug, Default)]
pub struct HeapMetrics {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub new_space_bytes: u64,
    pub old_space_bytes: u64,
    pub code_space_bytes: u64,
    pub map_space_bytes: u64,
    pub minor_gc_rate: f64,
    pub minor_gc_avg_ms: f64,
    pub major_gc_rate: f64,
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

#[derive(Clone, Debug, Default)]
pub struct LibuvMetrics {
    pub active_handles: u32,
    pub active_requests: u32,
    pub threadpool_queue: u32,
}

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
    pub elu_history: VecDeque<u64>,
    pub delay_history: VecDeque<u64>,
    pub heap_used_history: VecDeque<u64>,
    pub minor_gc_history: VecDeque<u64>,
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
        push(&mut self.minor_gc_history, (self.metrics.heap.minor_gc_rate * 10.0) as u64);
        push(&mut self.major_gc_history, (self.metrics.heap.major_gc_rate * 10.0) as u64);
    }
}

// ─── Expresión JS única que extrae todas las métricas necesarias ──────────────

/// Una sola evaluación que devuelve JSON con todos los campos de métricas.
/// Usa try/catch para que `require('v8')` no rompa si no está disponible.
/// `require` no es global en el contexto del inspector V8 (no hay module wrapper).
/// Usamos `process.mainModule.require` como alternativa portable, con fallbacks.
const METRICS_EXPRESSION: &str = r#"(function(){
  var m=process.memoryUsage();
  var elu=(typeof performance!=='undefined'&&performance.eventLoopUtilization)?performance.eventLoopUtilization():{utilization:0};
  var spaces={};
  try{
    var req=typeof require!=='undefined'?require:(process.mainModule?process.mainModule.require:null);
    if(req){req('v8').getHeapSpaceStatistics().forEach(function(s){spaces[s.space_name]=s.space_used_size;});}
  }catch(e){}
  return JSON.stringify({
    heapUsed:m.heapUsed,heapTotal:m.heapTotal,
    elu:elu.utilization,
    newSpace:spaces['new_space']||0,
    oldSpace:spaces['old_space']||0,
    codeSpace:spaces['code_space']||0,
    mapSpace:spaces['map_space']||0
  });
})()"#;

// ─── Sesión persistente de WebSocket al V8 Inspector ─────────────────────────

struct InspectorSession {
    stream: TcpStream,
    msg_id: u32,
}

impl InspectorSession {
    /// Establece la conexión WebSocket con el inspector de Node.js en el puerto dado.
    async fn connect(port: u16) -> Option<Self> {
        let ws_url = get_inspector_ws_url(port).await?;
        let parsed = url_mod::parse_simple(&ws_url)?;
        let addr = format!("{}:{}", parsed.host, parsed.port);

        let mut stream = timeout(Duration::from_millis(1500), TcpStream::connect(&addr))
            .await
            .ok()?
            .ok()?;

        let path = match parsed.query {
            Some(ref q) if !q.is_empty() => format!("{}?{}", parsed.path, q),
            _ => parsed.path.clone(),
        };
        let key = "dEdJuAx5DkXel2KQQQYQJQ==";
        let handshake = format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
            if path.starts_with('/') { path } else { format!("/{}", path) },
            parsed.host,
            parsed.port,
            key
        );
        stream.write_all(handshake.as_bytes()).await.ok()?;

        let mut buf = [0u8; 4096];
        let n = timeout(Duration::from_millis(1500), stream.read(&mut buf))
            .await
            .ok()?
            .ok()?;
        let header = String::from_utf8_lossy(&buf[..n]);
        if !header.contains("101") {
            return None;
        }

        Some(Self { stream, msg_id: 1 })
    }

    /// Envía un Runtime.evaluate y devuelve el string resultado o None si la conexión falló.
    async fn eval(&mut self, expression: &str) -> Option<String> {
        let id = self.msg_id;
        self.msg_id += 1;

        let cmd = serde_json::json!({
            "id": id,
            "method": "Runtime.evaluate",
            "params": {
                "expression": expression,
                "returnByValue": true
            }
        });

        let frame = encode_ws_frame(cmd.to_string().as_bytes());
        self.stream.write_all(&frame).await.ok()?;

        // Leer respuesta — puede llegar en varios fragmentos TCP.
        // Leemos hasta tener un frame WS completo.
        let mut buf = vec![0u8; 65536];
        let n = timeout(Duration::from_secs(3), self.stream.read(&mut buf))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            return None;
        }

        let payload = decode_ws_frame(&buf[..n])?;
        let resp: serde_json::Value = serde_json::from_slice(&payload).ok()?;

        // El inspector puede enviar eventos no solicitados (Runtime.executionContextCreated, etc.)
        // antes del resultado. Si el `id` no coincide, descartamos y no intentamos releer
        // (el siguiente poll obtendrá el valor correcto).
        if resp.get("id").and_then(|v| v.as_u64()) != Some(id as u64) {
            // Era un evento, no el resultado de nuestro eval
            return None;
        }

        resp.pointer("/result/result/value")
            .and_then(|v| v.as_str())
            .map(String::from)
    }

    /// Envía un frame de cierre WebSocket (opcode 0x08) antes de cerrar.
    async fn close(mut self) {
        let close_frame = vec![0x88u8, 0x80, 0x00, 0x00, 0x00, 0x00]; // FIN+close, masked empty
        let _ = self.stream.write_all(&close_frame).await;
    }
}

// ─── Loop de sesión persistente (llamado desde app.rs via tokio::spawn) ──────

/// Arranca un loop de monitoreo persistente para un proceso Node.js.
/// Mantiene la conexión WebSocket abierta entre polls.
/// Solo reconecta cuando la conexión se rompe.
pub async fn run_inspector_session_process(
    pid: u32,
    tx: mpsc::Sender<NodeMonitorData>,
) {
    run_session_loop(move || NodeMonitorData::new(pid), None, Some(pid), tx).await;
}

pub async fn run_inspector_session_container(
    container_id: String,
    tx: mpsc::Sender<NodeMonitorData>,
) {
    let cid_for_loop = container_id.clone();
    run_session_loop(
        move || NodeMonitorData::new_container(container_id.clone()),
        Some(cid_for_loop),
        None,
        tx,
    )
    .await;
}

async fn run_session_loop<F>(
    make_data: F,
    container_id: Option<String>,
    pid: Option<u32>,
    tx: mpsc::Sender<NodeMonitorData>,
) where
    F: Fn() -> NodeMonitorData,
{
    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    let mut session: Option<InspectorSession> = None;

    loop {
        ticker.tick().await;

        // Si no hay sesión, intentar conectar
        if session.is_none() {
            let port = find_inspector_port(pid, container_id.as_deref()).await;
            if let Some(p) = port {
                let mut data = make_data();
                data.status = NodeConnectionStatus::Connecting;
                let _ = tx.send(data).await;

                session = InspectorSession::connect(p).await;
            }

            if session.is_none() {
                let mut data = make_data();
                data.status = NodeConnectionStatus::Unavailable;
                if tx.send(data).await.is_err() {
                    break;
                }
                continue;
            }
        }

        // Evaluar métricas en la sesión existente
        let metrics_opt = if let Some(ref mut sess) = session {
            match sess.eval(METRICS_EXPRESSION).await {
                Some(val_str) => parse_metrics_json(&val_str),
                None => None,
            }
        } else {
            None
        };

        match metrics_opt {
            Some(metrics) => {
                let mut data = make_data();
                data.status = NodeConnectionStatus::Connected;
                data.metrics = metrics;
                if tx.send(data).await.is_err() {
                    break;
                }
            }
            None => {
                // Conexión rota — cerrar limpiamente y marcar para reconexión
                if let Some(sess) = session.take() {
                    sess.close().await;
                }
                let mut data = make_data();
                data.status = NodeConnectionStatus::Unavailable;
                if tx.send(data).await.is_err() {
                    break;
                }
            }
        }
    }

    // Cerrar sesión al terminar
    if let Some(sess) = session {
        sess.close().await;
    }
}

fn parse_metrics_json(val_str: &str) -> Option<NodeRuntimeMetrics> {
    let v: serde_json::Value = serde_json::from_str(val_str).ok()?;
    Some(NodeRuntimeMetrics {
        event_loop: EventLoopMetrics {
            delay_ms: 0.0,
            utilization_pct: v["elu"].as_f64().unwrap_or(0.0) * 100.0,
        },
        heap: HeapMetrics {
            used_bytes: v["heapUsed"].as_u64().unwrap_or(0),
            total_bytes: v["heapTotal"].as_u64().unwrap_or(0),
            new_space_bytes: v["newSpace"].as_u64().unwrap_or(0),
            old_space_bytes: v["oldSpace"].as_u64().unwrap_or(0),
            code_space_bytes: v["codeSpace"].as_u64().unwrap_or(0),
            map_space_bytes: v["mapSpace"].as_u64().unwrap_or(0),
            minor_gc_rate: 0.0,
            minor_gc_avg_ms: 0.0,
            major_gc_rate: 0.0,
            major_gc_avg_ms: 0.0,
        },
        libuv: LibuvMetrics::default(),
    })
}

// ─── Descubrimiento de puerto ────────────────────────────────────────────────

async fn find_inspector_port(_pid: Option<u32>, _container_id: Option<&str>) -> Option<u16> {
    // Rango convencional de Node.js inspector
    for port in 9229u16..=9239 {
        if probe_tcp_port(port).await {
            return Some(port);
        }
    }

    // En Linux: leer cmdline del proceso para extraer --inspect-port
    #[cfg(target_os = "linux")]
    if let Some(p) = _pid {
        if let Ok(cmdline) = tokio::fs::read_to_string(format!("/proc/{}/cmdline", p)).await {
            for part in cmdline.split('\0') {
                if let Some(rest) = part
                    .strip_prefix("--inspect-port=")
                    .or_else(|| part.strip_prefix("--inspect=127.0.0.1:"))
                    .or_else(|| part.strip_prefix("--inspect=0.0.0.0:"))
                {
                    if let Ok(port) = rest.trim_end_matches('\0').parse::<u16>() {
                        if probe_tcp_port(port).await {
                            return Some(port);
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
        Duration::from_millis(200),
        TcpStream::connect(&addr),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .is_some()
}

// ─── HTTP GET al endpoint /json del inspector ─────────────────────────────────

async fn get_inspector_ws_url(port: u16) -> Option<String> {
    let body = timeout(
        Duration::from_millis(800),
        fetch_http_json(port),
    )
    .await
    .ok()??;

    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    let raw_url = parsed
        .as_array()?
        .first()?
        .get("webSocketDebuggerUrl")?
        .as_str()?;

    Some(normalize_ws_url(raw_url, port))
}

async fn fetch_http_json(port: u16) -> Option<String> {
    let addr = format!("127.0.0.1:{}", port);
    let mut stream = timeout(Duration::from_millis(800), TcpStream::connect(&addr))
        .await
        .ok()?
        .ok()?;

    // HTTP/1.1 con Host incluyendo puerto — Node.js lo usa para construir la wsURL
    let req = format!(
        "GET /json HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        port
    );
    stream.write_all(req.as_bytes()).await.ok()?;

    let mut buf = Vec::new();
    let _ = timeout(Duration::from_millis(800), stream.read_to_end(&mut buf)).await;
    let response = String::from_utf8_lossy(&buf);

    response
        .split_once("\r\n\r\n")
        .or_else(|| response.split_once("\n\n"))
        .map(|(_, body)| body.to_string())
}

/// Garantiza que la URL WS tenga el puerto explícito.
fn normalize_ws_url(url: &str, fallback_port: u16) -> String {
    let without_scheme = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))
        .unwrap_or(url);

    let has_port = without_scheme
        .split('/')
        .next()
        .map(|h| h.contains(':'))
        .unwrap_or(false);

    if has_port {
        return url.to_string();
    }

    let scheme = if url.starts_with("wss://") { "wss" } else { "ws" };
    if let Some(slash) = without_scheme.find('/') {
        let host = &without_scheme[..slash];
        let path = &without_scheme[slash..];
        format!("{}://{}:{}{}", scheme, host, fallback_port, path)
    } else {
        format!("{}://{}:{}", scheme, without_scheme, fallback_port)
    }
}

// ─── Framing WebSocket (RFC 6455) ─────────────────────────────────────────────

fn encode_ws_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.push(0x81); // FIN=1, opcode=1 (text)

    let len = payload.len();
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

fn decode_ws_frame(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 2 {
        return None;
    }
    let masked = (data[1] & 0x80) != 0;
    let mut len = (data[1] & 0x7f) as usize;
    let mut offset = 2usize;

    if len == 126 {
        if data.len() < 4 {
            return None;
        }
        len = u16::from_be_bytes([data[2], data[3]]) as usize;
        offset = 4;
    } else if len == 127 {
        if data.len() < 10 {
            return None;
        }
        len = u64::from_be_bytes(data[2..10].try_into().ok()?) as usize;
        offset = 10;
    }

    if masked {
        if data.len() < offset + 4 {
            return None;
        }
        let mask = &data[offset..offset + 4];
        offset += 4;
        if data.len() < offset + len {
            return None;
        }
        Some(
            data[offset..offset + len]
                .iter()
                .enumerate()
                .map(|(i, b)| b ^ mask[i % 4])
                .collect(),
        )
    } else {
        if data.len() < offset + len {
            return None;
        }
        Some(data[offset..offset + len].to_vec())
    }
}

// ─── Parser de URL mínimo ─────────────────────────────────────────────────────

mod url_mod {
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

        let (host_port, rest) = without_scheme
            .split_once('/')
            .unwrap_or((without_scheme, ""));
        let path = format!("/{}", rest.split('?').next().unwrap_or(""));
        let query = if rest.contains('?') {
            rest.split_once('?').map(|(_, q)| q.to_string())
        } else {
            None
        };

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
