mod app;
mod collectors;
mod config;
mod models;
mod ui;
mod localization;

use std::io::Stdout;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let lang = localization::Language::detect();
    if args.len() > 1 {
        match args[1].as_str() {
            "-v" | "--version" | "-V" => {
                println!("rtop v{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-h" | "--help" => {
                if lang == localization::Language::Spanish {
                    println!("rtop - Un monitor de recursos del sistema moderno en Rust");
                    println!();
                    println!("Uso: rtop [OPCIONES]");
                    println!();
                    println!("Opciones:");
                    println!("  -h, --help     Muestra este mensaje de ayuda");
                    println!("  -v, --version  Muestra la versión");
                } else {
                    println!("rtop - A modern TUI system resource monitor in Rust");
                    println!();
                    println!("Usage: rtop [OPTIONS]");
                    println!();
                    println!("Options:");
                    println!("  -h, --help     Show this help message");
                    println!("  -v, --version  Show the version");
                }
                return Ok(());
            }
            _ => {
                if lang == localization::Language::Spanish {
                    eprintln!("Error: Argumento no reconocido '{}'", args[1]);
                    eprintln!("Usa 'rtop --help' para ver las opciones.");
                } else {
                    eprintln!("Error: Unrecognized argument '{}'", args[1]);
                    eprintln!("Use 'rtop --help' to view options.");
                }
                std::process::exit(1);
            }
        }
    }

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let mut terminal = setup_terminal()?;
    let result = app::run(&mut terminal).await;
    restore_terminal(&mut terminal)?;
    result
}
