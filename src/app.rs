use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use sysinfo::System;
use tokio::sync::mpsc;

use crate::collectors::system::{SystemCollector, SystemSnapshot};
use crate::models::{CpuData, DiskData, MemoryData};
use crate::ui;

pub struct AppState {
    pub hostname: String,
    pub cpu: CpuData,
    pub memory: MemoryData,
    pub disks: Vec<DiskData>,
    metrics_rx: mpsc::Receiver<SystemSnapshot>,
}

impl AppState {
    fn new(rx: mpsc::Receiver<SystemSnapshot>) -> Self {
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        Self {
            hostname,
            cpu: CpuData::default(),
            memory: MemoryData::default(),
            disks: vec![],
            metrics_rx: rx,
        }
    }

    fn try_update(&mut self) {
        while let Ok(snapshot) = self.metrics_rx.try_recv() {
            self.cpu = snapshot.cpu;
            self.memory = snapshot.memory;
            self.disks = snapshot.disks;
        }
    }
}

pub async fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let (tx, rx) = mpsc::channel::<SystemSnapshot>(8);

    tokio::spawn(async move {
        let mut collector = SystemCollector::new();
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            collector.refresh();
            let snapshot = collector.snapshot();
            if tx.send(snapshot).await.is_err() {
                break;
            }
        }
    });

    let mut state = AppState::new(rx);

    loop {
        state.try_update();
        terminal.draw(|f| ui::draw(f, &state))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) => break,
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
