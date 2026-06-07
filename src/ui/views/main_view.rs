use chrono::Local;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::config::{interval_label, INTERVALS};
use crate::ui::theme::Theme;
use crate::ui::views::{disk_selector, nic_selector};
use crate::ui::widgets::{container_table, cpu_bar, disk_bar, memory_bar, network_widget, process_table};

pub fn draw(f: &mut Frame, state: &AppState) {
    let theme = Theme::default_theme();
    let area = f.size();

    // Layout vertical: header | métricas | tab_bar | contenido_pestaña | footer
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(10), // métricas (CPU/RAM/Disco + Red)
            Constraint::Length(3),  // barra de pestañas
            Constraint::Min(5),     // contenido de pestaña activa
            Constraint::Length(3),  // footer
        ])
        .split(area);

    let header_area = vertical[0];
    let metrics_area = vertical[1];
    let tabbar_area = vertical[2];
    let content_area = vertical[3];
    let footer_area = vertical[4];

    // — Header —
    let now = Local::now().format("%H:%M:%S").to_string();
    let idx = state.interval_idx;
    let left_arrow = if idx > 0 { "◀ " } else { "  " };
    let right_arrow = if idx < INTERVALS.len() - 1 { " ▶" } else { "  " };
    let label = interval_label(idx);
    let interval_ctrl = format!("[ {}{}{} ]", left_arrow, label, right_arrow);

    let header_text = Line::from(vec![
        Span::styled(" rtop ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(theme.muted)),
        Span::styled(state.hostname.as_str(), Style::default().fg(theme.text)),
        Span::styled("    Refresco: ", Style::default().fg(theme.muted)),
        Span::styled(interval_ctrl, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("    ", Style::default()),
        Span::styled(now.as_str(), Style::default().fg(theme.muted)),
    ]);
    f.render_widget(
        Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.accent))),
        header_area,
    );

    // — Métricas: izquierda CPU/RAM/Disco | derecha Red —
    let metrics_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(metrics_area);

    let left_block = Block::default()
        .title(Span::styled(" CPU · RAM · Disco ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    let left_inner = left_block.inner(metrics_cols[0]);
    f.render_widget(left_block, metrics_cols[0]);
    draw_metrics(f, left_inner, state);

    let right_block = Block::default()
        .title(Span::styled(" Red ", Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.muted));
    let right_inner = right_block.inner(metrics_cols[1]);
    f.render_widget(right_block, metrics_cols[1]);
    network_widget::render(f, right_inner, state);

    // — Barra de pestañas —
    let tabs_line = Line::from(vec![
        Span::styled(
            " Procesos ",
            Style::default()
                .fg(if state.active_tab == Tab::Processes { Color::Black } else { theme.text })
                .bg(if state.active_tab == Tab::Processes { theme.accent } else { theme.bg })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            " Contenedores ",
            Style::default()
                .fg(if state.active_tab == Tab::Containers { Color::Black } else { theme.text })
                .bg(if state.active_tab == Tab::Containers { theme.accent } else { theme.bg })
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(tabs_line)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.muted))),
        tabbar_area,
    );

    // — Contenido de la pestaña activa —
    match state.active_tab {
        Tab::Processes => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.muted));
            let inner = block.inner(content_area);
            f.render_widget(block, content_area);
            process_table::render(f, inner, &state.processes, &state.process_table);
        }
        Tab::Containers => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.muted));
            let inner = block.inner(content_area);
            f.render_widget(block, content_area);
            if state.container_state.available {
                container_table::render(f, inner, &state.containers);
            } else {
                let msg = state
                    .container_state
                    .message
                    .clone()
                    .unwrap_or_else(|| "Docker / Podman no detectado".to_string());
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("⚠  ", Style::default().fg(Color::Yellow)),
                        Span::styled(msg, Style::default().fg(theme.muted)),
                    ])),
                    inner,
                );
            }
        }
        Tab::Network => {}
    }

    // — Footer —
    use crate::config::Tab;
    let footer_text = if state.active_tab == Tab::Processes {
        Line::from(vec![
            Span::styled(" [q] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Salir  ", Style::default().fg(theme.muted)),
            Span::styled("[/] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Filtrar  ", Style::default().fg(theme.muted)),
            Span::styled("[c] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("CPU  ", Style::default().fg(theme.muted)),
            Span::styled("[m] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("RAM  ", Style::default().fg(theme.muted)),
            Span::styled("[r] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("DiskR  ", Style::default().fg(theme.muted)),
            Span::styled("[w] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("DiskW  ", Style::default().fg(theme.muted)),
            Span::styled("[Tab] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Contenedores", Style::default().fg(theme.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" [q] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Salir  ", Style::default().fg(theme.muted)),
            Span::styled("[◀▶] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Refresco  ", Style::default().fg(theme.muted)),
            Span::styled("[F2] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Disco  ", Style::default().fg(theme.muted)),
            Span::styled("[F3] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Red  ", Style::default().fg(theme.muted)),
            Span::styled("[Tab] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Cambiar pestaña", Style::default().fg(theme.muted)),
        ])
    };
    f.render_widget(
        Paragraph::new(footer_text)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.muted))),
        footer_area,
    );

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
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    cpu_bar::render(f, chunks[0], &state.cpu);
    memory_bar::render(f, chunks[2], &state.memory);

    let selected_disk = state.selected_disk.as_deref().unwrap_or("");
    let disk_to_render = state
        .disks
        .iter()
        .find(|d| {
            let short = crate::collectors::disk::device_short_name(&d.device);
            short == selected_disk
        })
        .or_else(|| state.disks.first());

    if let Some(disk) = disk_to_render {
        disk_bar::render(f, chunks[4], disk);
    }
}
