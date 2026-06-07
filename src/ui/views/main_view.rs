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
use crate::ui::views::{disk_selector, nic_selector};
use crate::ui::widgets::{cpu_bar, disk_bar, memory_bar, network_widget};

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
    let right_block = Block::default()
        .title(Span::styled(" Red ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    let right_inner = right_block.inner(right_area);
    f.render_widget(right_block, right_area);
    network_widget::render(f, right_inner, state);

    // Panel inferior: Procesos / Contenedores
    let bottom_title = if state.proc_permission_denied {
        " Procesos (requiere sudo) · Contenedores "
    } else {
        " Procesos · Contenedores "
    };
    let bottom = Block::default()
        .title(Span::styled(
            bottom_title,
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
            "[ ] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Refresco  ", Style::default().fg(theme.muted)),
        Span::styled(
            "[F2] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Disco  ", Style::default().fg(theme.muted)),
        Span::styled(
            "[F3] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("Red  ", Style::default().fg(theme.muted)),
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

    if state.show_nic_selector {
        nic_selector::render(f, state);
    }
    if state.show_disk_selector {
        disk_selector::render(f, state);
    }
}

fn draw_metrics(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // CPU
            Constraint::Length(1), // spacer
            Constraint::Length(2), // RAM
            Constraint::Length(1), // spacer
            Constraint::Length(3), // Disk
            Constraint::Min(0),    // remainder
        ])
        .split(area);

    cpu_bar::render(f, chunks[0], &state.cpu);
    // chunks[1] is spacer
    memory_bar::render(f, chunks[2], &state.memory);
    // chunks[3] is spacer

    let selected_disk = state.selected_disk.as_deref().unwrap_or("");
    let disk_to_render = state.disks.iter().find(|d| {
        let short = crate::collectors::disk::device_short_name(&d.device);
        short == selected_disk
    }).or_else(|| state.disks.first());

    if let Some(disk) = disk_to_render {
        disk_bar::render(f, chunks[4], disk);
    }
}
