use std::collections::HashMap;
use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use sysinfo::System;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

use crate::collectors::containers::{ContainerBackendState, ContainerCollector};
use crate::collectors::disk::{DiskIoCollector, DiskSelectorEntry};
use crate::collectors::system::SystemCollector;
use crate::config::{self, Config, INTERVALS, Tab};
use crate::models::{ContainerData, CpuData, DiskData, MemoryData, NetworkData, NetworkInterface, ProcessData};
use crate::ui;

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
    pub containers: Vec<ContainerData>,
    pub container_state: ContainerBackendState,

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
            containers: vec![],
            container_state: ContainerBackendState::default(),
            metrics_rx: rx,
            interval_tx,
        }
    }

    fn try_update(&mut self) {
        while let Ok(snapshot) = self.metrics_rx.try_recv() {
            self.cpu = snapshot.cpu;
            self.memory = snapshot.memory;
            self.selector_entries = DiskIoCollector::build_selector_entries(&snapshot.disks);
            self.disks = snapshot.disks;
            self.network_by_nic = snapshot.network_by_nic;
            self.available_nics = snapshot.available_nics;
            self.proc_permission_denied = snapshot.proc_permission_denied;
            self.processes = snapshot.processes;
            self.containers = snapshot.containers;
            self.container_state = snapshot.container_state;

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
            interface: "todas".to_string(),
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

    fn step_interval(&mut self, delta: i32) {
        let new_idx = (self.interval_idx as i32 + delta)
            .clamp(0, (INTERVALS.len() - 1) as i32) as usize;
        if new_idx != self.interval_idx {
            self.interval_idx = new_idx;
            let _ = self.interval_tx.send(INTERVALS[new_idx]);
            self.cfg.refresh_interval_secs = INTERVALS[new_idx];
            config::save_non_blocking(self.cfg.clone());
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

    tokio::spawn(async move {
        let mut collector = SystemCollector::new();
        let mut container_collector = timeout(Duration::from_secs(1), ContainerCollector::new())
            .await
            .ok();
        let mut current_secs = INTERVALS[config::DEFAULT_INTERVAL_IDX];
        let mut ticker = tokio::time::interval(Duration::from_secs_f64(current_secs));
        ticker.tick().await; // consume immediate first tick

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    collector.refresh();
                    let system = collector.snapshot();
                    let (containers, container_state) = if let Some(ref mut cc) = container_collector {
                        match timeout(Duration::from_millis(800), cc.refresh()).await {
                            Ok(containers) => (containers, cc.state.clone()),
                            Err(_) => {
                                cc.state.available = false;
                                cc.state.message = Some("Contenedores no responden a tiempo".to_string());
                                (vec![], cc.state.clone())
                            }
                        }
                    } else {
                        (
                            vec![],
                            ContainerBackendState {
                                available: false,
                                message: Some("Docker/Podman no disponible".to_string()),
                            },
                        )
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
                if let Ok(Event::Key(key)) = event::read() {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('q'), _) if !state.show_nic_selector => break,
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                        (KeyCode::Tab, _) => {
                            state.active_tab = match state.active_tab {
                                Tab::Processes => Tab::Containers,
                                Tab::Containers => Tab::Processes,
                                Tab::Network => Tab::Processes,
                            };
                            state.cfg.default_tab = state.active_tab.clone();
                            config::save_non_blocking(state.cfg.clone());
                        }
                        (KeyCode::F(3), _) => {
                            state.toggle_nic_selector();
                        }
                        (KeyCode::Up, _) if state.show_nic_selector => {
                            if state.nic_cursor > 0 {
                                state.nic_cursor -= 1;
                            }
                        }
                        (KeyCode::Down, _) if state.show_nic_selector => {
                            // +1 because position 0 is "Todas"
                            let max = state.available_nics.len(); // len() not saturating_sub because 0 = All
                            if state.nic_cursor < max {
                                state.nic_cursor += 1;
                            }
                        }
                        (KeyCode::Enter, _) if state.show_nic_selector => {
                            if state.nic_cursor == 0 {
                                // "Todas las interfaces"
                                state.selected_nic = None;
                                state.cfg.selected_nic = None;
                                config::save_non_blocking(state.cfg.clone());
                            } else if let Some(nic) = state.available_nics.get(state.nic_cursor - 1) {
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
                        (KeyCode::F(2), _) => {
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
                        (KeyCode::Char('['), _) if !state.show_nic_selector && !state.show_disk_selector => {
                            state.step_interval(-1);
                        }
                        (KeyCode::Char(']'), _) if !state.show_nic_selector && !state.show_disk_selector => {
                            state.step_interval(1);
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
