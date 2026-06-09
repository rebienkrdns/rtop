use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::models::ProcessData;
use crate::ui::history::ProcessHistorySample;
use crate::ui::theme::Theme;
use crate::ui::widgets::history_chart::render_history_canvas_dual;

fn samples_from_history(
    history: &std::collections::VecDeque<ProcessHistorySample>,
    limit: usize,
) -> Vec<&ProcessHistorySample> {
    let s_len = history.len().min(limit);
    let skip = history.len().saturating_sub(s_len);
    history.iter().skip(skip).collect()
}

fn max_bps_proc(
    history: &std::collections::VecDeque<ProcessHistorySample>,
    limit: usize,
    f: impl Fn(&ProcessHistorySample) -> f64,
) -> f64 {
    let s_len = history.len().min(limit);
    let skip = history.len().saturating_sub(s_len);
    history.iter().skip(skip).map(f).fold(0.0_f64, f64::max)
}

pub fn render(f: &mut Frame, area: Rect, process: &ProcessData, state: &AppState) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Span::styled(
            format!(
                " {} {} (PID {}) ",
                state.t("ProcessDetailHeader"),
                process.name,
                process.pid
            ),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (left_area, db_area) = if process.database_type.is_some() {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .split(inner);
        (h_chunks[0], Some(h_chunks[1]))
    } else {
        (inner, None)
    };

    let inner_height = left_area.height;
    let remaining_height = inner_height.saturating_sub(10);
    let chart_height = if state.history_mode {
        (remaining_height / 4).max(3)
    } else {
        3
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),            // info fields
            Constraint::Length(chart_height), // CPU bar
            Constraint::Length(chart_height), // Memory bar
            Constraint::Length(chart_height), // Disk Read bar
            Constraint::Length(chart_height), // Disk Write bar
            Constraint::Length(2),            // footer hint
            Constraint::Min(0),
        ])
        .split(left_area);

    // Info fields
    let uptime_str = format_uptime(process.uptime_secs);
    let info_lines = vec![
        Line::from(vec![
            Span::styled("PID:        ", Style::default().fg(theme.muted)),
            Span::styled(
                process.pid.to_string(),
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                format!("{}: ", state.t("UserLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(process.user.clone(), Style::default().fg(theme.text)),
            Span::raw("    "),
            Span::styled(
                format!("{}: ", state.t("StatusLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                process.status.to_localized_str(state.lang),
                Style::default().fg(theme.ok),
            ),
            Span::raw("    "),
            Span::styled(
                format!("{}: ", state.t("ThreadsLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(process.threads.to_string(), Style::default().fg(theme.text)),
            Span::raw("    "),
            Span::styled(
                format!("{}: ", state.t("UptimeLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(uptime_str, Style::default().fg(theme.text)),
            Span::raw("    "),
            Span::styled("RAM: ", Style::default().fg(theme.muted)),
            Span::styled(
                ByteSize(process.memory_bytes).to_string(),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", state.t("PathLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(process.exe_path.clone(), Style::default().fg(theme.accent)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}: ", state.t("DirectoryLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(process.cwd.clone(), Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{}:    ", state.t("CommandLabel")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(process.cmd.clone(), Style::default().fg(theme.warn)),
        ]),
    ];
    f.render_widget(
        Paragraph::new(info_lines).wrap(ratatui::widgets::Wrap { trim: false }),
        chunks[0],
    );

    let cpu_pct = process.cpu_pct.clamp(0.0, 100.0);
    let mem_pct = process.memory_pct.clamp(0.0, 100.0);
    let read_rate = process.disk_read_per_sec.unwrap_or(0.0);
    let write_rate = process.disk_write_per_sec.unwrap_or(0.0);
    let limit = state.history_range.samples();

    if state.history_mode {
        let samps = samples_from_history(&state.process_history, limit);

        // CPU History
        let cpu_block = Block::default()
            .title(Span::styled(
                format!(
                    " {} ({}) · {}: {:.1}% ",
                    state.t("CPUHistory"),
                    state.history_range.label(),
                    state.t("LastLabel"),
                    cpu_pct
                ),
                Style::default().fg(theme.accent),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = cpu_block.inner(chunks[1]);
        f.render_widget(cpu_block, chunks[1]);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            100.0,
            Theme::color_for_pct(cpu_pct),
            |s: &ProcessHistorySample| s.cpu_pct,
            None,
        );

        // Memory History
        let mem_block = Block::default()
            .title(Span::styled(
                format!(
                    " {} ({}) · {}: {} ({:.1}%) ",
                    state.t("MemHistory"),
                    state.history_range.label(),
                    state.t("LastLabel"),
                    ByteSize(process.memory_bytes),
                    mem_pct
                ),
                Style::default().fg(theme.accent),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = mem_block.inner(chunks[2]);
        f.render_widget(mem_block, chunks[2]);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            100.0,
            Theme::color_for_pct(mem_pct),
            |s: &ProcessHistorySample| s.mem_pct,
            None,
        );

        // Disk Read History
        let read_label = if read_rate > 0.0 {
            format!(
                " {} ({}) · {}: {}/s ",
                state.t("DiskReadHistory"),
                state.history_range.label(),
                state.t("LastLabel"),
                ByteSize(read_rate as u64)
            )
        } else {
            format!(
                " {} ({}) · {}: — ",
                state.t("DiskReadHistory"),
                state.history_range.label(),
                state.t("LastLabel")
            )
        };
        let read_block = Block::default()
            .title(Span::styled(read_label, Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = read_block.inner(chunks[3]);
        f.render_widget(read_block, chunks[3]);
        let read_max = max_bps_proc(&state.process_history, limit, |s| s.disk_read_bps).max(1.0);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            read_max,
            theme.accent_dim,
            |s: &ProcessHistorySample| s.disk_read_bps,
            None,
        );

        // Disk Write History
        let write_label = if write_rate > 0.0 {
            format!(
                " {} ({}) · {}: {}/s ",
                state.t("DiskWriteHistory"),
                state.history_range.label(),
                state.t("LastLabel"),
                ByteSize(write_rate as u64)
            )
        } else {
            format!(
                " {} ({}) · {}: — ",
                state.t("DiskWriteHistory"),
                state.history_range.label(),
                state.t("LastLabel")
            )
        };
        let write_block = Block::default()
            .title(Span::styled(write_label, Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = write_block.inner(chunks[4]);
        f.render_widget(write_block, chunks[4]);
        let write_max = max_bps_proc(&state.process_history, limit, |s| s.disk_write_bps).max(1.0);
        render_history_canvas_dual(
            f,
            inner_area,
            &samps,
            state.history_range,
            write_max,
            theme.ok,
            |s: &ProcessHistorySample| s.disk_write_bps,
            None,
        );
    } else {
        // CPU Gauge
        let cpu_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(" CPU  {:.1}% ", cpu_pct),
                        Style::default().fg(theme.accent),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent_dim)),
            )
            .gauge_style(
                Style::default()
                    .fg(Theme::color_for_pct(cpu_pct))
                    .bg(Color::Rgb(51, 52, 61)),
            )
            .ratio(cpu_pct / 100.0);
        f.render_widget(cpu_gauge, chunks[1]);

        // Memory Gauge
        let mem_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(
                        format!(
                            " {}  {} ({:.1}%) ",
                            state.t("MemLabel"),
                            ByteSize(process.memory_bytes),
                            mem_pct
                        ),
                        Style::default().fg(theme.accent),
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent_dim)),
            )
            .gauge_style(
                Style::default()
                    .fg(Theme::color_for_pct(mem_pct))
                    .bg(Color::Rgb(51, 52, 61)),
            )
            .ratio(mem_pct / 100.0);
        f.render_widget(mem_gauge, chunks[2]);

        // Disk Read Gauge
        let read_label = if read_rate > 0.0 {
            format!(
                " {}  {}/s ",
                state.t("DiskReadLabel"),
                ByteSize(read_rate as u64)
            )
        } else {
            format!(" {}  — ", state.t("DiskReadLabel"))
        };
        let read_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(read_label, Style::default().fg(theme.accent)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent_dim)),
            )
            .gauge_style(
                Style::default()
                    .fg(theme.accent_dim)
                    .bg(Color::Rgb(51, 52, 61)),
            )
            .ratio((read_rate / 100_000_000.0).clamp(0.0, 1.0));
        f.render_widget(read_gauge, chunks[3]);

        // Disk Write Gauge
        let write_label = if write_rate > 0.0 {
            format!(
                " {}  {}/s ",
                state.t("DiskWriteLabel"),
                ByteSize(write_rate as u64)
            )
        } else {
            format!(" {}  — ", state.t("DiskWriteLabel"))
        };
        let write_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(write_label, Style::default().fg(theme.accent)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent_dim)),
            )
            .gauge_style(Style::default().fg(theme.ok).bg(Color::Rgb(51, 52, 61)))
            .ratio((write_rate / 100_000_000.0).clamp(0.0, 1.0));
        f.render_widget(write_gauge, chunks[4]);
    }

    // Footer hint
    let hint = Line::from(vec![
        Span::styled(
            " [ESC] ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}  ", state.t("Back")),
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
            format!("{} ({})", state.t("Range"), state.history_range.label()),
            Style::default().fg(theme.muted),
        ),
    ]);
    f.render_widget(Paragraph::new(hint), chunks[5]);

    if let Some(db_rect) = db_area {
        render_db_panel(f, db_rect, state, &theme);
    }
}

fn render_db_panel(f: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    use crate::collectors::database::DbConnectionStatus;
    
    let db_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim))
        .title(Span::styled(" Database Dashboard ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)));
        
    let inner_rect = db_block.inner(area);
    f.render_widget(db_block, area);

    let monitor_data = match &state.db_monitor {
        Some(data) => data,
        None => {
            let paragraph = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  [DB Initializing / Disconnected]", Style::default().fg(theme.muted))),
                Line::from(""),
                Line::from("  No metrics collected yet. Checking status..."),
            ]);
            f.render_widget(paragraph, inner_rect);
            return;
        }
    };

    match &monitor_data.status {
        DbConnectionStatus::Disconnected => {
            let paragraph = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  [DB Disconnected]", Style::default().fg(theme.crit))),
                Line::from(""),
                Line::from("  Polling is currently disabled or disconnected."),
            ]);
            f.render_widget(paragraph, inner_rect);
        }
        DbConnectionStatus::Connecting => {
            let paragraph = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  [DB Connecting...]", Style::default().fg(theme.warn))),
                Line::from(""),
                Line::from("  Attempting connection to local instance..."),
            ]);
            f.render_widget(paragraph, inner_rect);
        }
        DbConnectionStatus::AuthRequired(instructions) => {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled("  [Authentication Required]", Style::default().fg(theme.warn).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from("  Authentication failed. Please check credentials:"),
                Line::from(Span::styled(format!("  {}", instructions), Style::default().fg(theme.warn))),
                Line::from(""),
                Line::from("  Set environment variables:"),
                Line::from("  - Postgres: PGUSER, PGPASSWORD"),
                Line::from("  - MySQL: MYSQL_USER, MYSQL_PWD"),
            ];
            let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(paragraph, inner_rect);
        }
        DbConnectionStatus::Error(err_msg) => {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled("  [DB Disconnected]", Style::default().fg(theme.crit).add_modifier(Modifier::BOLD))),
                Line::from(""),
                Line::from(format!("  Error: {}", err_msg)),
                Line::from(""),
                Line::from("  Check if the database service is running locally"),
                Line::from("  and accepts connections on localhost."),
            ];
            let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(paragraph, inner_rect);
        }
        DbConnectionStatus::Connected => {
            let db_type_str = match monitor_data.db_type {
                crate::models::DatabaseType::PostgreSQL => "PostgreSQL",
                crate::models::DatabaseType::MySqlMariaDb => "MySQL / MariaDB",
            };
            
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("  Engine:   ", Style::default().fg(theme.muted)),
                    Span::styled(db_type_str, Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
                    Span::raw("   "),
                    Span::styled("● Active", Style::default().fg(theme.ok)),
                ]),
                Line::from(""),
            ];

            match monitor_data.db_type {
                crate::models::DatabaseType::PostgreSQL => {
                    lines.push(Line::from(vec![
                        Span::styled("  Connections:  ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{} active", monitor_data.metrics.connections_active), Style::default().fg(theme.text)),
                        Span::raw(", "),
                        Span::styled(format!("{} idle", monitor_data.metrics.connections_idle), Style::default().fg(theme.muted)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  Waiting Locks:", Style::default().fg(theme.muted)),
                        Span::styled(
                            format!(" {}", monitor_data.metrics.locks_count),
                            if monitor_data.metrics.locks_count > 0 { Style::default().fg(theme.crit) } else { Style::default().fg(theme.ok) }
                        ),
                    ]));
                    
                    let hit_ratio = monitor_data.metrics.cache_hit_ratio;
                    lines.push(Line::from(vec![
                        Span::styled("  Cache Hit Ratio: ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{:.2}%", hit_ratio), Style::default().fg(if hit_ratio >= 99.0 { theme.ok } else if hit_ratio >= 95.0 { theme.warn } else { theme.crit })),
                    ]));
                    
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled("  Long-Running Queries (>5s):", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD))));
                    if monitor_data.metrics.long_running_queries.is_empty() {
                        lines.push(Line::from(Span::styled("    No queries match criteria", Style::default().fg(theme.muted))));
                    } else {
                        for (pid, query, dur) in &monitor_data.metrics.long_running_queries {
                            let truncated_query = if query.len() > 30 {
                                format!("{}...", &query[0..27])
                            } else {
                                query.clone()
                            };
                            lines.push(Line::from(vec![
                                Span::styled(format!("    PID {}: ", pid), Style::default().fg(theme.muted)),
                                Span::styled(truncated_query, Style::default().fg(theme.warn)),
                                Span::raw(format!(" ({})", dur)),
                            ]));
                        }
                    }
                }
                crate::models::DatabaseType::MySqlMariaDb => {
                    lines.push(Line::from(vec![
                        Span::styled("  Threads:  ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{} running", monitor_data.metrics.threads_running), Style::default().fg(theme.text)),
                        Span::raw(", "),
                        Span::styled(format!("{} connected", monitor_data.metrics.threads_connected), Style::default().fg(theme.muted)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  Slow Queries:", Style::default().fg(theme.muted)),
                        Span::styled(format!(" {}", monitor_data.metrics.slow_queries), Style::default().fg(theme.text)),
                    ]));

                    let read = monitor_data.metrics.read_queries;
                    let write = monitor_data.metrics.write_queries;
                    let total = read + write;
                    let read_pct = if total > 0 { (read as f64 / total as f64) * 100.0 } else { 0.0 };
                    let write_pct = if total > 0 { (write as f64 / total as f64) * 100.0 } else { 0.0 };
                    
                    lines.push(Line::from(vec![
                        Span::styled("  Workload: ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{:.1}% Read", read_pct), Style::default().fg(theme.ok)),
                        Span::raw(" / "),
                        Span::styled(format!("{:.1}% Write", write_pct), Style::default().fg(theme.warn)),
                    ]));

                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("  Buffer Pool Hit Rate: ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{:.2}%", monitor_data.metrics.buffer_pool_hit_rate), Style::default().fg(theme.ok)),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("  Buffer Pool Util:     ", Style::default().fg(theme.muted)),
                        Span::styled(format!("{:.2}%", monitor_data.metrics.buffer_pool_util_pct), Style::default().fg(theme.accent)),
                    ]));
                }
            }
            
            let paragraph = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(paragraph, inner_rect);
        }
    }
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
