use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{backend::CrosstermBackend, Terminal};
use sysinfo::System;

use crate::ui;

pub struct AppState {
    pub hostname: String,
}

impl AppState {
    fn new() -> Self {
        let hostname = System::host_name().unwrap_or_else(|| "unknown".to_string());
        Self { hostname }
    }
}

pub async fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let state = AppState::new();

    loop {
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
