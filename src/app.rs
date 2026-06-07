use std::collections::HashMap;
use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use sysinfo::System;
use tokio::sync::{mpsc, watch};

use crate::collectors::disk::{DiskIoCollector, DiskSelectorEntry};
use crate::collectors::system::{SystemCollector, SystemSnapshot};
use crate::config::{self, Config, INTERVALS};
use crate::models::{CpuData, DiskData, MemoryData, NetworkData, NetworkInterface};
use crate::ui;

pub struct AppState {
    pub hostname: String,
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    pub interval_idx: usize,
    pub cfg: Config,
    pub selected_nic: Option<String>,
    pub network_by_nic: HashMap<String, NetworkData>,
    pub available_nics: Vec<NetworkInterface>,
    pub show_nic_selector: bool,
    pub nic_cursor: usize,
    pub show_disk_selector: bool,
    pub disk_selector_cursor: usize,
    pub selector_entries: Vec<DiskSelectorEntry>,
    metrics_rx: mpsc::Receiver<SystemSnapshot>,
    interval_tx: watch::Sender<f64>,
}

impl AppState {
    fn new(
        rx: mpsc::Receiver<SystemSnapshot>,
        interval_tx: watch::Sender<f64>,
        initial_idx: usize,
        cfg: Config,
    ) -> Self {
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        Self {
            hostname,
            cpu: CpuData::default(),
            memory: MemoryData::default(),
            disks: vec![],
            interval_idx: initial_idx,
            cfg,
            selected_nic: None,
            network_by_nic: HashMap::new(),
            available_nics: vec![],
            show_nic_selector: false,
            nic_cursor: 0,
            show_disk_selector: false,
            disk_selector_cursor: 0,
            selector_entries: vec![],
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
            if self.selected_nic.is_none() {
                self.selected_nic = snapshot.suggested_nic;
            }
            if self.cfg.selected_disk.is_none() {
                if let Some(first) = self.disks.first() {
                    self.cfg.selected_disk = Some(
                        first.device.strip_prefix("/dev/").unwrap_or(&first.device).to_string(),
                    );
                }
            }
        }
    }

    pub fn current_network(&self) -> Option<&NetworkData> {
        self.selected_nic
            .as_ref()
            .and_then(|nic| self.network_by_nic.get(nic))
    }

    pub fn open_disk_selector(&mut self) {
        self.show_disk_selector = true;
        if let Some(sel) = &self.cfg.selected_disk {
            self.disk_selector_cursor = self
                .selector_entries
                .iter()
                .position(|e| &e.device_short == sel)
                .unwrap_or(0);
        }
    }

    pub fn disk_selector_confirm(&mut self) {
        if let Some(entry) = self.selector_entries.get(self.disk_selector_cursor) {
            self.cfg.selected_disk = Some(entry.device_short.clone());
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

    let (tx, rx) = mpsc::channel::<SystemSnapshot>(8);
    let (interval_tx, mut interval_rx) = watch::channel(INTERVALS[initial_idx]);

    tokio::spawn(async move {
        let mut collector = SystemCollector::new();
        let mut current_secs = INTERVALS[config::DEFAULT_INTERVAL_IDX];
        let mut ticker = tokio::time::interval(Duration::from_secs_f64(current_secs));
        ticker.tick().await; // consume immediate first tick

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    collector.refresh();
                    let snapshot = collector.snapshot();
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

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) if !state.show_nic_selector => break,
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::F(3), _) => {
                        state.show_nic_selector = !state.show_nic_selector;
                        if state.show_nic_selector {
                            state.nic_cursor = state
                                .available_nics
                                .iter()
                                .position(|n| Some(&n.name) == state.selected_nic.as_ref())
                                .unwrap_or(0);
                        }
                    }
                    (KeyCode::Up, _) if state.show_nic_selector => {
                        if state.nic_cursor > 0 {
                            state.nic_cursor -= 1;
                        }
                    }
                    (KeyCode::Down, _) if state.show_nic_selector => {
                        let max = state.available_nics.len().saturating_sub(1);
                        if state.nic_cursor < max {
                            state.nic_cursor += 1;
                        }
                    }
                    (KeyCode::Enter, _) if state.show_nic_selector => {
                        if let Some(nic) = state.available_nics.get(state.nic_cursor) {
                            if nic.is_up {
                                state.selected_nic = Some(nic.name.clone());
                            }
                        }
                        state.show_nic_selector = false;
                    }
                    (KeyCode::Esc, _) if state.show_nic_selector => {
                        state.show_nic_selector = false;
                    }
                    (KeyCode::F(2), _) if !state.show_nic_selector && !state.show_disk_selector => {
                        state.open_disk_selector();
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
    }

    Ok(())
}
