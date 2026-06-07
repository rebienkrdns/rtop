use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use sysinfo::System;
use tokio::sync::{mpsc, watch};

use crate::collectors::system::{SystemCollector, SystemSnapshot};
use crate::config::{self, Config, INTERVALS};
use crate::models::{CpuData, DiskData, MemoryData};
use crate::ui;

pub struct AppState {
    pub hostname: String,
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    pub interval_idx: usize,
    metrics_rx: mpsc::Receiver<SystemSnapshot>,
    interval_tx: watch::Sender<f64>,
}

impl AppState {
    fn new(
        rx: mpsc::Receiver<SystemSnapshot>,
        interval_tx: watch::Sender<f64>,
        initial_idx: usize,
    ) -> Self {
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        Self {
            hostname,
            cpu: CpuData::default(),
            memory: MemoryData::default(),
            disks: vec![],
            interval_idx: initial_idx,
            metrics_rx: rx,
            interval_tx,
        }
    }

    fn try_update(&mut self) {
        while let Ok(snapshot) = self.metrics_rx.try_recv() {
            self.cpu = snapshot.cpu;
            self.memory = snapshot.memory;
            self.disks = snapshot.disks;
        }
    }

    fn step_interval(&mut self, delta: i32) {
        let new_idx = (self.interval_idx as i32 + delta)
            .clamp(0, (INTERVALS.len() - 1) as i32) as usize;
        if new_idx != self.interval_idx {
            self.interval_idx = new_idx;
            let _ = self.interval_tx.send(INTERVALS[new_idx]);
            let cfg = Config {
                refresh_interval_secs: INTERVALS[new_idx],
            };
            let _ = config::save(&cfg);
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

    let mut state = AppState::new(rx, interval_tx, initial_idx);

    loop {
        state.try_update();
        terminal.draw(|f| ui::draw(f, &state))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) => break,
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    (KeyCode::Char('['), _) => state.step_interval(-1),
                    (KeyCode::Char(']'), _) => state.step_interval(1),
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
