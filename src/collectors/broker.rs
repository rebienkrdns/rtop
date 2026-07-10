use crate::models::{ContainerData, MessageBrokerType, ProcessData};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Clone, Debug, Default)]
pub struct BrokerMetrics {
    pub messages_per_sec: f64,
    pub bytes_per_sec: f64,
    pub under_replicated_partitions: u32,
    pub consumer_lag: u64,
    pub active_topics: u32,
    pub active_partitions: u32,
    pub active_consumers: u32,

    // Raw values to compute rates
    pub raw_messages_total: u64,
    pub raw_bytes_total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrokerConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct BrokerMonitorData {
    pub pid: Option<u32>,
    pub container_id: Option<String>,
    pub broker_type: MessageBrokerType,
    pub status: BrokerConnectionStatus,
    pub metrics: BrokerMetrics,
}

impl BrokerMonitorData {
    pub fn new(pid: u32, broker_type: MessageBrokerType) -> Self {
        Self {
            pid: Some(pid),
            container_id: None,
            broker_type,
            status: BrokerConnectionStatus::Disconnected,
            metrics: BrokerMetrics::default(),
        }
    }

    pub fn new_container(container_id: String, broker_type: MessageBrokerType) -> Self {
        Self {
            pid: None,
            container_id: Some(container_id),
            broker_type,
            status: BrokerConnectionStatus::Disconnected,
            metrics: BrokerMetrics::default(),
        }
    }
}

pub fn extract_broker_port(cmd: &str, broker_type: MessageBrokerType) -> u16 {
    // Redpanda admin port is 9644. Kafka Prometheus/JMX exporter usually on 9404 or 7071
    let default_port = match broker_type {
        MessageBrokerType::Redpanda => 9644,
        MessageBrokerType::Kafka => 9404,
    };

    let words: Vec<&str> = cmd.split_whitespace().collect();
    for i in 0..words.len() {
        if (words[i] == "-p" || words[i] == "--port") && i + 1 < words.len() {
            if let Ok(port) = words[i + 1].parse::<u16>() {
                return port;
            }
        }
    }
    default_port
}

pub fn extract_container_host_port(ports: &[String], broker_type: MessageBrokerType) -> u16 {
    let target_port = match broker_type {
        MessageBrokerType::Redpanda => "9644",
        MessageBrokerType::Kafka => "9404",
    };

    for p in ports {
        if p.contains(&format!("->{}", target_port))
            || p.contains(&format!("->{}/tcp", target_port))
        {
            if let Some(left) = p.split("->").next() {
                let port_str = left.split(':').next_back().unwrap_or(left).trim();
                if let Ok(port) = port_str.parse::<u16>() {
                    return port;
                }
            }
        }
    }

    match broker_type {
        MessageBrokerType::Redpanda => 9644,
        MessageBrokerType::Kafka => 9404,
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
    }

    if let Some(pos) = resp.find("\r\n\r\n") {
        Ok(resp[pos + 4..].to_string())
    } else if let Some(pos) = resp.find("\n\n") {
        Ok(resp[pos + 2..].to_string())
    } else {
        Ok(resp)
    }
}

fn parse_prometheus_metrics(body: &str, metrics: &mut BrokerMetrics) {
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split metric name and value (ignoring labels in brackets)
        let parts: Vec<&str> = if let Some(brace_start) = line.find('{') {
            let metric_name = &line[0..brace_start];
            let rest = &line[brace_start..];
            if let Some(brace_end) = rest.find('}') {
                let value_part = rest[brace_end + 1..].trim();
                vec![metric_name, value_part]
            } else {
                line.split_whitespace().collect()
            }
        } else {
            line.split_whitespace().collect()
        };

        if parts.len() < 2 {
            continue;
        }

        let key = parts[0].trim();
        let val_str = parts[1].trim();
        let val: f64 = val_str.parse().unwrap_or(0.0);

        // Map redpanda or kafka metrics
        match key {
            // Under-replicated partitions
            "redpanda_cluster_partition_under_replicated_replicas"
            | "redpanda_under_replicated_partition"
            | "kafka_server_ReplicaManager_UnderReplicatedPartitions"
            | "kafka_server_replica_manager_underreplicatedpartitions" => {
                metrics.under_replicated_partitions = val as u32;
            }

            // Consumer Lag
            "redpanda_kafka_max_consumer_lag"
            | "redpanda_kafka_consumer_group_lag"
            | "kafka_consumergroup_lag"
            | "kafka_consumer_group_lag" => {
                if val as u64 > metrics.consumer_lag {
                    metrics.consumer_lag = val as u64;
                }
            }

            // Messages/Bytes rates raw counters
            "redpanda_kafka_request_bytes_total"
            | "kafka_server_brokertopicmetrics_bytesin_total" => {
                metrics.raw_bytes_total = val as u64;
            }
            "redpanda_kafka_request_tx_bytes_total"
            | "kafka_server_brokertopicmetrics_bytesout_total" => {
                if metrics.raw_bytes_total == 0 {
                    metrics.raw_bytes_total = val as u64;
                }
            }
            "kafka_server_brokertopicmetrics_messagesin_total" => {
                metrics.raw_messages_total = val as u64;
            }

            // Topics, Partitions, Consumers metadata
            "redpanda_cluster_topics" | "kafka_controller_KafkaController_GlobalTopicCount" => {
                metrics.active_topics = val as u32;
            }
            "redpanda_cluster_partitions"
            | "kafka_controller_KafkaController_GlobalPartitionCount" => {
                metrics.active_partitions = val as u32;
            }
            "redpanda_kafka_consumer_groups" | "kafka_server_GroupMetadataManager_NumGroups" => {
                metrics.active_consumers = val as u32;
            }
            _ => {}
        }
    }
}

