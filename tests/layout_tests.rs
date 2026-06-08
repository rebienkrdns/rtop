use ratatui::{backend::TestBackend, Terminal};

fn make_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).unwrap()
}

#[test]
fn layout_no_panic_80x24() {
    let mut terminal = make_terminal(80, 24);
    terminal
        .draw(|f| {
            let area = f.size();
            assert_eq!(area.width, 80);
            assert_eq!(area.height, 24);
            // El layout principal debería caber exactamente en 80x24
            // Solo verificamos que el cálculo de Rect no produce overflow
            use ratatui::{
                layout::{Constraint, Direction, Layout},
                widgets::{Block, Borders},
            };
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(13),
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);
            // El contenido mínimo (Min(5)) no debe ser negativo
            assert!(
                vertical[3].height >= 1,
                "área de contenido demasiado pequeña en 80x24"
            );

            // Test compact horizontal split (width < 120)
            let metrics_area = vertical[1];
            let metrics_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(metrics_area);

            // Columna 2: Red y PSI
            let col2_block = Block::default().borders(Borders::ALL);
            let col2_inner = col2_block.inner(metrics_cols[1]);
            let col2_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4), // Red
                    Constraint::Length(1), // spacer
                    Constraint::Min(0),    // PSI
                ])
                .split(col2_inner);
            assert!(col2_layout[2].height >= 1);

            f.render_widget(Block::default().borders(Borders::ALL), area);
        })
        .unwrap();
}

#[test]
fn layout_no_panic_120x35() {
    let mut terminal = make_terminal(120, 35);
    terminal
        .draw(|f| {
            let area = f.size();
            use ratatui::{
                layout::{Constraint, Direction, Layout},
                widgets::{Block, Borders},
            };
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(13),
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);
            assert!(vertical[3].height >= 11);

            // Test wide horizontal split (width >= 120)
            let metrics_area = vertical[1];
            let metrics_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(30),
                    Constraint::Percentage(35),
                    Constraint::Percentage(35),
                ])
                .split(metrics_area);

            // Columna 1: CPU y RAM
            let col1_block = Block::default().borders(Borders::ALL);
            let col1_inner = col1_block.inner(metrics_cols[0]);
            let col1_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Length(2),
                    Constraint::Min(0),
                ])
                .split(col1_inner);
            assert!(col1_layout[2].height >= 1);

            // Columna 2: Disco y Red
            let col2_block = Block::default().borders(Borders::ALL);
            let col2_inner = col2_block.inner(metrics_cols[1]);
            let col2_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(col2_inner);
            assert!(col2_layout[2].height >= 1);

            f.render_widget(Block::default().borders(Borders::ALL), area);
        })
        .unwrap();
}

#[test]
fn layout_no_panic_200x50() {
    let mut terminal = make_terminal(200, 50);
    terminal
        .draw(|f| {
            let area = f.size();
            use ratatui::{
                layout::{Constraint, Direction, Layout},
                widgets::{Block, Borders},
            };
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(13),
                    Constraint::Length(3),
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);
            assert!(vertical[3].height >= 26);

            // Test wide horizontal split (width >= 120)
            let metrics_area = vertical[1];
            let metrics_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(30),
                    Constraint::Percentage(35),
                    Constraint::Percentage(35),
                ])
                .split(metrics_area);

            // Columna 1
            let col1_block = Block::default().borders(Borders::ALL);
            let col1_inner = col1_block.inner(metrics_cols[0]);
            let col1_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(2),
                    Constraint::Length(1),
                    Constraint::Length(2),
                    Constraint::Min(0),
                ])
                .split(col1_inner);
            assert!(col1_layout[2].height >= 1);

            f.render_widget(Block::default().borders(Borders::ALL), area);
        })
        .unwrap();
}

#[test]
fn small_terminal_below_minimum() {
    // Terminales debajo del mínimo 80x24 no deben causar panic
    let mut terminal = make_terminal(60, 20);
    terminal
        .draw(|f| {
            let area = f.size();
            // Simulamos la misma detección que hace ui::draw
            if area.width < 80 || area.height < 24 {
                use ratatui::{
                    layout::Alignment,
                    style::{Color, Modifier, Style},
                    text::{Line, Span},
                    widgets::{Block, Borders, Paragraph},
                };
                let msg = Paragraph::new(Line::from(vec![Span::styled(
                    "⚠  Terminal muy pequeña (mínimo 80×24)",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]))
                .block(Block::default().borders(Borders::ALL).title(" rtop "))
                .alignment(Alignment::Center);
                f.render_widget(msg, area);
            }
        })
        .unwrap();
}
