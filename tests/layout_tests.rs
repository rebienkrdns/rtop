use ratatui::{
    backend::TestBackend,
    Terminal,
};

fn make_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).unwrap()
}

#[test]
fn layout_no_panic_80x24() {
    let mut terminal = make_terminal(80, 24);
    terminal.draw(|f| {
        let area = f.size();
        assert_eq!(area.width, 80);
        assert_eq!(area.height, 24);
        // El layout principal debería caber exactamente en 80x24
        // Solo verificamos que el cálculo de Rect no produce overflow
        use ratatui::{layout::{Constraint, Direction, Layout}, widgets::{Block, Borders}};
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
        assert!(vertical[3].height >= 1, "área de contenido demasiado pequeña en 80x24");
        f.render_widget(Block::default().borders(Borders::ALL), area);
    }).unwrap();
}

#[test]
fn layout_no_panic_120x35() {
    let mut terminal = make_terminal(120, 35);
    terminal.draw(|f| {
        let area = f.size();
        use ratatui::{layout::{Constraint, Direction, Layout}, widgets::{Block, Borders}};
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
        f.render_widget(Block::default().borders(Borders::ALL), area);
    }).unwrap();
}

#[test]
fn layout_no_panic_200x50() {
    let mut terminal = make_terminal(200, 50);
    terminal.draw(|f| {
        let area = f.size();
        use ratatui::{layout::{Constraint, Direction, Layout}, widgets::{Block, Borders}};
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
        f.render_widget(Block::default().borders(Borders::ALL), area);
    }).unwrap();
}

#[test]
fn small_terminal_below_minimum() {
    // Terminales debajo del mínimo 80x24 no deben causar panic
    let mut terminal = make_terminal(60, 20);
    terminal.draw(|f| {
        let area = f.size();
        // Simulamos la misma detección que hace ui::draw
        if area.width < 80 || area.height < 24 {
            use ratatui::{
                layout::Alignment,
                style::{Color, Modifier, Style},
                text::{Line, Span},
                widgets::{Block, Borders, Paragraph},
            };
            let msg = Paragraph::new(Line::from(vec![
                Span::styled(
                    "⚠  Terminal muy pequeña (mínimo 80×24)",
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ]))
            .block(Block::default().borders(Borders::ALL).title(" rtop "))
            .alignment(Alignment::Center);
            f.render_widget(msg, area);
        }
    }).unwrap();
}