pub async fn poll_broker_at_port(
    broker_type: MessageBrokerType,
    port: u16,
    metrics: &mut BrokerMetrics,
) -> BrokerConnectionStatus {
    let path = match broker_type {
        MessageBrokerType::Redpanda => "/public_metrics",
        MessageBrokerType::Kafka => "/metrics",
    };

    match http_get("127.0.0.1", port, path).await {
        Ok(body) => {
            parse_prometheus_metrics(&body, metrics);
            BrokerConnectionStatus::Connected
        }
        Err(e) => {
            // For Redpanda, if /public_metrics failed, try alternative /metrics path
            if broker_type == MessageBrokerType::Redpanda {
                if let Ok(body) = http_get("127.0.0.1", port, "/metrics").await {
                    parse_prometheus_metrics(&body, metrics);
                    return BrokerConnectionStatus::Connected;
                }
            }
            BrokerConnectionStatus::Error(e)
        }
    }
}

pub async fn poll_broker(process: ProcessData) -> BrokerMonitorData {
    let broker_type = match process.message_broker_type {
        Some(t) => t,
        None => return BrokerMonitorData::new(process.pid, MessageBrokerType::Redpanda),
    };

    let mut data = BrokerMonitorData::new(process.pid, broker_type);
    data.status = BrokerConnectionStatus::Connecting;

    let port = extract_broker_port(&process.cmd, broker_type);
    data.status = poll_broker_at_port(broker_type, port, &mut data.metrics).await;
    data
}

pub async fn poll_broker_container(container: ContainerData) -> BrokerMonitorData {
    let broker_type = match container.message_broker_type {
        Some(t) => t,
        None => return BrokerMonitorData::new_container(container.id, MessageBrokerType::Redpanda),
    };

    let mut data = BrokerMonitorData::new_container(container.id.clone(), broker_type);
    data.status = BrokerConnectionStatus::Connecting;

    let port = extract_container_host_port(&container.ports, broker_type);
    data.status = poll_broker_at_port(broker_type, port, &mut data.metrics).await;
    data
}
