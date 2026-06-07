use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::models::ProcessData;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, process: &ProcessData) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Span::styled(
            format!(" Detalle de proceso: {} (PID {}) ", process.name, process.pid),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // info fields
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
            Span::styled("PID:      ", Style::default().fg(theme.muted)),
            Span::styled(process.pid.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Usuario: ", Style::default().fg(theme.muted)),
            Span::styled(process.user.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Threads:  ", Style::default().fg(theme.muted)),
            Span::styled(process.threads.to_string(), Style::default().fg(Color::White)),
            Span::raw("   "),
            Span::styled("Uptime:  ", Style::default().fg(theme.muted)),
            Span::styled(uptime_str, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Estado:   ", Style::default().fg(theme.muted)),
            Span::styled(process.status.to_string_es(), Style::default().fg(Color::Green)),
            Span::raw("   "),
            Span::styled("RAM:     ", Style::default().fg(theme.muted)),
            Span::styled(ByteSize(process.memory_bytes).to_string(), Style::default().fg(Color::White)),
        ]),
    ];
    f.render_widget(Paragraph::new(info_lines), chunks[0]);

    // CPU bar
    let cpu_pct = process.cpu_pct.clamp(0.0, 100.0);
    let cpu_gauge = Gauge::default()
        .block(Block::default().title(Span::styled(
            format!(" CPU  {:.1}% ", cpu_pct),
            Style::default().fg(theme.muted),
        )).borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Theme::color_for_pct(cpu_pct)).bg(Color::DarkGray))
        .ratio(cpu_pct / 100.0);
    f.render_widget(cpu_gauge, chunks[1]);

    // Memory bar
    let mem_pct = process.memory_pct.clamp(0.0, 100.0);
    let mem_gauge = Gauge::default()
        .block(Block::default().title(Span::styled(
            format!(" Memoria  {} ({:.1}%) ", ByteSize(process.memory_bytes), mem_pct),
            Style::default().fg(theme.muted),
        )).borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Theme::color_for_pct(mem_pct)).bg(Color::DarkGray))
        .ratio(mem_pct / 100.0);
    f.render_widget(mem_gauge, chunks[2]);

    // Disk Read bar
    let read_rate = process.disk_read_per_sec.unwrap_or(0.0);
    let read_label = if read_rate > 0.0 {
        format!(" Disco Lectura  {}/s ", ByteSize(read_rate as u64))
    } else {
        " Disco Lectura  — ".to_string()
    };
    let read_ratio = (read_rate / 100_000_000.0).clamp(0.0, 1.0);
    let read_gauge = Gauge::default()
        .block(Block::default().title(Span::styled(read_label, Style::default().fg(theme.muted)))
            .borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Color::Blue).bg(Color::DarkGray))
        .ratio(read_ratio);
    f.render_widget(read_gauge, chunks[3]);

    // Disk Write bar
    let write_rate = process.disk_write_per_sec.unwrap_or(0.0);
    let write_label = if write_rate > 0.0 {
        format!(" Disco Escritura  {}/s ", ByteSize(write_rate as u64))
    } else {
        " Disco Escritura  — ".to_string()
    };
    let write_ratio = (write_rate / 100_000_000.0).clamp(0.0, 1.0);
    let write_gauge = Gauge::default()
        .block(Block::default().title(Span::styled(write_label, Style::default().fg(theme.muted)))
            .borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .ratio(write_ratio);
    f.render_widget(write_gauge, chunks[4]);

    // Footer hint
    let hint = Line::from(vec![
        Span::styled(" [ESC] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Volver", Style::default().fg(theme.muted)),
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
