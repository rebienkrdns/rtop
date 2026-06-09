use std::collections::{HashMap, HashSet};
use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use sysinfo::System;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use bollard::Docker;

use crate::collectors::containers::{ContainerBackendState, ContainerCollector};
use crate::collectors::disk::{DiskIoCollector, DiskSelectorEntry};
use crate::collectors::system::SystemCollector;
use crate::config::{self, Config, Tab, INTERVALS};
use crate::models::{
    ContainerData, ContainerSortColumn, CpuData, DiskData, MemoryData, NetworkData,
    NetworkInterface, ProcessData, ProcessSortColumn, PsiData,
};
use crate::ui;
use crate::ui::history::{
    ContainerHistorySample, HistoryRange, MetricSample, MetricsHistory, ProcessHistorySample,
};
use crate::ui::views::container_detail::ConfirmAction;
use crate::ui::views::container_logs::LogsViewState;
use crate::ui::widgets::process_table::ProcessTableState;

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Main,
    ProcessDetail,
    ContainerDetail,
    ContainerLogs,
}

pub struct AppSnapshot {
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    pub network_by_nic: HashMap<String, NetworkData>,
    pub available_nics: Vec<NetworkInterface>,
    pub suggested_nic: Option<String>,
    pub proc_permission_denied: bool,
    pub processes: Vec<ProcessData>,
    pub containers: Vec<ContainerData>,
    pub container_state: ContainerBackendState,
    pub docker_client: Option<Docker>,
    pub psi: Option<PsiData>,
}

pub struct AppState {
    pub hostname: String,
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    pub interval_idx: usize,
    pub cfg: Config,
    pub active_tab: Tab,

    // Disk selection
    pub selected_disk: Option<String>,
    pub selector_entries: Vec<DiskSelectorEntry>,
    pub disk_selector_cursor: usize,
    pub show_disk_selector: bool,

    // Network selection
    pub selected_nic: Option<String>,
    pub network_by_nic: HashMap<String, NetworkData>,
    pub available_nics: Vec<NetworkInterface>,
    pub show_nic_selector: bool,
    pub nic_cursor: usize,

    pub proc_permission_denied: bool,
    pub processes: Vec<ProcessData>,
    pub process_table: ProcessTableState,
    pub containers: Vec<ContainerData>,
    pub container_state: ContainerBackendState,
    pub container_sort_col: ContainerSortColumn,
    pub container_sort_asc: bool,
    pub collapsed_compose_groups: HashSet<String>,

    // View navigation
    pub current_view: View,
    pub detail_process_pid: Option<u32>,
    pub detail_container_id: Option<String>,
    #[allow(dead_code)]
    pub selected_process_idx: Option<usize>,
    #[allow(dead_code)]
    pub selected_container_idx: Option<usize>,
    pub container_cursor: usize,
    pub confirm_action: Option<ConfirmAction>,
    pub logs_state: Option<LogsViewState>,
    pub docker_client: Option<Docker>,

    pub data_loaded: bool,
    pub refresh_tick: bool,
    pub show_help: bool,
    pub psi: Option<PsiData>,

    // Historial de PSI avg10 — some y full para mem e I/O
    pub psi_history_cpu: std::collections::VecDeque<f64>,
    pub psi_history_mem: std::collections::VecDeque<f64>,
    pub psi_history_mem_full: std::collections::VecDeque<f64>,
    pub psi_history_io: std::collections::VecDeque<f64>,
    pub psi_history_io_full: std::collections::VecDeque<f64>,

    // Historial de métricas
    pub metrics_history: MetricsHistory,
    pub history_mode: bool,
    pub history_range: HistoryRange,
    pub process_history: std::collections::VecDeque<ProcessHistorySample>,
    pub container_history: std::collections::VecDeque<ContainerHistorySample>,
    pub lang: crate::localization::Language,

    metrics_rx: mpsc::Receiver<AppSnapshot>,
    interval_tx: watch::Sender<f64>,
}

impl AppState {
    fn new(
        rx: mpsc::Receiver<AppSnapshot>,
        interval_tx: watch::Sender<f64>,
        initial_idx: usize,
        cfg: Config,
    ) -> Self {
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        let selected_disk = cfg.selected_disk.clone();
        let selected_nic = cfg.selected_nic.clone();
        let lang = crate::localization::Language::detect();
        Self {
            hostname,
            cpu: CpuData::default(),
            memory: MemoryData::default(),
            disks: vec![],
            interval_idx: initial_idx,
            active_tab: cfg.default_tab.clone(),
            cfg,
            selected_disk,
            selector_entries: vec![],
            disk_selector_cursor: 0,
            show_disk_selector: false,
            selected_nic,
            network_by_nic: HashMap::new(),
            available_nics: vec![],
            show_nic_selector: false,
            nic_cursor: 0,
            proc_permission_denied: false,
            processes: vec![],
            process_table: ProcessTableState::default(),
            containers: vec![],
            container_state: ContainerBackendState::default(),
            container_sort_col: ContainerSortColumn::default(),
            container_sort_asc: true,
            collapsed_compose_groups: HashSet::new(),
            current_view: View::Main,
            detail_process_pid: None,
            detail_container_id: None,
            selected_process_idx: None,
            selected_container_idx: None,
            container_cursor: 0,
            confirm_action: None,
            logs_state: None,
            docker_client: None,
            data_loaded: false,
            refresh_tick: false,
            show_help: false,
            psi: None,
            psi_history_cpu: std::collections::VecDeque::new(),
            psi_history_mem: std::collections::VecDeque::new(),
            psi_history_mem_full: std::collections::VecDeque::new(),
            psi_history_io: std::collections::VecDeque::new(),
            psi_history_io_full: std::collections::VecDeque::new(),
            metrics_history: MetricsHistory::new(),
            history_mode: false,
            history_range: HistoryRange::OneMin,
            process_history: std::collections::VecDeque::new(),
            container_history: std::collections::VecDeque::new(),
            lang,
            metrics_rx: rx,
            interval_tx,
        }
    }

