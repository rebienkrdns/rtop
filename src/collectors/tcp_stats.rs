use crate::models::network::TcpStats;

#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::time::Instant;

pub struct TcpStatsCollector {
    #[cfg(target_os = "linux")]
    prev_retrans: Option<u64>,
    #[cfg(target_os = "linux")]
    prev_out_segs: Option<u64>,
}

impl Default for TcpStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpStatsCollector {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            prev_retrans: None,
            #[cfg(target_os = "linux")]
            prev_out_segs: None,
        }
    }

    pub fn collect(&mut self) -> Option<TcpStats> {
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
        #[cfg(target_os = "linux")]
        {
            self.collect_linux()
        }
    }

    #[cfg(target_os = "linux")]
    fn collect_linux(&mut self) -> Option<TcpStats> {
        let snmp = std::fs::read_to_string("/proc/net/snmp").ok()?;
        let snmp_map = parse_snmp_block(&snmp);

        let netstat = std::fs::read_to_string("/proc/net/netstat").ok()?;
        let netstat_map = parse_snmp_block(&netstat);

        let retrans_segs = snmp_map.get("Tcp/RetransSegs").copied().unwrap_or(0);
        let out_segs = snmp_map.get("Tcp/OutSegs").copied().unwrap_or(1);
        let attempt_fails = snmp_map.get("Tcp/AttemptFails").copied().unwrap_or(0);
        let estab_resets = snmp_map.get("Tcp/EstabResets").copied().unwrap_or(0);
        let retrans_fail = netstat_map
            .get("TcpExt/TCPRetransFail")
            .copied()
            .unwrap_or(0);

        let retransmission_rate = match (self.prev_retrans, self.prev_out_segs) {
            (Some(prev_r), Some(prev_o)) => {
                let delta_r = retrans_segs.saturating_sub(prev_r) as f64;
                let delta_o = out_segs.saturating_sub(prev_o) as f64;
                if delta_o > 0.0 {
                    (delta_r / delta_o * 100.0).clamp(0.0, 100.0)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

        self.prev_retrans = Some(retrans_segs);
        self.prev_out_segs = Some(out_segs);

        Some(TcpStats {
            tcp_retransmissions: retrans_segs,
            tcp_retransmission_rate: retransmission_rate,
            tcp_failed_connections: attempt_fails,
            tcp_resets: estab_resets,
            tcp_retrans_fail: retrans_fail,
            timestamp: Instant::now(),
        })
    }
}

#[cfg(target_os = "linux")]
fn parse_snmp_block(content: &str) -> HashMap<String, u64> {
    let mut result = HashMap::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i + 1 < lines.len() {
        let header_parts: Vec<&str> = lines[i].split_whitespace().collect();
        let value_parts: Vec<&str> = lines[i + 1].split_whitespace().collect();

        if header_parts.len() >= 2 && value_parts.len() >= 2 {
            let prefix = header_parts[0].trim_end_matches(':');
            for (key, val) in header_parts[1..].iter().zip(value_parts[1..].iter()) {
                let full_key = format!("{}/{}", prefix, key);
                if let Ok(n) = val.parse::<u64>() {
                    result.insert(full_key, n);
                }
            }
        }
        i += 2;
    }
    result
}
