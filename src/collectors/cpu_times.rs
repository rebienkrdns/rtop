/// Collector for CPU time breakdown and system-wide counters from /proc/stat.
/// Linux only; returns default (all None) on other platforms.

#[cfg(target_os = "linux")]
#[derive(Default, Clone)]
struct ProcStatSample {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    ctx_switches: u64,
    interrupts: u64,
}

#[derive(Default, Clone)]
pub struct CpuTimesResult {
    pub user_pct: Option<f64>,
    pub system_pct: Option<f64>,
    pub iowait_pct: Option<f64>,
    pub steal_pct: Option<f64>,
    pub ctx_switches_per_sec: Option<f64>,
    pub interrupts_per_sec: Option<f64>,
}

pub struct CpuTimesCollector {
    #[cfg(target_os = "linux")]
    prev: Option<(ProcStatSample, std::time::Instant)>,
}

impl Default for CpuTimesCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuTimesCollector {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            prev: None,
        }
    }

    pub fn collect(&mut self) -> CpuTimesResult {
        #[cfg(target_os = "linux")]
        {
            let now = std::time::Instant::now();
            if let Some(sample) = Self::read_proc_stat() {
                let result = if let Some((ref prev_sample, prev_time)) = self.prev {
                    let elapsed = now.duration_since(prev_time).as_secs_f64();
                    compute_result(prev_sample, &sample, elapsed)
                } else {
                    CpuTimesResult::default()
                };
                self.prev = Some((sample, now));
                return result;
            }
        }
        CpuTimesResult::default()
    }

    #[cfg(target_os = "linux")]
    fn read_proc_stat() -> Option<ProcStatSample> {
        let content = std::fs::read_to_string("/proc/stat").ok()?;
        let mut sample = ProcStatSample::default();

        for line in content.lines() {
            if line.starts_with("cpu ") {
                // cpu user nice system idle iowait irq softirq steal ...
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 9 {
                    sample.user = parts[1].parse().unwrap_or(0);
                    sample.nice = parts[2].parse().unwrap_or(0);
                    sample.system = parts[3].parse().unwrap_or(0);
                    sample.idle = parts[4].parse().unwrap_or(0);
                    sample.iowait = parts[5].parse().unwrap_or(0);
                    sample.irq = parts[6].parse().unwrap_or(0);
                    sample.softirq = parts[7].parse().unwrap_or(0);
                    sample.steal = parts[8].parse().unwrap_or(0);
                }
            } else if line.starts_with("ctxt ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    sample.ctx_switches = parts[1].parse().unwrap_or(0);
                }
            } else if line.starts_with("intr ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    sample.interrupts = parts[1].parse().unwrap_or(0);
                }
            }
        }

        Some(sample)
    }
}

#[cfg(target_os = "linux")]
fn compute_result(prev: &ProcStatSample, curr: &ProcStatSample, elapsed: f64) -> CpuTimesResult {
    let delta_user = curr.user.saturating_sub(prev.user);
    let delta_nice = curr.nice.saturating_sub(prev.nice);
    let delta_system = curr.system.saturating_sub(prev.system);
    let delta_idle = curr.idle.saturating_sub(prev.idle);
    let delta_iowait = curr.iowait.saturating_sub(prev.iowait);
    let delta_irq = curr.irq.saturating_sub(prev.irq);
    let delta_softirq = curr.softirq.saturating_sub(prev.softirq);
    let delta_steal = curr.steal.saturating_sub(prev.steal);

    let delta_total = delta_user
        + delta_nice
        + delta_system
        + delta_idle
        + delta_iowait
        + delta_irq
        + delta_softirq
        + delta_steal;

    if delta_total == 0 || elapsed <= 0.0 {
        return CpuTimesResult::default();
    }

    let pct = |v: u64| -> Option<f64> { Some(v as f64 / delta_total as f64 * 100.0) };

    let delta_ctx = curr.ctx_switches.saturating_sub(prev.ctx_switches);
    let delta_intr = curr.interrupts.saturating_sub(prev.interrupts);

    CpuTimesResult {
        // Include nice in user, irq/softirq in system for simplicity
        user_pct: pct(delta_user + delta_nice),
        system_pct: pct(delta_system + delta_irq + delta_softirq),
        iowait_pct: pct(delta_iowait),
        steal_pct: pct(delta_steal),
        ctx_switches_per_sec: Some(delta_ctx as f64 / elapsed),
        interrupts_per_sec: Some(delta_intr as f64 / elapsed),
    }
}
