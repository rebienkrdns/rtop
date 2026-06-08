use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Frame,
};

use crate::app::AppState;
use crate::models::ProcessData;
use crate::ui::theme::Theme;

fn prepare_sparkline_data<T>(
    history: &std::collections::VecDeque<T>,
    limit: usize,
    width: usize,
    f: impl Fn(&T) -> u64,
) -> Vec<u64> {
    let display_samples = limit.min(width);
    let mut data: Vec<u64> = history
        .iter()
        .skip(history.len().saturating_sub(display_samples))
        .map(f)
        .collect();
    if data.len() < width {
        let mut padded = vec![0; width - data.len()];
        padded.extend(data);
        data = padded;
    }
    data
}

fn calculate_max<T>(
    history: &std::collections::VecDeque<T>,
    limit: usize,
    width: usize,
    f: impl Fn(&T) -> f64,
) -> f64 {
    let display_samples = limit.min(width);
    history
        .iter()
        .skip(history.len().saturating_sub(display_samples))
        .map(f)
        .fold(0.0_f64, f64::max)
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

    let inner_height = inner.height;
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
        .split(inner);

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

    // CPU bar / history
    let cpu_pct = process.cpu_pct.clamp(0.0, 100.0);
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
                Style::default().fg(theme.accent),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = cpu_block.inner(chunks[1]);
        f.render_widget(cpu_block, chunks[1]);
        let width = inner_area.width as usize;
        let cpu_data = prepare_sparkline_data(
            &state.process_history,
            state.history_range.samples(),
            width,
            |s| s.cpu_pct as u64,
        );
        let cpu_spark = Sparkline::default().data(&cpu_data).max(100).style(
            Style::default()
                .fg(Theme::color_for_pct(cpu_pct))
                .bg(Color::Rgb(51, 52, 61)),
        );
        f.render_widget(cpu_spark, inner_area);
    } else {
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
    }

    // Memory bar / history
    let mem_pct = process.memory_pct.clamp(0.0, 100.0);
    if state.history_mode {
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
        let width = inner_area.width as usize;
        let mem_data = prepare_sparkline_data(
            &state.process_history,
            state.history_range.samples(),
            width,
            |s| s.mem_pct as u64,
        );
        let mem_spark = Sparkline::default().data(&mem_data).max(100).style(
            Style::default()
                .fg(Theme::color_for_pct(mem_pct))
                .bg(Color::Rgb(51, 52, 61)),
        );
        f.render_widget(mem_spark, inner_area);
    } else {
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
    }

    // Disk Read bar / history
    let read_rate = process.disk_read_per_sec.unwrap_or(0.0);
    if state.history_mode {
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
        let width = inner_area.width as usize;
        let read_data = prepare_sparkline_data(
            &state.process_history,
            state.history_range.samples(),
            width,
            |s| s.disk_read_bps as u64,
        );
        let read_max = calculate_max(
            &state.process_history,
            state.history_range.samples(),
            width,
            |s| s.disk_read_bps,
        ).max(1.0);
        let read_spark = Sparkline::default()
            .data(&read_data)
            .max(read_max as u64)
            .style(
                Style::default()
                    .fg(theme.accent_dim)
                    .bg(Color::Rgb(51, 52, 61)),
            );
        f.render_widget(read_spark, inner_area);
    } else {
        let read_label = if read_rate > 0.0 {
            format!(
                " {}  {}/s ",
                state.t("DiskReadLabel"),
                ByteSize(read_rate as u64)
            )
        } else {
            format!(" {}  — ", state.t("DiskReadLabel"))
        };
        let read_ratio = (read_rate / 100_000_000.0).clamp(0.0, 1.0);
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
            .ratio(read_ratio);
        f.render_widget(read_gauge, chunks[3]);
    }

    // Disk Write bar / history
    let write_rate = process.disk_write_per_sec.unwrap_or(0.0);
    if state.history_mode {
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
        let width = inner_area.width as usize;
        let write_data = prepare_sparkline_data(
            &state.process_history,
            state.history_range.samples(),
            width,
            |s| s.disk_write_bps as u64,
        );
        let write_max = calculate_max(
            &state.process_history,
            state.history_range.samples(),
            width,
            |s| s.disk_write_bps,
        ).max(1.0);
        let write_spark = Sparkline::default()
            .data(&write_data)
            .max(write_max as u64)
            .style(Style::default().fg(theme.ok).bg(Color::Rgb(51, 52, 61)));
        f.render_widget(write_spark, inner_area);
    } else {
        let write_label = if write_rate > 0.0 {
            format!(
                " {}  {}/s ",
                state.t("DiskWriteLabel"),
                ByteSize(write_rate as u64)
            )
        } else {
            format!(" {}  — ", state.t("DiskWriteLabel"))
        };
        let write_ratio = (write_rate / 100_000_000.0).clamp(0.0, 1.0);
        let write_gauge = Gauge::default()
            .block(
                Block::default()
                    .title(Span::styled(write_label, Style::default().fg(theme.accent)))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent_dim)),
            )
            .gauge_style(Style::default().fg(theme.ok).bg(Color::Rgb(51, 52, 61)))
            .ratio(write_ratio);
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
