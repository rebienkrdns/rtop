use bytesize::ByteSize;
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
use crate::ui::widgets::{
    container_table, cpu_bar, disk_bar, history_chart, memory_bar, network_widget, process_table,
    psi_widget,
};

pub fn draw(f: &mut Frame, state: &AppState) {
    let theme = Theme::default_theme();
    let area = f.size();

    // Layout vertical: header | métricas | tab_bar | contenido_pestaña | footer
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(13), // métricas (CPU/RAM/Disco + Red)
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
    let right_arrow = if idx < INTERVALS.len() - 1 {
        " ▶"
    } else {
        "  "
    };
    let label = interval_label(idx);
    let interval_ctrl = format!("[ {}{}{} ]", left_arrow, label, right_arrow);

    // 10.4 — Indicador de actualización: alterna ● / ○ en cada refresh
    let tick_dot = if state.refresh_tick { "●" } else { "○" };
    let tick_color = if state.data_loaded {
        Color::Green
    } else {
        theme.muted
    };

    let header_text = Line::from(vec![
        Span::styled(
            " rtop ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(theme.muted)),
        Span::styled(state.hostname.as_str(), Style::default().fg(theme.text)),
        Span::styled(
            format!("    {}: ", state.t("Refresh")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            interval_ctrl,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
        Span::styled(tick_dot, Style::default().fg(tick_color)),
        Span::styled("  ", Style::default()),
        Span::styled(now.as_str(), Style::default().fg(theme.muted)),
        Span::styled(
            format!("   [F1 {}]", state.t("Help")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            format!("   [F4 {}: {}]", state.t("Theme"), state.cfg.theme.name()),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent)),
        ),
        header_area,
    );

    // — Métricas: 2 columnas (CPU·RAM·Disco·Red | Presión PSI) —
    let metrics_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(metrics_area);

    // Columna 1: CPU · RAM · Disco · Red
    let col1_block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("CPU RAM")),
            Style::default().fg(theme.accent),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));
    let col1_inner = col1_block.inner(metrics_cols[0]);
    f.render_widget(col1_block, metrics_cols[0]);

    if state.history_mode {
        let samples = state.metrics_history.tail_n(state.history_range.samples());
        history_chart::render_cpu_ram(f, col1_inner, &samples, state.history_range, state.lang);
    } else {
        draw_metrics(f, col1_inner, state);
    }

    // Columna 2: Disco·Red (historial) | Presión (PSI)
    let col2_title = if state.history_mode {
        format!(" {} ", state.t("Disk Net IO"))
    } else {
        format!(" {} ", state.t("Pressure PSI"))
    };
    let col2_block = Block::default()
        .title(Span::styled(col2_title, Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));
    let col2_inner = col2_block.inner(metrics_cols[1]);
    f.render_widget(col2_block, metrics_cols[1]);

    if state.history_mode {
        let samples = state.metrics_history.tail_n(state.history_range.samples());
        history_chart::render_disk_net(f, col2_inner, &samples, state.history_range, state.lang);
    } else {
        psi_widget::render(f, col2_inner, state);
    }

    // — Barra de pestañas —
    let tabs_line = Line::from(vec![
        Span::styled(
            format!(" {} ", state.t("Processes")),
            Style::default()
                .fg(if state.active_tab == Tab::Processes {
                    theme.selected_fg
                } else {
                    theme.muted
                })
                .bg(if state.active_tab == Tab::Processes {
                    theme.accent_dim
                } else {
                    theme.bg
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!(" {} ", state.t("Containers")),
            Style::default()
                .fg(if state.active_tab == Tab::Containers {
                    theme.selected_fg
                } else {
                    theme.muted
                })
                .bg(if state.active_tab == Tab::Containers {
                    theme.accent_dim
                } else {
                    theme.bg
                })
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(tabs_line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent_dim)),
        ),
        tabbar_area,
    );

    // — Contenido de la pestaña activa —
    match state.active_tab {
        Tab::Processes => {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent_dim));
            let inner = block.inner(content_area);
            f.render_widget(block, content_area);
            process_table::render(f, inner, &state.processes, &state.process_table, state.lang);
        }
        Tab::Containers => {
            let title_str = if state.container_state.available {
                let host_mem = state.memory.total_bytes;
                let total_used: u64 = state.containers.iter().map(|c| c.memory_bytes).sum();
                let has_unlimited = state
                    .containers
                    .iter()
                    .any(|c| c.memory_limit_bytes >= host_mem || c.memory_limit_bytes == 0);
                let (total_limit, has_limit) = if has_unlimited {
                    (0, false)
                } else {
                    (
                        state.containers.iter().map(|c| c.memory_limit_bytes).sum(),
                        true,
                    )
                };
                if has_limit {
                    format!(
                        " {}: {} {} · Mem: {} / {} ",
                        state.t("Containers"),
                        state.containers.len(),
                        state.t("Active"),
                        ByteSize(total_used),
                        ByteSize(total_limit)
                    )
                } else {
                    format!(
                        " {}: {} {} · Mem: {} ",
                        state.t("Containers"),
                        state.containers.len(),
                        state.t("Active"),
                        ByteSize(total_used)
                    )
                }
            } else {
                format!(" {} ", state.t("Containers"))
            };

            let block = Block::default()
                .title(Span::styled(title_str, Style::default().fg(theme.accent)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent_dim));
            let inner = block.inner(content_area);
            f.render_widget(block, content_area);
            if state.container_state.available {
                container_table::render_with_cursor(
                    f,
                    inner,
                    &state.containers,
                    state.container_cursor,
                    state.container_scroll,
                    state.container_sort_col,
                    state.container_sort_asc,
                    &state.collapsed_compose_groups,
                    &state.container_filter,
                    state.container_filter_active,
                    state.lang,
                );
            } else {
                let msg = state
                    .container_state
                    .message
                    .clone()
                    .unwrap_or_else(|| "Docker / Podman not detected".to_string());
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
            Span::styled(
                " [q] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Quit rtop")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[/] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Filter")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[c] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("CPU  ", Style::default().fg(theme.muted)),
            Span::styled(
                "[m] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("RAM  ", Style::default().fg(theme.muted)),
            Span::styled(
                "[r] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("DiskR  ", Style::default().fg(theme.muted)),
            Span::styled(
                "[w] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("DiskW  ", Style::default().fg(theme.muted)),
            Span::styled(
                "[Tab] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Containers")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[h] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("History")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[F4] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ({})  ", state.t("Theme"), state.cfg.theme.name()),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[F1] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(state.t("Help"), Style::default().fg(theme.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " [q] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Quit rtop")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[◀▶] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Refresh")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[F2] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Disk")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[F3] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Network")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[h] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("History")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[t] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Range")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[Tab] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Change tab")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[F4] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ({})  ", state.t("Theme"), state.cfg.theme.name()),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[F1] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(state.t("Help"), Style::default().fg(theme.muted)),
        ])
    };
    f.render_widget(
        Paragraph::new(footer_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent_dim)),
        ),
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
            Constraint::Length(2), // CPU
            Constraint::Length(2), // RAM
            Constraint::Length(3), // Disco (título + barra + I/O)
            Constraint::Min(0),    // Red (ocupa el resto)
        ])
        .split(area);

    cpu_bar::render_with_loading(f, chunks[0], &state.cpu, state.data_loaded);
    memory_bar::render_with_loading(f, chunks[1], &state.memory, state.data_loaded);

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
        disk_bar::render(f, chunks[2], disk, state.lang);
    }

    network_widget::render(f, chunks[3], state);
}
