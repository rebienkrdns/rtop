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

pub fn render(f: &mut Frame, area: Rect, process: &ProcessData, state: &AppState) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Span::styled(
            format!(" Detalle de proceso: {} (PID {}) ", process.name, process.pid),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // info fields
            Constraint::Length(3), // CPU bar
            Constraint::Length(3), // Memory bar
            Constraint::Length(3), // Disk Read bar
            Constraint::Length(3), // Disk Write bar
            Constraint::Length(2), // footer hint
            Constraint::Min(0),
        ])
        .split(inner);

    // Info fields
    let uptime_str = format_uptime(process.uptime_secs);
    let info_lines = vec![
        Line::from(vec![
            Span::styled("PID:        ", Style::default().fg(theme.muted)),
            Span::styled(process.pid.to_string(), Style::default().fg(theme.text).add_modifier(Modifier::BOLD)),
            Span::raw("    "),
            Span::styled("Usuario: ", Style::default().fg(theme.muted)),
            Span::styled(process.user.clone(), Style::default().fg(theme.text)),
            Span::raw("    "),
            Span::styled("Estado: ", Style::default().fg(theme.muted)),
            Span::styled(process.status.to_string_es(), Style::default().fg(theme.ok)),
            Span::raw("    "),
            Span::styled("Threads: ", Style::default().fg(theme.muted)),
            Span::styled(process.threads.to_string(), Style::default().fg(theme.text)),
            Span::raw("    "),
            Span::styled("Uptime: ", Style::default().fg(theme.muted)),
            Span::styled(uptime_str, Style::default().fg(theme.text)),
            Span::raw("    "),
            Span::styled("RAM: ", Style::default().fg(theme.muted)),
            Span::styled(ByteSize(process.memory_bytes).to_string(), Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("Ruta (Exe): ", Style::default().fg(theme.muted)),
            Span::styled(process.exe_path.clone(), Style::default().fg(theme.accent)),
        ]),
        Line::from(vec![
            Span::styled("Directorio: ", Style::default().fg(theme.muted)),
            Span::styled(process.cwd.clone(), Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("Comando:    ", Style::default().fg(theme.muted)),
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
        let cpu_data: Vec<u64> = state.process_history.iter()
            .skip(state.process_history.len().saturating_sub(state.history_range.samples()))
            .map(|s| s.cpu_pct as u64)
            .collect();
        let cpu_block = Block::default()
            .title(Span::styled(
                format!(" CPU Historial ({}) · Último: {:.1}% ", state.history_range.label(), cpu_pct),
                Style::default().fg(theme.accent),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = cpu_block.inner(chunks[1]);
        f.render_widget(cpu_block, chunks[1]);
        let cpu_spark = Sparkline::default()
            .data(&cpu_data)
            .max(100)
            .style(Style::default().fg(Theme::color_for_pct(cpu_pct)).bg(Color::Rgb(51, 52, 61)));
        f.render_widget(cpu_spark, inner_area);
    } else {
        let cpu_gauge = Gauge::default()
            .block(Block::default().title(Span::styled(
                format!(" CPU  {:.1}% ", cpu_pct),
                Style::default().fg(theme.accent),
            )).borders(Borders::ALL).border_style(Style::default().fg(theme.accent_dim)))
            .gauge_style(Style::default().fg(Theme::color_for_pct(cpu_pct)).bg(Color::Rgb(51, 52, 61)))
            .ratio(cpu_pct / 100.0);
        f.render_widget(cpu_gauge, chunks[1]);
    }

    // Memory bar / history
    let mem_pct = process.memory_pct.clamp(0.0, 100.0);
    if state.history_mode {
        let mem_data: Vec<u64> = state.process_history.iter()
            .skip(state.process_history.len().saturating_sub(state.history_range.samples()))
            .map(|s| s.mem_pct as u64)
            .collect();
        let mem_block = Block::default()
            .title(Span::styled(
                format!(" Memoria Historial ({}) · Último: {} ({:.1}%) ", state.history_range.label(), ByteSize(process.memory_bytes), mem_pct),
                Style::default().fg(theme.accent),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = mem_block.inner(chunks[2]);
        f.render_widget(mem_block, chunks[2]);
        let mem_spark = Sparkline::default()
            .data(&mem_data)
            .max(100)
            .style(Style::default().fg(Theme::color_for_pct(mem_pct)).bg(Color::Rgb(51, 52, 61)));
        f.render_widget(mem_spark, inner_area);
    } else {
        let mem_gauge = Gauge::default()
            .block(Block::default().title(Span::styled(
                format!(" Memoria  {} ({:.1}%) ", ByteSize(process.memory_bytes), mem_pct),
                Style::default().fg(theme.accent),
            )).borders(Borders::ALL).border_style(Style::default().fg(theme.accent_dim)))
            .gauge_style(Style::default().fg(Theme::color_for_pct(mem_pct)).bg(Color::Rgb(51, 52, 61)))
            .ratio(mem_pct / 100.0);
        f.render_widget(mem_gauge, chunks[2]);
    }

    // Disk Read bar / history
    let read_rate = process.disk_read_per_sec.unwrap_or(0.0);
    if state.history_mode {
        let read_data: Vec<u64> = state.process_history.iter()
            .skip(state.process_history.len().saturating_sub(state.history_range.samples()))
            .map(|s| s.disk_read_bps as u64)
            .collect();
        let read_max = state.process_history.iter()
            .skip(state.process_history.len().saturating_sub(state.history_range.samples()))
            .map(|s| s.disk_read_bps)
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let read_label = if read_rate > 0.0 {
            format!(" Disco Lectura Historial ({}) · Último: {}/s ", state.history_range.label(), ByteSize(read_rate as u64))
        } else {
            format!(" Disco Lectura Historial ({}) · Último: — ", state.history_range.label())
        };
        let read_block = Block::default()
            .title(Span::styled(read_label, Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = read_block.inner(chunks[3]);
        f.render_widget(read_block, chunks[3]);
        let read_spark = Sparkline::default()
            .data(&read_data)
            .max(read_max as u64)
            .style(Style::default().fg(theme.accent_dim).bg(Color::Rgb(51, 52, 61)));
        f.render_widget(read_spark, inner_area);
    } else {
        let read_label = if read_rate > 0.0 {
            format!(" Disco Lectura  {}/s ", ByteSize(read_rate as u64))
        } else {
            " Disco Lectura  — ".to_string()
        };
        let read_ratio = (read_rate / 100_000_000.0).clamp(0.0, 1.0);
        let read_gauge = Gauge::default()
            .block(Block::default().title(Span::styled(read_label, Style::default().fg(theme.accent)))
                .borders(Borders::ALL).border_style(Style::default().fg(theme.accent_dim)))
            .gauge_style(Style::default().fg(theme.accent_dim).bg(Color::Rgb(51, 52, 61)))
            .ratio(read_ratio);
        f.render_widget(read_gauge, chunks[3]);
    }

    // Disk Write bar / history
    let write_rate = process.disk_write_per_sec.unwrap_or(0.0);
    if state.history_mode {
        let write_data: Vec<u64> = state.process_history.iter()
            .skip(state.process_history.len().saturating_sub(state.history_range.samples()))
            .map(|s| s.disk_write_bps as u64)
            .collect();
        let write_max = state.process_history.iter()
            .skip(state.process_history.len().saturating_sub(state.history_range.samples()))
            .map(|s| s.disk_write_bps)
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let write_label = if write_rate > 0.0 {
            format!(" Disco Escritura Historial ({}) · Último: {}/s ", state.history_range.label(), ByteSize(write_rate as u64))
        } else {
            format!(" Disco Escritura Historial ({}) · Último: — ", state.history_range.label())
        };
        let write_block = Block::default()
            .title(Span::styled(write_label, Style::default().fg(theme.accent)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_dim));
        let inner_area = write_block.inner(chunks[4]);
        f.render_widget(write_block, chunks[4]);
        let write_spark = Sparkline::default()
            .data(&write_data)
            .max(write_max as u64)
            .style(Style::default().fg(theme.ok).bg(Color::Rgb(51, 52, 61)));
        f.render_widget(write_spark, inner_area);
    } else {
        let write_label = if write_rate > 0.0 {
            format!(" Disco Escritura  {}/s ", ByteSize(write_rate as u64))
        } else {
            " Disco Escritura  — ".to_string()
        };
        let write_ratio = (write_rate / 100_000_000.0).clamp(0.0, 1.0);
        let write_gauge = Gauge::default()
            .block(Block::default().title(Span::styled(write_label, Style::default().fg(theme.accent)))
                .borders(Borders::ALL).border_style(Style::default().fg(theme.accent_dim)))
            .gauge_style(Style::default().fg(theme.ok).bg(Color::Rgb(51, 52, 61)))
            .ratio(write_ratio);
        f.render_widget(write_gauge, chunks[4]);
    }

    // Footer hint
    let hint = Line::from(vec![
        Span::styled(" [ESC] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Volver  ", Style::default().fg(theme.muted)),
        Span::styled("[H] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Historial  ", Style::default().fg(theme.muted)),
        Span::styled("[T] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(format!("Rango ({})", state.history_range.label()), Style::default().fg(theme.muted)),
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
