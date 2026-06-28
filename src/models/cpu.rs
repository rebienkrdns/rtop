#[derive(Clone, Default, Debug)]
#[allow(dead_code)]
pub struct CpuCoreData {
    pub core_id: usize,
    pub usage_pct: f64,
    pub frequency_mhz: u64,
    pub temperature_celsius: Option<f64>,
    pub core_type: CoreType,
    pub vendor_id: String,
    pub brand: String,
}

#[derive(Clone, Default, Debug, PartialEq)]
#[allow(dead_code)]
pub enum CoreType {
    #[default]
    Standard,
    Performance,
    Efficiency,
    Unknown,
}

impl CoreType {
    pub fn label(&self) -> &'static str {
        match self {
            CoreType::Performance => "P",
            CoreType::Efficiency => "E",
            CoreType::Standard => "S",
            CoreType::Unknown => "?",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        match self {
            CoreType::Performance => ratatui::style::Color::Red,
            CoreType::Efficiency => ratatui::style::Color::Green,
            CoreType::Standard => ratatui::style::Color::Blue,
            CoreType::Unknown => ratatui::style::Color::Gray,
        }
    }
}

#[derive(Clone, Default, Debug)]
#[allow(dead_code)]
pub struct CpuData {
    pub global_usage_pct: f64,
    pub per_core: Vec<CpuCoreData>,
    pub core_count: usize,
    // CPU time breakdown (Linux /proc/stat)
    pub user_pct: Option<f64>,
    pub system_pct: Option<f64>,
    pub iowait_pct: Option<f64>,
    pub steal_pct: Option<f64>,
    pub ctx_switches_per_sec: Option<f64>,
    pub interrupts_per_sec: Option<f64>,
}