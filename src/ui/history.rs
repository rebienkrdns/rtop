use std::collections::VecDeque;

const MAX_SAMPLES: usize = 3600; // 1 hora a 1 muestra/seg

/// Calculates throughput utilization using the configured I/O capacity in decimal MB/s.
/// Storage capacity is intentionally not part of this calculation.
pub fn disk_throughput_pct(read_bps: f64, write_bps: f64, capacity_mb_s: u64) -> f64 {
    let capacity_bps = capacity_mb_s as f64 * 1_000_000.0;
    if capacity_bps <= 0.0 {
        return 0.0;
    }
    ((read_bps.max(0.0) + write_bps.max(0.0)) / capacity_bps * 100.0).clamp(0.0, 100.0)
}

#[derive(Clone)]
pub struct MetricSample {
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub net_recv_bps: f64,
    pub net_sent_bps: f64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
    /// Read + write throughput expressed as a percentage of the configured I/O capacity.
    pub disk_throughput_pct: f64,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ProcessHistorySample {
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub memory_bytes: u64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ContainerHistorySample {
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub memory_bytes: u64,
    pub net_recv_bps: f64,
    pub net_sent_bps: f64,
    pub disk_read_bps: f64,
    pub disk_write_bps: f64,
}

pub struct MetricsHistory {
    pub samples: VecDeque<MetricSample>,
}

impl MetricsHistory {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_SAMPLES),
        }
    }

    pub fn push(&mut self, sample: MetricSample) {
        if self.samples.len() >= MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    pub fn tail_n(&self, n: usize) -> Vec<&MetricSample> {
        let skip = self.samples.len().saturating_sub(n);
        self.samples.iter().skip(skip).collect()
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum HistoryRange {
    OneMin,
    FiveMin,
    FifteenMin,
    OneHour,
}

impl HistoryRange {
    pub fn samples(self) -> usize {
        match self {
            Self::OneMin => 60,
            Self::FiveMin => 300,
            Self::FifteenMin => 900,
            Self::OneHour => 3600,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OneMin => "1 min",
            Self::FiveMin => "5 min",
            Self::FifteenMin => "15 min",
            Self::OneHour => "1 hora",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::OneMin => Self::FiveMin,
            Self::FiveMin => Self::FifteenMin,
            Self::FifteenMin => Self::OneHour,
            Self::OneHour => Self::OneMin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::disk_throughput_pct;

    #[test]
    fn disk_throughput_percentage_uses_transfer_capacity() {
        assert_eq!(disk_throughput_pct(0.0, 10_000_000.0, 100), 10.0);
        assert_eq!(disk_throughput_pct(20_000_000.0, 90_000_000.0, 100), 100.0);
        assert_eq!(disk_throughput_pct(10_000_000.0, 0.0, 0), 0.0);
    }
}
