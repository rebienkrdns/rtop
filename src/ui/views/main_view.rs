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
    container_table, cpu_cores, disk_bar, gpu_widget, history_chart, memory_bar, network_widget,
    process_table, psi_widget,
};

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("up {}d {}h", days, hours)
    } else if hours > 0 {
        format!("up {}h {}m", hours, mins)
    } else {
        format!("up {}m", mins)
    }
}

fn cpu_arch_label() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "ARM64",
        "x86_64" => "x86_64",
        "x86" => "x86",
        "arm" => "ARM",
        other => other,
    }
}

pub fn draw(f: &mut Frame, state: &AppState) {
    let theme = Theme::default_theme();
    let area = f.size();

    let gpu_count = state.gpus.len() as u16;
    let gpu_section_height = if gpu_count > 0 { gpu_count * 4 } else { 0 };

    // Altura: 1 agregado + 2 filas SRE (Linux) + 1 por núcleo + 2 bordes
    let num_cores = state.cpu.per_core.len().max(1);
    #[cfg(target_os = "linux")]
    let extra_cpu_rows: u16 = 2; // USR/SYS/IOW/STL + CTX/INT
    #[cfg(not(target_os = "linux"))]
    let extra_cpu_rows: u16 = 0;

    // Height calculation:
    // Ideal height is 2 (gauge) + extra + 2 lines per core pair + 2 borders
    let ideal_core_rows = ((num_cores + 1) / 2) as u16;
    let ideal_cpu_height = 2 + extra_cpu_rows + (ideal_core_rows * 2) + 2;

    // Calculate maximum safe height without pushing bottom elements off-screen
    let max_safe_height = area
        .height
        .saturating_sub(3 + 3 + 5 + 3 + gpu_section_height);

    // Assign ideal height, constrained by safe limits and a small minimum for neighboring metrics
    let metrics_height = ideal_cpu_height.min(max_safe_height).max(13);

    // Layout vertical: header | métricas (3 grupos) | [GPU] | tab_bar | contenido_pestaña | footer
    let mut constraints = vec![
        Constraint::Length(3),              // header
        Constraint::Length(metrics_height), // métricas (3 grupos)
    ];
    if gpu_section_height > 0 {
        constraints.push(Constraint::Length(gpu_section_height));
    }
    constraints.push(Constraint::Length(3)); // barra de pestañas
    constraints.push(Constraint::Min(5)); // contenido de pestaña activa
    constraints.push(Constraint::Length(3)); // footer

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let header_area = vertical[0];
    let metrics_area = vertical[1];
    let (gpu_area_opt, tabbar_area, content_area, footer_area) = if gpu_section_height > 0 {
        (Some(vertical[2]), vertical[3], vertical[4], vertical[5])
    } else {
        (None, vertical[2], vertical[3], vertical[4])
    };

    // — Header —
    let now_dt = Local::now();
    let now_str = now_dt.format("%H:%M:%S").to_string();
    let mut tz_str = now_dt.format("%Z").to_string(); // e.g. "UTC", "CST", "PDT"
    if tz_str == "+00:00" || tz_str == "Z" {
        tz_str = "UTC".to_string();
    } else if tz_str.starts_with('+') || tz_str.starts_with('-') {
        tz_str = format!("UTC{}", tz_str);
    }
    let uptime_str = format_uptime(state.uptime_secs);
    let idx = state.interval_idx;
    let left_arrow = if idx > 0 { "◀ " } else { "  " };
    let right_arrow = if idx < INTERVALS.len() - 1 {
        " ▶"
    } else {
        "  "
    };
    let label = interval_label(idx);
    let interval_ctrl = format!("[ {}{}{} ]", left_arrow, label, right_arrow);

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
        Span::styled(
            format!("v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme.muted),
        ),
        Span::styled("│ ", Style::default().fg(theme.muted)),
        Span::styled(state.hostname.as_str(), Style::default().fg(theme.text)),
        Span::styled(
            format!("  {}", uptime_str),
            Style::default().fg(theme.muted),
        ),
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
        Span::styled(now_str, Style::default().fg(theme.muted)),
        Span::styled(format!(" {}", tz_str), Style::default().fg(theme.muted)),
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

    // — Métricas: 3 columnas iguales (CPU | Memoria·Disco·Red | Presión PSI) —
    let metrics_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3), // CPU por núcleo
            Constraint::Ratio(1, 3), // RAM + Disco + Red
            Constraint::Ratio(1, 3), // PSI
        ])
        .split(metrics_area);

    // Grupo 1: CPU
    let core_count = state.cpu.per_core.len();
    let cpu_brand = state
        .cpu
        .per_core
        .first()
        .map(|c| c.brand.as_str())
        .unwrap_or("CPU")
        .trim();
    let temp_sum: f64 = state
        .cpu
        .per_core
        .iter()
        .filter_map(|c| c.temperature_celsius)
        .sum();
    let temp_count = state
        .cpu
        .per_core
        .iter()
        .filter(|c| c.temperature_celsius.is_some())
        .count();
    let temp_str = if temp_count > 0 {
        format!(" · {:.1}°C", temp_sum / temp_count as f64)
    } else {
        String::new()
    };

    let cpu_title = format!(
        " {} · {} CORES [{}]{} ",
        cpu_brand,
        core_count,
        cpu_arch_label(),
        temp_str
    );
    let col1_block = Block::default()
        .title(Span::styled(cpu_title, Style::default().fg(theme.accent)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));
    let col1_inner = col1_block.inner(metrics_cols[0]);
    f.render_widget(col1_block, metrics_cols[0]);

    if state.history_mode {
        let samples = state.metrics_history.tail_n(state.history_range.samples());
        history_chart::render_cpu_ram(f, col1_inner, &samples, state.history_range, state.lang);
    } else {
        cpu_cores::render_cpu_cores(f, col1_inner, &state.cpu, state.data_loaded);
    }

    // Grupo 2: Memoria · Disco · Red
    let col2_title = if state.history_mode {
        format!(" {} ", state.t("Disk Net IO"))
    } else {
        format!(" {} ", state.t("Mem Disk Net"))
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
        draw_system_metrics(f, col2_inner, state);
    }

    // Grupo 3: Presión PSI
    let col3_block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("Pressure PSI")),
            Style::default().fg(theme.accent),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));
    let col3_inner = col3_block.inner(metrics_cols[2]);
    f.render_widget(col3_block, metrics_cols[2]);
    psi_widget::render(f, col3_inner, state);

    // — GPU (si hay GPUs detectadas) —
    if let Some(gpu_area) = gpu_area_opt {
        gpu_widget::render(f, gpu_area, &state.gpus);
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

fn best_disk_for_display<'a>(
    disks: &'a [crate::models::DiskData],
    selected: Option<&str>,
) -> Option<&'a crate::models::DiskData> {
    if let Some(sel) = selected {
        if let Some(d) = disks
            .iter()
            .find(|d| crate::collectors::disk::device_short_name(&d.device) == sel)
        {
            return Some(d);
        }
    }
    disks
        .iter()
        .find(|d| d.device.starts_with("/dev/") && !d.device.contains("loop") && d.total_bytes > 0)
        .or_else(|| disks.first())
}

fn draw_system_metrics(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // RAM + Swap
            Constraint::Length(4), // Disco + latencia
            Constraint::Min(0),    // Red
        ])
        .split(area);

    memory_bar::render_with_loading(f, chunks[0], &state.memory, state.data_loaded);

    let disk_to_render = best_disk_for_display(&state.disks, state.selected_disk.as_deref());

    if let Some(disk) = disk_to_render {
        disk_bar::render(f, chunks[1], disk, state.lang);
    }

    network_widget::render(f, chunks[2], state);
}