    fn try_update(&mut self) {
        while let Ok(snapshot) = self.metrics_rx.try_recv() {
            self.data_loaded = true;
            self.refresh_tick = !self.refresh_tick;
            self.cpu = snapshot.cpu;
            self.memory = snapshot.memory;

            // Calcular valores de red y disco para la muestra de historial
            let (net_recv, net_sent) = snapshot
                .network_by_nic
                .values()
                .fold((0.0_f64, 0.0_f64), |(r, s), nd| {
                    (r + nd.recv_bytes_per_sec, s + nd.sent_bytes_per_sec)
                });
            let (disk_read, disk_write) = snapshot
                .disks
                .first()
                .map(|d| {
                    (
                        d.read_bytes_per_sec.unwrap_or(0.0),
                        d.write_bytes_per_sec.unwrap_or(0.0),
                    )
                })
                .unwrap_or((0.0, 0.0));
            let mem_pct = if self.memory.total_bytes > 0 {
                self.memory.used_bytes as f64 / self.memory.total_bytes as f64 * 100.0
            } else {
                0.0
            };
            self.metrics_history.push(MetricSample {
                cpu_pct: self.cpu.global_usage_pct,
                mem_pct,
                load1: self.cpu.load_avg[0],
                net_recv_bps: net_recv,
                net_sent_bps: net_sent,
                disk_read_bps: disk_read,
                disk_write_bps: disk_write,
            });

            // Update process history
            if let Some(pid) = self.detail_process_pid {
                if let Some(proc) = snapshot.processes.iter().find(|p| p.pid == pid) {
                    self.process_history.push_back(ProcessHistorySample {
                        cpu_pct: proc.cpu_pct,
                        mem_pct: proc.memory_pct,
                        memory_bytes: proc.memory_bytes,
                        disk_read_bps: proc.disk_read_per_sec.unwrap_or(0.0),
                        disk_write_bps: proc.disk_write_per_sec.unwrap_or(0.0),
                    });
                    if self.process_history.len() > 3600 {
                        self.process_history.pop_front();
                    }
                }
            } else {
                self.process_history.clear();
            }

            // Update container history
            if let Some(ref cid) = self.detail_container_id {
                if let Some(c) = snapshot.containers.iter().find(|c| &c.id == cid) {
                    self.container_history.push_back(ContainerHistorySample {
                        cpu_pct: c.cpu_pct,
                        mem_pct: c.memory_pct,
                        memory_bytes: c.memory_bytes,
                        net_recv_bps: c.net_recv_per_sec,
                        net_sent_bps: c.net_sent_per_sec,
                        disk_read_bps: c.disk_read_per_sec,
                        disk_write_bps: c.disk_write_per_sec,
                    });
                    if self.container_history.len() > 3600 {
                        self.container_history.pop_front();
                    }
                }
            } else {
                self.container_history.clear();
            }
            self.selector_entries = DiskIoCollector::build_selector_entries(&snapshot.disks);
            self.disks = snapshot.disks;
            self.network_by_nic = snapshot.network_by_nic;
            self.available_nics = snapshot.available_nics;
            self.proc_permission_denied = snapshot.proc_permission_denied;
            self.processes = snapshot.processes;
            self.containers = snapshot.containers;
            self.sort_containers();
            self.container_state = snapshot.container_state;
            self.psi = snapshot.psi.clone();
            if let Some(ref p) = snapshot.psi {
                const PSI_MAX: usize = 3600;
                if self.psi_history_cpu.len() >= PSI_MAX { self.psi_history_cpu.pop_front(); }
                if self.psi_history_mem.len() >= PSI_MAX { self.psi_history_mem.pop_front(); }
                if self.psi_history_mem_full.len() >= PSI_MAX { self.psi_history_mem_full.pop_front(); }
                if self.psi_history_io.len() >= PSI_MAX { self.psi_history_io.pop_front(); }
                if self.psi_history_io_full.len() >= PSI_MAX { self.psi_history_io_full.pop_front(); }
                self.psi_history_cpu.push_back(p.cpu_some.avg10);
                self.psi_history_mem.push_back(p.memory_some.avg10);
                self.psi_history_mem_full.push_back(p.memory_full.avg10);
                self.psi_history_io.push_back(p.io_some.avg10);
                self.psi_history_io_full.push_back(p.io_full.avg10);
            }
            if snapshot.docker_client.is_some() {
                self.docker_client = snapshot.docker_client;
            }

            // If the detailed process or container no longer exists, exit the detail view
            if self.current_view == View::ProcessDetail {
                if let Some(pid) = self.detail_process_pid {
                    if !self.processes.iter().any(|p| p.pid == pid) {
                        self.current_view = View::Main;
                        self.detail_process_pid = None;
                        self.process_history.clear();
                    }
                }
            }
            if self.current_view == View::ContainerDetail {
                if let Some(ref cid) = self.detail_container_id {
                    if !self.containers.iter().any(|c| &c.id == cid) {
                        self.current_view = View::Main;
                        self.detail_container_id = None;
                        self.container_history.clear();
                    }
                }
            }

            // By default keep selected_nic as None which means "all interfaces"
            // Only auto-select if the config had a specific NIC saved
            let _ = snapshot.suggested_nic; // unused but kept for future use
                                            // Auto-select disk on first snapshot if not previously configured
            if self.selected_disk.is_none() {
                if let Some(first) = self.disks.first() {
                    let short = first
                        .device
                        .strip_prefix("/dev/")
                        .unwrap_or(&first.device)
                        .to_string();
                    self.selected_disk = Some(short);
                }
            }
        }
    }

