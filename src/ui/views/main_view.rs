use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn draw(f: &mut Frame, state: &AppState) {
    let theme = Theme::default_theme();
    let area = f.size();

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(area);

    let header_area = vertical[0];
    let middle_area = vertical[1];
    let bottom_area = vertical[2];
    let footer_area = vertical[3];

    let middle_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(middle_area);

    let left_area = middle_cols[0];
    let right_area = middle_cols[1];

    // Header
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let header_text = Line::from(vec![
        Span::styled(" rtop ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(theme.muted)),
        Span::styled(&state.hostname, Style::default().fg(theme.text)),
        Span::styled("  ", Style::default()),
        Span::styled(now, Style::default().fg(theme.muted)),
    ]);
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.accent)));
    f.render_widget(header, header_area);

    // Panel izquierdo: CPU, RAM, Disco
    let left = Block::default()
        .title(Span::styled(" CPU · RAM · Disco ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    f.render_widget(left, left_area);

    // Panel derecho: Red
    let right = Block::default()
        .title(Span::styled(" Red ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    f.render_widget(right, right_area);

    // Panel inferior: Procesos / Contenedores
    let bottom = Block::default()
        .title(Span::styled(" Procesos · Contenedores ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    f.render_widget(bottom, bottom_area);

    // Footer con atajos
    let footer_text = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Salir  ", Style::default().fg(theme.muted)),
        Span::styled("[↑↓] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Navegar  ", Style::default().fg(theme.muted)),
        Span::styled("[Tab] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Cambiar panel  ", Style::default().fg(theme.muted)),
        Span::styled("[c] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Contenedores", Style::default().fg(theme.muted)),
    ]);
    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.muted)));
    f.render_widget(footer, footer_area);
}
