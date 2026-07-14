use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::app::AppState;
use crate::models::{ContainerData, ContainerStatus};
use crate::ui::history::ContainerHistorySample;
use crate::ui::theme::Theme;
use crate::ui::widgets::history_chart::render_history_canvas_dual;

fn samples_from_history(
    history: &std::collections::VecDeque<ContainerHistorySample>,
    limit: usize,
) -> Vec<&ContainerHistorySample> {
    let s_len = history.len().min(limit);
    let skip = history.len().saturating_sub(s_len);
    history.iter().skip(skip).collect()
}

fn max_bps_cont(
    history: &std::collections::VecDeque<ContainerHistorySample>,
    limit: usize,
    f: impl Fn(&ContainerHistorySample) -> f64,
) -> f64 {
    let s_len = history.len().min(limit);
    let skip = history.len().saturating_sub(s_len);
    history.iter().skip(skip).map(f).fold(0.0_f64, f64::max)
}

pub fn render(
    f: &mut Frame,
    area: Rect,
    container: &ContainerData,
    confirm: Option<&ConfirmAction>,
    state: &AppState,
) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Span::styled(
            format!(
                " {}: {} ",
                state.t("ContainerDetailHeader").trim_end_matches(':'),
                container.name
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let has_right_panel = container.database_type.is_some()
        || container.proxy_type.is_some()
        || container.node_runtime_type.is_some()
        || container.message_broker_type.is_some();
    let (left_area, db_area) = if has_right_panel {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(inner);
        (cols[0], Some(cols[1]))
    } else {
        (inner, None)
    };

    let inner_height = left_area.height;
    let remaining_height = inner_height.saturating_sub(7); // basic info (5) + footer (2) = 7
    let metadata_reserved = 6;
    let charts_total_height = remaining_height.saturating_sub(metadata_reserved);
    let chart_height = if state.history_mode {
        (charts_total_height / 4).max(3)
    } else {
        3
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),            // info básica
            Constraint::Length(chart_height), // CPU bar
            Constraint::Length(chart_height), // Memory bar
            Constraint::Length(chart_height), // Net bar
            Constraint::Length(chart_height), // Disk bar
            Constraint::Min(1),               // metadata (puertos, volúmenes, redes, env)
            Constraint::Length(2),            // footer
        ])
        .split(left_area);

    // Info fields
    let uptime_str = container
        .uptime_secs
        .map(format_uptime)
        .unwrap_or_else(|| "—".to_string());
    let status_color = match &container.status {
        ContainerStatus::Running => Color::Green,
        ContainerStatus::Paused => Color::Yellow,
        ContainerStatus::Restarting => Color::Magenta,
        ContainerStatus::Exited => Color::DarkGray,
        ContainerStatus::Dead => Color::Red,
        ContainerStatus::Unknown => Color::Gray,
    };

    let id_short = if container.id.len() > 12 {
        &container.id[..12]
    } else {
        &container.id
    };
    let info_lines = vec![
        Line::from(vec![
            Span::styled("ID:     ", Style::default().fg(theme.muted)),
            Span::styled(
                id_short,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                format!("{}: ", state.t("Image")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(container.image.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", state.t("State")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                container.status.as_str(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                format!("{}: ", state.t("Uptime")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(uptime_str, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("RAM:    ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} / {}",
                    ByteSize(container.memory_bytes),
                    ByteSize(container.memory_limit_bytes)
                ),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", state.t("Net Tot")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!(
                    "↓ {}  ·  ↑ {}",
                    ByteSize(container.net_recv_total),
                    ByteSize(container.net_sent_total)
                ),
                Style::default().fg(Color::White),
            ),
            Span::raw("   "),
            Span::styled(
                format!("{}: ", state.t("Disk Tot")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!(
                    "R {}  ·  W {}",
                    ByteSize(container.disk_read_total),
                    ByteSize(container.disk_write_total)
                ),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(info_lines), chunks[0]);

    // CPU bar / history
    let cpu_pct = container.cpu_pct.clamp(0.0, 100.0);
    let limit = state.history_range.samples();
    let samps = samples_from_history(&state.container_history, limit);
    if state.history_mode {
        let cpu_block = Block::default()
            .title(Span::styled(
                format!(
                    " {} ({}) · {}: {:.1}% ",
                    state.t("CPUHistory"),
                    state.history_range.label(),
                    state.t("LastLabel"),
                    cpu_pct
                ),
                Style::default().fg(theme.muted),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let inner_area = cpu_block.inner(chunks[1]);
        f.render_widget(cpu_block, chunks[1]);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            100.0,
            Theme::color_for_pct(cpu_pct),
            |s: &ContainerHistorySample| s.cpu_pct,
            None,
        );
    } else {
        let cpu_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(" CPU  {:.1}% ", cpu_pct),
                        Style::default().fg(theme.muted),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.muted)),
            )
            .gauge_style(
                Style::default()
                    .fg(Theme::color_for_pct(cpu_pct))
                    .bg(Color::DarkGray),
            )
            .ratio(cpu_pct / 100.0);
        f.render_widget(cpu_gauge, chunks[1]);
    }

    // Memory bar / history
    let mem_pct = container.memory_pct.clamp(0.0, 100.0);
    if state.history_mode {
        let mem_block = Block::default()
            .title(Span::styled(
                format!(
                    " {} ({}) · {}: {:.1}% ",
                    state.t("MemHistory"),
                    state.history_range.label(),
                    state.t("LastLabel"),
                    mem_pct
                ),
                Style::default().fg(theme.muted),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let inner_area = mem_block.inner(chunks[2]);
        f.render_widget(mem_block, chunks[2]);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            100.0,
            Theme::color_for_pct(mem_pct),
            |s: &ContainerHistorySample| s.mem_pct,
            None,
        );
    } else {
        let mem_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(" Memoria  {:.1}% ", mem_pct),
                        Style::default().fg(theme.muted),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.muted)),
            )
            .gauge_style(
                Style::default()
                    .fg(Theme::color_for_pct(mem_pct))
                    .bg(Color::DarkGray),
            )
            .ratio(mem_pct / 100.0);
        f.render_widget(mem_gauge, chunks[2]);
    }

    // Network bar / history
    let net_recv = container.net_recv_per_sec;
    let net_sent = container.net_sent_per_sec;
    if state.history_mode {
        let net_block = Block::default()
            .title(Span::styled(
                format!(
                    " {} ({}) · {}: ↓{}/s · ↑{}/s ",
                    state.t("Net History"),
                    state.history_range.label(),
                    state.t("LastLabel"),
                    ByteSize(net_recv as u64),
                    ByteSize(net_sent as u64)
                ),
                Style::default().fg(theme.muted),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let inner_area = net_block.inner(chunks[3]);
        f.render_widget(net_block, chunks[3]);
        let net_max = max_bps_cont(&state.container_history, limit, |s| {
            s.net_recv_bps.max(s.net_sent_bps)
        })
        .max(1.0);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            net_max,
            Color::Cyan,
            |s: &ContainerHistorySample| s.net_recv_bps,
            Some((Color::Rgb(255, 80, 80), |s: &ContainerHistorySample| {
                s.net_sent_bps
            })),
        );
    } else {
        let net_ratio = (net_recv.max(net_sent) / 10_000_000.0).clamp(0.0, 1.0);
        let net_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(
                            " {}  ↓{}/s (Total: {})  ↑{}/s (Total: {}) ",
                            state.t("Network"),
                            ByteSize(net_recv as u64),
                            ByteSize(container.net_recv_total),
                            ByteSize(net_sent as u64),
                            ByteSize(container.net_sent_total)
                        ),
                        Style::default().fg(theme.muted),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.muted)),
            )
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(net_ratio);
        f.render_widget(net_gauge, chunks[3]);
    }

    // Disk bar / history
    let disk_r = container.disk_read_per_sec;
    let disk_w = container.disk_write_per_sec;
    if state.history_mode {
        let disk_block = Block::default()
            .title(Span::styled(
                format!(
                    " {} ({}) · {}: R:{}/s · W:{}/s ",
                    state.t("Disk History"),
                    state.history_range.label(),
                    state.t("LastLabel"),
                    ByteSize(disk_r as u64),
                    ByteSize(disk_w as u64)
                ),
                Style::default().fg(theme.muted),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let inner_area = disk_block.inner(chunks[4]);
        f.render_widget(disk_block, chunks[4]);
        let disk_max = max_bps_cont(&state.container_history, limit, |s| {
            s.disk_read_bps.max(s.disk_write_bps)
        })
        .max(1.0);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            disk_max,
            Color::Yellow,
            |s: &ContainerHistorySample| s.disk_read_bps,
            Some((Color::Rgb(255, 200, 80), |s: &ContainerHistorySample| {
                s.disk_write_bps
            })),
        );
    } else {
        let disk_ratio = (disk_r.max(disk_w) / 100_000_000.0).clamp(0.0, 1.0);
        let disk_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(
                            " {}  R:{}/s (Total: {})  W:{}/s (Total: {}) ",
                            state.t("Disk"),
                            ByteSize(disk_r as u64),
                            ByteSize(container.disk_read_total),
                            ByteSize(disk_w as u64),
                            ByteSize(container.disk_write_total)
                        ),
                        Style::default().fg(theme.muted),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.muted)),
            )
            .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .ratio(disk_ratio);
        f.render_widget(disk_gauge, chunks[4]);
    }

    // Metadata: puertos, volúmenes, redes, env vars — todo en un único bloque con scroll
    {
        let meta_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.muted));
        let meta_inner = meta_block.inner(chunks[5]);
        f.render_widget(meta_block, chunks[5]);

        let mut lines: Vec<Line> = Vec::new();

        let muted = theme.muted;

        lines.push(Line::from(Span::styled(
            format!("── {} ", state.t("Ports")),
            Style::default().fg(muted),
        )));
        if container.ports.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("—", Style::default().fg(Color::DarkGray)),
            ]));
        } else {
            let mut sorted_ports = container.ports.clone();
            sorted_ports.sort();
            for p in &sorted_ports {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(p.clone(), Style::default().fg(Color::Cyan)),
                ]));
            }
        }

        lines.push(Line::from(Span::styled(
            format!("── {} ", state.t("Volumes")),
            Style::default().fg(muted),
        )));
        if container.volumes.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("—", Style::default().fg(Color::DarkGray)),
            ]));
        } else {
            let mut sorted_volumes = container.volumes.clone();
            sorted_volumes.sort();
            for v in &sorted_volumes {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(v.clone(), Style::default().fg(Color::Yellow)),
                ]));
            }
        }

        lines.push(Line::from(Span::styled(
            format!("── {} ", state.t("Networks")),
            Style::default().fg(muted),
        )));
        if container.networks.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("—", Style::default().fg(Color::DarkGray)),
            ]));
        } else {
            let mut sorted_networks = container.networks.clone();
            sorted_networks.sort();
            for n in &sorted_networks {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(n.clone(), Style::default().fg(Color::Magenta)),
                ]));
            }
        }

        let env_toggle_hint = if state.show_env_values {
            format!(" [E {}]", state.t("EnvToggleHide"))
        } else {
            format!(" [E {}]", state.t("EnvToggleShow"))
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("── {} ", state.t("EnvVarsLabel")),
                Style::default().fg(muted),
            ),
            Span::styled(env_toggle_hint, Style::default().fg(Color::DarkGray)),
        ]));
        if container.env_vars.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("—", Style::default().fg(Color::DarkGray)),
            ]));
        } else {
            let mut sorted_env = container.env_vars.clone();
            sorted_env.sort();
            for e in &sorted_env {
                if let Some((key, val)) = e.split_once('=') {
                    let display_val = if state.show_env_values {
                        val.to_string()
                    } else {
                        "●●●●●●".to_string()
                    };
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            format!("{}=", key),
                            Style::default().fg(Color::Rgb(165, 213, 102)),
                        ),
                        Span::styled(
                            display_val,
                            Style::default().fg(if state.show_env_values {
                                Color::White
                            } else {
                                Color::DarkGray
                            }),
                        ),
                    ]));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(e.clone(), Style::default().fg(Color::White)),
                    ]));
                }
            }
        }

        let total_lines = lines.len();
        let visible_height = meta_inner.height as usize;
        let max_scroll = total_lines.saturating_sub(visible_height);
        let scroll = state.detail_meta_scroll.min(max_scroll);

        f.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), meta_inner);

        // Scrollbar visual
        if total_lines > visible_height {
            let mut scrollbar_state = ScrollbarState::new(max_scroll).position(scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                chunks[5],
                &mut scrollbar_state,
            );
        }
    }

    // Footer
    let hint = Line::from(vec![
        Span::styled(
            " [ESC] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("BackLabel")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[L] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("Logs")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[R] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("Restart")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[S] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("Stop")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[H] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("History")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[T] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} ({})  ", state.t("Range"), state.history_range.label()),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[↑↓] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("Navigate")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "[E] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if state.show_env_values {
                state.t("EnvToggleHide")
            } else {
                state.t("EnvToggleShow")
            },
            Style::default().fg(theme.muted),
        ),
    ]);
    f.render_widget(Paragraph::new(hint), chunks[6]);

    if let Some(db_rect) = db_area {
        if container.database_type.is_some() {
            crate::ui::views::process_detail::render_db_panel(f, db_rect, state, &theme);
        } else if container.proxy_type.is_some() {
            crate::ui::views::process_detail::render_proxy_panel(f, db_rect, state, &theme);
        } else if let Some(node_type) = container.node_runtime_type {
            crate::ui::views::node_runtime_panel::render_node_panel(
                f,
                db_rect,
                state,
                node_type.as_str(),
            );
        } else if container.message_broker_type.is_some() {
            crate::ui::views::process_detail::render_broker_panel(f, db_rect, state, &theme);
        }
    }

    // Confirmation overlay
    if let Some(action) = confirm {
        render_confirm_dialog(f, area, action, state);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    Restart(String), // container id
    Stop(String),
}

fn render_confirm_dialog(f: &mut Frame, area: Rect, action: &ConfirmAction, state: &AppState) {
    let theme = Theme::default_theme();

    // Center a small dialog box
    let dialog_w = 50u16.min(area.width.saturating_sub(4));
    let dialog_h = 5u16;
    let x = area.x + (area.width.saturating_sub(dialog_w)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_h)) / 2;
    let dialog_area = Rect {
        x,
        y,
        width: dialog_w,
        height: dialog_h,
    };

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("Confirm")),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(dialog_area);
    f.render_widget(ratatui::widgets::Clear, dialog_area);
    f.render_widget(block, dialog_area);

    let msg = state
        .t(if matches!(action, ConfirmAction::Restart(_)) {
            "YesNoConfirmRestart"
        } else {
            "YesNoConfirmStop"
        })
        .to_string();
    let lines = vec![
        Line::from(Span::styled(msg, Style::default().fg(Color::White))),
        Line::from(vec![]),
        Line::from(vec![
            Span::styled(
                "[Enter] ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}  ", state.t("Confirm")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[ESC] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(state.t("Cancel"), Style::default().fg(theme.muted)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}
