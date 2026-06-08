use std::collections::VecDeque;

const MAX_SAMPLES: usize = 3600; // 1 hora a 1 muestra/seg

#[derive(Clone)]
pub struct MetricSample {
    pub cpu_pct: f64,
    pub mem_pct: f64,
    #[allow(dead_code)]
    pub load1: f64,
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