    pub fn t(&self, key: &'static str) -> &'static str {
        crate::localization::translate(key, self.lang)
    }

    pub fn current_network(&self) -> Option<&NetworkData> {
        self.selected_nic
            .as_ref()
            .and_then(|nic| self.network_by_nic.get(nic))
    }

    /// Returns aggregated NetworkData across all (non-loopback) NICs, or None if no data.
    pub fn current_network_total(&self) -> Option<NetworkData> {
        if self.network_by_nic.is_empty() {
            return None;
        }
        // Filter out loopback from the aggregation
        let loopback_names: std::collections::HashSet<&str> = self
            .available_nics
            .iter()
            .filter(|n| n.is_loopback)
            .map(|n| n.name.as_str())
            .collect();

        let mut total = NetworkData {
            interface: self.t("AllNICs").to_string(),
            recv_bytes_per_sec: 0.0,
            sent_bytes_per_sec: 0.0,
            total_recv_bytes: 0,
            total_sent_bytes: 0,
        };
        let mut count = 0u32;
        for (name, data) in &self.network_by_nic {
            if loopback_names.contains(name.as_str()) {
                continue;
            }
            total.recv_bytes_per_sec += data.recv_bytes_per_sec;
            total.sent_bytes_per_sec += data.sent_bytes_per_sec;
            total.total_recv_bytes += data.total_recv_bytes;
            total.total_sent_bytes += data.total_sent_bytes;
            count += 1;
        }
        if count == 0 {
            None
        } else {
            Some(total)
        }
    }

    pub fn toggle_disk_selector(&mut self) {
        self.show_disk_selector = !self.show_disk_selector;
        if !self.show_disk_selector {
            return;
        }
        if let Some(sel) = &self.cfg.selected_disk {
            self.disk_selector_cursor = self
                .selector_entries
                .iter()
                .position(|e| &e.device_short == sel)
                .unwrap_or(0);
        }
    }

    pub fn toggle_nic_selector(&mut self) {
        self.show_nic_selector = !self.show_nic_selector;
        if !self.show_nic_selector {
            return;
        }
        // Position 0 = "Todas", positions 1..N = individual NICs
        self.nic_cursor = if self.selected_nic.is_none() {
            0
        } else {
            self.available_nics
                .iter()
                .position(|n| Some(&n.name) == self.selected_nic.as_ref())
                .map(|p| p + 1) // +1 because index 0 is the "All" entry
                .unwrap_or(0)
        };
    }

    pub fn disk_selector_confirm(&mut self) {
        if let Some(entry) = self.selector_entries.get(self.disk_selector_cursor) {
            let short = entry.device_short.clone();
            self.selected_disk = Some(short.clone());
            self.cfg.selected_disk = Some(short);
            config::save_non_blocking(self.cfg.clone());
        }
        self.show_disk_selector = false;
    }

    pub fn filtered_process_count(&self) -> usize {
        let f = self.process_table.filter.to_lowercase();
        self.processes
            .iter()
            .filter(|p| {
                let name_ok = f.is_empty() || p.name.to_lowercase().contains(&f);
                let status_ok = self.process_table.status_filter.matches(p.status);
                name_ok && status_ok
            })
            .count()
    }

    pub fn process_move_cursor(&mut self, delta: i32) {
        let count = self.filtered_process_count();
        if count == 0 {
            return;
        }
        let new_cursor =
            (self.process_table.cursor as i32 + delta).clamp(0, (count as i32) - 1) as usize;
        self.process_table.cursor = new_cursor;

        // Adjust scroll to keep cursor visible
        // We need to estimate visible rows: use a fixed estimate here, UI will handle the real clamp
        let visible = 20usize; // conservative
        if new_cursor < self.process_table.scroll {
            self.process_table.scroll = new_cursor;
        } else if new_cursor >= self.process_table.scroll + visible {
            self.process_table.scroll = new_cursor.saturating_sub(visible - 1);
        }
    }

    pub fn process_sort_by(&mut self, col: ProcessSortColumn) {
        if self.process_table.sort_col == col {
            self.process_table.sort_asc = !self.process_table.sort_asc;
        } else {
            self.process_table.sort_col = col;
            self.process_table.sort_asc = false;
        }
        self.process_table.cursor = 0;
        self.process_table.scroll = 0;
    }

    pub fn sort_containers(&mut self) {
        self.containers.sort_by(|a, b| {
            let ord = match self.container_sort_col {
                ContainerSortColumn::Name => a.name.cmp(&b.name),
                ContainerSortColumn::Cpu => a
                    .cpu_pct
                    .partial_cmp(&b.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ContainerSortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
                ContainerSortColumn::NetRecv => a
                    .net_recv_per_sec
                    .partial_cmp(&b.net_recv_per_sec)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ContainerSortColumn::NetSent => a
                    .net_sent_per_sec
                    .partial_cmp(&b.net_sent_per_sec)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ContainerSortColumn::DiskRead => a
                    .disk_read_per_sec
                    .partial_cmp(&b.disk_read_per_sec)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ContainerSortColumn::DiskWrite => a
                    .disk_write_per_sec
                    .partial_cmp(&b.disk_write_per_sec)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            if self.container_sort_asc {
                ord
            } else {
                ord.reverse()
            }
        });
    }

    pub fn container_sort_by(&mut self, col: ContainerSortColumn) {
        if self.container_sort_col == col {
            self.container_sort_asc = !self.container_sort_asc;
        } else {
            self.container_sort_col = col;
            self.container_sort_asc = false;
        }
        self.sort_containers();
        self.container_cursor = 0;
    }

    pub fn container_visual_rows(&self) -> Vec<ContainerVisualRow> {
        build_container_visual_rows(&self.containers, &self.collapsed_compose_groups)
    }

    pub fn container_move_cursor(&mut self, delta: i32) {
        let rows = self.container_visual_rows();
        let count = rows.len();
        if count == 0 {
            return;
        }
        let new_cursor =
            (self.container_cursor as i32 + delta).clamp(0, (count as i32) - 1) as usize;
        self.container_cursor = new_cursor;
    }

    pub fn container_toggle_group_at_cursor(&mut self) {
        let rows = self.container_visual_rows();
        if let Some(ContainerVisualRow::GroupHeader { group_key, .. }) =
            rows.get(self.container_cursor)
        {
            let key = group_key.clone();
            if self.collapsed_compose_groups.contains(&key) {
                self.collapsed_compose_groups.remove(&key);
            } else {
                self.collapsed_compose_groups.insert(key);
            }
            // Re-clamp cursor after visibility change
            let new_rows = self.container_visual_rows();
            let count = new_rows.len();
            if count > 0 && self.container_cursor >= count {
                self.container_cursor = count - 1;
            }
        }
    }

    pub fn selected_process(&self) -> Option<&ProcessData> {
        // If a PID was pinned (user pressed Enter), always track that specific process
        if let Some(pid) = self.detail_process_pid {
            return self.processes.iter().find(|p| p.pid == pid);
        }
        // Get the filtered+sorted list the same way the table does, then index into it
        let filter_lower = self.process_table.filter.to_lowercase();
        let cursor = self.process_table.cursor;
        let mut filtered: Vec<&ProcessData> = self
            .processes
            .iter()
            .filter(|p| {
                let name_ok =
                    filter_lower.is_empty() || p.name.to_lowercase().contains(&filter_lower);
                let status_ok = self.process_table.status_filter.matches(p.status);
                name_ok && status_ok
            })
            .collect();
        filtered.sort_by(|a, b| {
            let ord = match self.process_table.sort_col {
                ProcessSortColumn::Cpu => a
                    .cpu_pct
                    .partial_cmp(&b.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
                ProcessSortColumn::DiskRead => {
                    let ar = a.disk_read_per_sec.unwrap_or(0.0);
                    let br = b.disk_read_per_sec.unwrap_or(0.0);
                    ar.partial_cmp(&br).unwrap_or(std::cmp::Ordering::Equal)
                }
                ProcessSortColumn::DiskWrite => {
                    let aw = a.disk_write_per_sec.unwrap_or(0.0);
                    let bw = b.disk_write_per_sec.unwrap_or(0.0);
                    aw.partial_cmp(&bw).unwrap_or(std::cmp::Ordering::Equal)
                }
                ProcessSortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            };
            if self.process_table.sort_asc {
                ord
            } else {
                ord.reverse()
            }
        });
        filtered.get(cursor).copied()
    }

    pub fn selected_container(&self) -> Option<&ContainerData> {
        if let Some(ref id) = self.detail_container_id {
            return self.containers.iter().find(|c| &c.id == id);
        }
        let rows = self.container_visual_rows();
        match rows.get(self.container_cursor) {
            Some(ContainerVisualRow::Container { real_idx, .. }) => {
                self.containers.get(*real_idx)
            }
            _ => None,
        }
    }

    fn step_interval(&mut self, delta: i32) {
        let new_idx =
            (self.interval_idx as i32 + delta).clamp(0, (INTERVALS.len() - 1) as i32) as usize;
        if new_idx != self.interval_idx {
            self.interval_idx = new_idx;
            let _ = self.interval_tx.send(INTERVALS[new_idx]);
            self.cfg.refresh_interval_secs = INTERVALS[new_idx];
            config::save_non_blocking(self.cfg.clone());
        }
    }
}

fn fetch_logs_blocking(docker: Docker, container_id: String) -> Vec<String> {
    use bollard::container::LogsOptions;
    use futures_util::StreamExt;

    let rt = tokio::runtime::Handle::try_current();
    let future = async move {
        let opts = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: "200".to_string(),
            ..Default::default()
        };
        let mut stream = docker.logs(&container_id, Some(opts));
        let mut lines = Vec::new();
        while let Some(Ok(msg)) = stream.next().await {
            let text = msg.to_string();
            for line in text.lines() {
                lines.push(line.to_string());
            }
        }
        lines
    };

    match rt {
        Ok(_handle) => {
            // We're in an async context — run in a blocking thread
            std::thread::spawn(move || {
                let rt2 = tokio::runtime::Runtime::new().unwrap();
                rt2.block_on(future)
            })
            .join()
            .unwrap_or_default()
        }
        Err(_) => {
            // Not in async context — create a new runtime
            let rt2 = tokio::runtime::Runtime::new().unwrap();
            rt2.block_on(future)
        }
    }
}

pub async fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let cfg = config::load();
    let initial_idx = INTERVALS
        .iter()
        .position(|&s| (s - cfg.refresh_interval_secs).abs() < f64::EPSILON)
        .unwrap_or(config::DEFAULT_INTERVAL_IDX);

    let (tx, rx) = mpsc::channel::<AppSnapshot>(8);
    let (interval_tx, mut interval_rx) = watch::channel(INTERVALS[initial_idx]);

    let shared_containers = std::sync::Arc::new(std::sync::Mutex::new((
        Vec::<ContainerData>::new(),
        ContainerBackendState::default(),
        None::<Docker>,
    )));

    // Background task for container metrics collection
    let shared_containers_clone = std::sync::Arc::clone(&shared_containers);
    tokio::spawn(async move {
        let container_collector = timeout(Duration::from_secs(1), ContainerCollector::new())
            .await
            .ok();

        let Some(mut cc) = container_collector else {
            let mut lock = shared_containers_clone.lock().unwrap();
            lock.1 = ContainerBackendState {
                available: false,
                message: Some("Docker/Podman no disponible".to_string()),
            };
            return;
        };

        // Seed initial state
        {
            let mut lock = shared_containers_clone.lock().unwrap();
            lock.1 = cc.state.clone();
            lock.2 = cc.docker_client();
        }

        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        loop {
            ticker.tick().await;

            let containers = match timeout(Duration::from_secs(3), cc.refresh()).await {
                Ok(res) => res,
                Err(_) => {
                    cc.state.available = false;
                    cc.state.message = Some("Contenedores no responden a tiempo".to_string());
                    vec![]
                }
            };

            let mut lock = shared_containers_clone.lock().unwrap();
            lock.0 = containers;
            lock.1 = cc.state.clone();
            lock.2 = cc.docker_client();
        }
    });

    // Background task for system metrics collection
    tokio::spawn(async move {
        let mut collector = SystemCollector::new();
        let mut current_secs = INTERVALS[config::DEFAULT_INTERVAL_IDX];
        let mut ticker = tokio::time::interval(Duration::from_secs_f64(current_secs));
        ticker.tick().await; // consume immediate first tick

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    collector.refresh();
                    let system = collector.snapshot();

                    // Retrieve cached container data
                    let (containers, container_state, docker_client) = {
                        let lock = shared_containers.lock().unwrap();
                        (lock.0.clone(), lock.1.clone(), lock.2.clone())
                    };

                    let snapshot = AppSnapshot {
                        cpu: system.cpu,
                        memory: system.memory,
                        disks: system.disks,
                        network_by_nic: system.network_by_nic,
                        available_nics: system.available_nics,
                        suggested_nic: system.suggested_nic,
                        proc_permission_denied: system.proc_permission_denied,
                        processes: system.processes,
                        containers,
                        container_state,
                        docker_client,
                        psi: system.psi,
                    };
                    if tx.send(snapshot).await.is_err() {
                        break;
                    }
                }
                Ok(()) = interval_rx.changed() => {
                    current_secs = *interval_rx.borrow();
                    ticker = tokio::time::interval(Duration::from_secs_f64(current_secs));
                    ticker.tick().await; // consume immediate first tick of new interval
                }
            }
        }
    });

    let mut state = AppState::new(rx, interval_tx, initial_idx, cfg);

    loop {
        state.try_update();
        terminal.draw(|f| ui::draw(f, &state))?;

        match event::poll(Duration::from_millis(250)) {
            Ok(true) => {
                let ev = event::read();
                if let Ok(Event::Key(key)) = ev {
                    match (key.code, key.modifiers) {
                        // ── Global exits ──────────────────────────────────────
                        (KeyCode::Char('q'), _)
                            if state.current_view == View::Main
                                && !state.show_nic_selector
                                && !state.show_help =>
                        {
                            break
                        }
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,

                        // ── Help modal (F1) ───────────────────────────────────
                        (KeyCode::F(1), _) => {
                            state.show_help = !state.show_help;
                        }
                        (KeyCode::Esc, _) if state.show_help => {
                            state.show_help = false;
                        }

                        // ── ContainerLogs view ────────────────────────────────
                        (KeyCode::Esc, _) if state.current_view == View::ContainerLogs => {
                            state.current_view = View::ContainerDetail;
                            state.logs_state = None;
                        }
                        (KeyCode::Char('f'), _) if state.current_view == View::ContainerLogs => {
                            if let Some(ref mut ls) = state.logs_state {
                                ls.toggle_follow();
                            }
                        }
                        (KeyCode::Up, _) if state.current_view == View::ContainerLogs => {
                            if let Some(ref mut ls) = state.logs_state {
                                ls.scroll_up();
                            }
                        }
                        (KeyCode::Down, _) if state.current_view == View::ContainerLogs => {
                            if let Some(ref mut ls) = state.logs_state {
                                ls.scroll_down(20);
                            }
                        }

                        // ── ContainerDetail view ──────────────────────────────
                        (KeyCode::Esc, _)
                            if state.current_view == View::ContainerDetail
                                && state.confirm_action.is_some() =>
                        {
                            state.confirm_action = None;
                        }
                        (KeyCode::Enter, _)
                            if state.current_view == View::ContainerDetail
                                && state.confirm_action.is_some() =>
                        {
                            if let Some(action) = state.confirm_action.take() {
                                let docker = state.docker_client.clone();
                                tokio::spawn(async move {
                                    if let Some(d) = docker {
                                        match &action {
                                            ConfirmAction::Restart(id) => {
                                                let _ = d.restart_container(id, None).await;
                                            }
                                            ConfirmAction::Stop(id) => {
                                                let _ = d.stop_container(id, None).await;
                                            }
                                        }
                                    }
                                });
                            }
                        }
                        (KeyCode::Esc, _) if state.current_view == View::ContainerDetail => {
                            state.current_view = View::Main;
                            state.detail_container_id = None;
                            state.container_history.clear();
                        }
                        (KeyCode::Char('l'), _) if state.current_view == View::ContainerDetail => {
                            if let Some(c) = state.selected_container().cloned() {
                                let mut ls = LogsViewState::new(c.id.clone(), c.name.clone());
                                // Fetch last 200 lines statically
                                if let Some(docker) = state.docker_client.clone() {
                                    let id = c.id.clone();
                                    // Fetch synchronously via blocking to avoid async complexity in event loop
                                    let lines = fetch_logs_blocking(docker, id);
                                    for line in lines {
                                        ls.lines.push(line);
                                    }
                                    ls.scroll = ls.lines.len().saturating_sub(20);
                                }
                                state.logs_state = Some(ls);
                                state.current_view = View::ContainerLogs;
                            }
                        }
                        (KeyCode::Char('r'), _) if state.current_view == View::ContainerDetail => {
                            if let Some(c) = state.selected_container() {
                                state.confirm_action = Some(ConfirmAction::Restart(c.id.clone()));
                            }
                        }
                        (KeyCode::Char('s'), _) if state.current_view == View::ContainerDetail => {
                            if let Some(c) = state.selected_container() {
                                state.confirm_action = Some(ConfirmAction::Stop(c.id.clone()));
                            }
                        }

                        // ── ProcessDetail view ────────────────────────────────
                        (KeyCode::Esc, _) if state.current_view == View::ProcessDetail => {
                            state.current_view = View::Main;
                            state.detail_process_pid = None;
                            state.process_history.clear();
                        }

                        // ── Main view ─────────────────────────────────────────
                        (KeyCode::Tab, _) if state.current_view == View::Main => {
                            state.active_tab = match state.active_tab {
                                Tab::Processes => Tab::Containers,
                                Tab::Containers => Tab::Processes,
                                Tab::Network => Tab::Processes,
                            };
                            state.cfg.default_tab = state.active_tab.clone();
                            config::save_non_blocking(state.cfg.clone());
                        }
                        (KeyCode::F(3), _) if state.current_view == View::Main => {
                            state.toggle_nic_selector();
                        }
                        (KeyCode::Up, _) if state.show_nic_selector => {
                            if state.nic_cursor > 0 {
                                state.nic_cursor -= 1;
                            }
                        }
                        (KeyCode::Down, _) if state.show_nic_selector => {
                            let max = state.available_nics.len();
                            if state.nic_cursor < max {
                                state.nic_cursor += 1;
                            }
                        }
                        (KeyCode::Enter, _) if state.show_nic_selector => {
                            if state.nic_cursor == 0 {
                                state.selected_nic = None;
                                state.cfg.selected_nic = None;
                                config::save_non_blocking(state.cfg.clone());
                            } else if let Some(nic) = state.available_nics.get(state.nic_cursor - 1)
                            {
                                if nic.is_up {
                                    let name = nic.name.clone();
                                    state.selected_nic = Some(name.clone());
                                    state.cfg.selected_nic = Some(name);
                                    config::save_non_blocking(state.cfg.clone());
                                }
                            }
                            state.show_nic_selector = false;
                        }
                        (KeyCode::Esc, _) if state.show_nic_selector => {
                            state.show_nic_selector = false;
                        }
                        (KeyCode::F(2), _) if state.current_view == View::Main => {
                            state.toggle_disk_selector();
                        }
                        (KeyCode::Up, _) if state.show_disk_selector => {
                            if state.disk_selector_cursor > 0 {
                                state.disk_selector_cursor -= 1;
                            }
                        }
                        (KeyCode::Down, _) if state.show_disk_selector => {
                            let max = state.selector_entries.len().saturating_sub(1);
                            if state.disk_selector_cursor < max {
                                state.disk_selector_cursor += 1;
                            }
                        }
                        (KeyCode::Enter, _) if state.show_disk_selector => {
                            state.disk_selector_confirm();
                        }
                        (KeyCode::Esc, _) if state.show_disk_selector => {
                            state.show_disk_selector = false;
                        }
                        (KeyCode::Char('['), _)
                            if state.current_view == View::Main
                                && !state.show_nic_selector
                                && !state.show_disk_selector
                                && !state.process_table.filter_active =>
                        {
                            state.step_interval(-1);
                        }
                        (KeyCode::Char(']'), _)
                            if state.current_view == View::Main
                                && !state.show_nic_selector
                                && !state.show_disk_selector
                                && !state.process_table.filter_active =>
                        {
                            state.step_interval(1);
                        }
                        // Toggle historial de métricas
                        (KeyCode::Char('h'), _)
                            if (state.current_view == View::Main
                                || state.current_view == View::ProcessDetail
                                || state.current_view == View::ContainerDetail)
                                && !state.process_table.filter_active =>
                        {
                            state.history_mode = !state.history_mode;
                        }
                        // Ciclar rango de tiempo del historial
                        (KeyCode::Char('t'), _)
                            if (state.current_view == View::Main
                                || state.current_view == View::ProcessDetail
                                || state.current_view == View::ContainerDetail)
                                && !state.process_table.filter_active =>
                        {
                            state.history_range = state.history_range.next();
                        }
                        // Process table: filter mode input
                        (KeyCode::Char(ch), _)
                            if state.process_table.filter_active
                                && state.active_tab == Tab::Processes =>
                        {
                            state.process_table.filter.push(ch);
                            state.process_table.cursor = 0;
                            state.process_table.scroll = 0;
                        }
                        (KeyCode::Backspace, _)
                            if state.process_table.filter_active
                                && state.active_tab == Tab::Processes =>
                        {
                            state.process_table.filter.pop();
                            state.process_table.cursor = 0;
                            state.process_table.scroll = 0;
                        }
                        (KeyCode::Esc, _)
                            if state.process_table.filter_active
                                && state.active_tab == Tab::Processes =>
                        {
                            state.process_table.filter_active = false;
                        }
                        (KeyCode::Enter, _)
                            if state.process_table.filter_active
                                && state.active_tab == Tab::Processes =>
                        {
                            state.process_table.filter_active = false;
                        }
                        (KeyCode::Esc, _)
                            if !state.process_table.filter.is_empty()
                                && state.active_tab == Tab::Processes =>
                        {
                            state.process_table.filter.clear();
                            state.process_table.cursor = 0;
                            state.process_table.scroll = 0;
                        }
                        // Process table: navigate to detail on Enter — pin the PID so the detail
                        // view always tracks the same process even when the list re-sorts.
                        (KeyCode::Enter, _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            let new_pid = state.selected_process().map(|p| p.pid);
                            if state.detail_process_pid != new_pid {
                                state.process_history.clear();
                            }
                            state.detail_process_pid = new_pid;
                            state.current_view = View::ProcessDetail;
                        }
                        // Container table: Space toggles group collapse
                        (KeyCode::Char(' '), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_toggle_group_at_cursor();
                        }
                        // Container table: navigate to detail on Enter
                        (KeyCode::Enter, _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            if !state.containers.is_empty() {
                                let new_cid = state.selected_container().map(|c| c.id.clone());
                                if state.detail_container_id != new_cid {
                                    state.container_history.clear();
                                }
                                state.detail_container_id = new_cid;
                                state.current_view = View::ContainerDetail;
                            }
                        }
                        // Process table: activate filter
                        (KeyCode::Char('/'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.show_nic_selector
                                && !state.show_disk_selector =>
                        {
                            state.process_table.filter_active = true;
                        }
                        (KeyCode::Char('c'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            state.process_sort_by(ProcessSortColumn::Cpu);
                        }
                        (KeyCode::Char('m'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            state.process_sort_by(ProcessSortColumn::Memory);
                        }
                        (KeyCode::Char('n'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            state.process_sort_by(ProcessSortColumn::Name);
                        }
                        (KeyCode::Char('r'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            state.process_sort_by(ProcessSortColumn::DiskRead);
                        }
                        (KeyCode::Char('w'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            state.process_sort_by(ProcessSortColumn::DiskWrite);
                        }
                        (KeyCode::Char('f'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.process_table.filter_active =>
                        {
                            state.process_table.status_filter =
                                state.process_table.status_filter.next();
                            state.process_table.cursor = 0;
                            state.process_table.scroll = 0;
                        }
                        // Container table: sort keys
                        (KeyCode::Char('c'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_sort_by(ContainerSortColumn::Cpu);
                        }
                        (KeyCode::Char('m'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_sort_by(ContainerSortColumn::Memory);
                        }
                        (KeyCode::Char('i'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_sort_by(ContainerSortColumn::NetRecv);
                        }
                        (KeyCode::Char('o'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_sort_by(ContainerSortColumn::NetSent);
                        }
                        (KeyCode::Char('r'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_sort_by(ContainerSortColumn::DiskRead);
                        }
                        (KeyCode::Char('w'), _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_sort_by(ContainerSortColumn::DiskWrite);
                        }
                        // Process table: navigation
                        (KeyCode::Up, _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.show_nic_selector
                                && !state.show_disk_selector =>
                        {
                            state.process_move_cursor(-1);
                        }
                        (KeyCode::Down, _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Processes
                                && !state.show_nic_selector
                                && !state.show_disk_selector =>
                        {
                            state.process_move_cursor(1);
                        }
                        // Container table: navigation
                        (KeyCode::Up, _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_move_cursor(-1);
                        }
                        (KeyCode::Down, _)
                            if state.current_view == View::Main
                                && state.active_tab == Tab::Containers =>
                        {
                            state.container_move_cursor(1);
                        }
                        _ => {}
                    }
                } else if let Ok(Event::Mouse(mouse)) = ev {
                    match mouse.kind {
                        crossterm::event::MouseEventKind::ScrollUp => {
                            terminal.clear()?;
                            if state.show_nic_selector {
                                if state.nic_cursor > 0 {
                                    state.nic_cursor -= 1;
                                }
                            } else if state.show_disk_selector {
                                if state.disk_selector_cursor > 0 {
                                    state.disk_selector_cursor -= 1;
                                }
                            } else if state.current_view == View::Main {
                                match state.active_tab {
                                    Tab::Processes => state.process_move_cursor(-1),
                                    Tab::Containers => state.container_move_cursor(-1),
                                    _ => {}
                                }
                            } else if state.current_view == View::ContainerLogs {
                                if let Some(ref mut ls) = state.logs_state {
                                    ls.scroll_up();
                                }
                            }
                        }
                        crossterm::event::MouseEventKind::ScrollDown => {
                            terminal.clear()?;
                            if state.show_nic_selector {
                                let max = state.available_nics.len();
                                if state.nic_cursor < max {
                                    state.nic_cursor += 1;
                                }
                            } else if state.show_disk_selector {
                                let max = state.selector_entries.len().saturating_sub(1);
                                if state.disk_selector_cursor < max {
                                    state.disk_selector_cursor += 1;
                                }
                            } else if state.current_view == View::Main {
                                match state.active_tab {
                                    Tab::Processes => state.process_move_cursor(1),
                                    Tab::Containers => state.container_move_cursor(1),
                                    _ => {}
                                }
                            } else if state.current_view == View::ContainerLogs {
                                if let Some(ref mut ls) = state.logs_state {
                                    ls.scroll_down(20);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(false) => {}
            Err(_) => {
                // If terminal input is unavailable, keep the dashboard alive instead of exiting.
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum ContainerVisualRow {
    GroupHeader {
        group_key: String,
        label: String,
        count: usize,
        collapsed: bool,
        cpu_sum: f64,
        mem_sum: u64,
    },
    Container {
        real_idx: usize,
    },
}

pub fn build_container_visual_rows(
    containers: &[ContainerData],
    collapsed: &HashSet<String>,
) -> Vec<ContainerVisualRow> {
    use std::collections::BTreeMap;

    // Group containers: keyed by compose project or "" for standalone
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, c) in containers.iter().enumerate() {
        let key = c.compose_project.clone().unwrap_or_default();
        groups.entry(key).or_default().push(idx);
    }

    // Build rows: standalone containers (key "") come first without a header
    let mut rows = Vec::new();

    // Standalone (no compose project) — emit directly without group header
    if let Some(indices) = groups.get("") {
        for &idx in indices {
            rows.push(ContainerVisualRow::Container { real_idx: idx });
        }
    }

    // Compose stacks — sorted alphabetically
    for (key, indices) in &groups {
        if key.is_empty() {
            continue;
        }
        let cpu_sum: f64 = indices.iter().map(|&i| containers[i].cpu_pct).sum();
        let mem_sum: u64 = indices.iter().map(|&i| containers[i].memory_bytes).sum();
        let is_collapsed = collapsed.contains(key);
        rows.push(ContainerVisualRow::GroupHeader {
            group_key: key.clone(),
            label: key.clone(),
            count: indices.len(),
            collapsed: is_collapsed,
            cpu_sum,
            mem_sum,
        });
        if !is_collapsed {
            for &idx in indices {
                rows.push(ContainerVisualRow::Container { real_idx: idx });
            }
        }
    }

    rows
}
