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
use crate::ui::widgets::{container_table, cpu_bar, disk_bar, memory_bar, network_widget, process_table, psi_widget};

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
    let right_arrow = if idx < INTERVALS.len() - 1 { " ▶" } else { "  " };
    let label = interval_label(idx);
    let interval_ctrl = format!("[ {}{}{} ]", left_arrow, label, right_arrow);

    // 10.4 — Indicador de actualización: alterna ● / ○ en cada refresh
    let tick_dot = if state.refresh_tick { "●" } else { "○" };
    let tick_color = if state.data_loaded { Color::Green } else { theme.muted };

    let header_text = Line::from(vec![
        Span::styled(" rtop ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(theme.muted)),
        Span::styled(state.hostname.as_str(), Style::default().fg(theme.text)),
        Span::styled("    Refresco: ", Style::default().fg(theme.muted)),
        Span::styled(interval_ctrl, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
        Span::styled(tick_dot, Style::default().fg(tick_color)),
        Span::styled("  ", Style::default()),
        Span::styled(now.as_str(), Style::default().fg(theme.muted)),
        Span::styled("   [F1 Ayuda]", Style::default().fg(theme.muted)),
    ]);
    f.render_widget(
        Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.accent))),
        header_area,
    );

    // — Métricas responsivas: 3 columnas si es ancho, 2 columnas si es compacto —
    let is_wide = area.width >= 120;

    if is_wide {
        let metrics_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(35),
                Constraint::Percentage(35),
            ])
            .split(metrics_area);

        // Columna 1: CPU y RAM
        let col1_block = Block::default()
            .title(Span::styled(" CPU · RAM ", Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let col1_inner = col1_block.inner(metrics_cols[0]);
        f.render_widget(col1_block, metrics_cols[0]);

        let col1_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // CPU
                Constraint::Length(1), // spacer
                Constraint::Length(2), // RAM
                Constraint::Min(0),
            ])
            .split(col1_inner);
        cpu_bar::render_with_loading(f, col1_layout[0], &state.cpu, state.data_loaded);
        memory_bar::render_with_loading(f, col1_layout[2], &state.memory, state.data_loaded);

        // Columna 2: Disco y Red (I/O)
        let col2_block = Block::default()
            .title(Span::styled(" Disco · Red (I/O) ", Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let col2_inner = col2_block.inner(metrics_cols[1]);
        f.render_widget(col2_block, metrics_cols[1]);

        let col2_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Disco (título + barra + I/O + margen/spacer)
                Constraint::Length(1), // spacer
                Constraint::Min(0),    // Red
            ])
            .split(col2_inner);

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
            disk_bar::render(f, col2_layout[0], disk);
        }
        network_widget::render(f, col2_layout[2], state);

        // Columna 3: Presión (PSI)
        let col3_block = Block::default()
            .title(Span::styled(" Presión (PSI) ", Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let col3_inner = col3_block.inner(metrics_cols[2]);
        f.render_widget(col3_block, metrics_cols[2]);
        psi_widget::render(f, col3_inner, state.psi.as_ref(), true);
    } else {
        let metrics_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Percentage(55),
            ])
            .split(metrics_area);

        // Columna 1: CPU, RAM y Disco
        let col1_block = Block::default()
            .title(Span::styled(" CPU · RAM · Disco ", Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let col1_inner = col1_block.inner(metrics_cols[0]);
        f.render_widget(col1_block, metrics_cols[0]);
        draw_metrics(f, col1_inner, state);

        // Columna 2: Red y Presión (PSI)
        let col2_block = Block::default()
            .title(Span::styled(" Red · Presión (PSI) ", Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let col2_inner = col2_block.inner(metrics_cols[1]);
        f.render_widget(col2_block, metrics_cols[1]);

        let col2_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Red
                Constraint::Length(1), // spacer
                Constraint::Min(0),    // PSI
            ])
            .split(col2_inner);

        network_widget::render(f, col2_layout[0], state);
        psi_widget::render(f, col2_layout[2], state.psi.as_ref(), false);
    }

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
                container_table::render_with_cursor(f, inner, &state.containers, state.container_cursor);
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
            Span::styled("Contenedores  ", Style::default().fg(theme.muted)),
            Span::styled("[F1] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Ayuda", Style::default().fg(theme.muted)),
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
            Span::styled("Cambiar  ", Style::default().fg(theme.muted)),
            Span::styled("[F1] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Ayuda", Style::default().fg(theme.muted)),
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
            Constraint::Length(2), // CPU
            Constraint::Length(1), // separador
            Constraint::Length(2), // RAM
            Constraint::Length(1), // separador
            Constraint::Length(4), // Disco (título + barra + I/O + margen)
            Constraint::Min(0),
        ])
        .split(area);

    cpu_bar::render_with_loading(f, chunks[0], &state.cpu, state.data_loaded);
    memory_bar::render_with_loading(f, chunks[2], &state.memory, state.data_loaded);

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
