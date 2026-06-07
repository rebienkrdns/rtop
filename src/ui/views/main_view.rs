use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::config::{interval_label, INTERVALS};
use crate::ui::theme::Theme;
use crate::ui::widgets::{cpu_bar, disk_bar, memory_bar};

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

    // Header con control de intervalo
    let now = Local::now().format("%H:%M:%S").to_string();
    let idx = state.interval_idx;
    let left_arrow = if idx > 0 { "◀ " } else { "  " };
    let right_arrow = if idx < INTERVALS.len() - 1 { " ▶" } else { "  " };
    let label = interval_label(idx);
    let interval_ctrl = format!("[ {}{}{} ]", left_arrow, label, right_arrow);

    let header_text = Line::from(vec![
        Span::styled(
            " rtop ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme.muted)),
        Span::styled(state.hostname.as_str(), Style::default().fg(theme.text)),
        Span::styled("    Refresco: ", Style::default().fg(theme.muted)),
        Span::styled(
            interval_ctrl,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("    ", Style::default()),
        Span::styled(now.as_str(), Style::default().fg(theme.muted)),
    ]);
    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent)),
    );
    f.render_widget(header, header_area);

    // Panel izquierdo: CPU, RAM, Disco
    let left_block = Block::default()
        .title(Span::styled(
            " CPU · RAM · Disco ",
            Style::default().fg(theme.accent),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    let left_inner = left_block.inner(left_area);
    f.render_widget(left_block, left_area);
    draw_metrics(f, left_inner, state);

    // Panel derecho: Red
    let right = Block::default()
        .title(Span::styled(" Red ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    f.render_widget(right, right_area);

    // Panel inferior: Procesos / Contenedores
    let bottom = Block::default()
        .title(Span::styled(
            " Procesos · Contenedores ",
            Style::default().fg(theme.accent),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    f.render_widget(bottom, bottom_area);

    // Footer con atajos
    let footer_text = Line::from(vec![
        Span::styled(
            " [q] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Salir  ", Style::default().fg(theme.muted)),
        Span::styled(
            "[↑↓] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Navegar  ", Style::default().fg(theme.muted)),
        Span::styled(
            "[ ] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Refresco  ", Style::default().fg(theme.muted)),
        Span::styled(
            "[Tab] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Cambiar panel  ", Style::default().fg(theme.muted)),
        Span::styled(
            "[c] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Contenedores", Style::default().fg(theme.muted)),
    ]);
    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted)),
    );
    f.render_widget(footer, footer_area);
}

fn draw_metrics(f: &mut Frame, area: Rect, state: &AppState) {
    // 2 lines per metric (label + gauge), 1 spacer between sections
    let disk_count = state.disks.len();
    let mut constraints = vec![
        Constraint::Length(2), // CPU
        Constraint::Length(1), // spacer
        Constraint::Length(2), // RAM
        Constraint::Length(1), // spacer
    ];
    for _ in 0..disk_count {
        constraints.push(Constraint::Length(2)); // disk
        constraints.push(Constraint::Length(1)); // spacer
    }
    constraints.push(Constraint::Min(0)); // remainder

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    cpu_bar::render(f, chunks[0], &state.cpu);
    // chunks[1] is spacer
    memory_bar::render(f, chunks[2], &state.memory);
    // chunks[3] is spacer
    for (i, disk) in state.disks.iter().enumerate() {
        let idx = 4 + i * 2;
        if idx < chunks.len() {
            disk_bar::render(f, chunks[idx], disk);
        }
    }
}
