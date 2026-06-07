use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::models::DiskData;
use crate::ui::theme::Theme;

fn fmt_rate(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}

pub fn render(f: &mut Frame, area: Rect, disk: &DiskData) {
    if area.height < 2 {
        return;
    }

    let has_io_row = area.height >= 3;

    let chunks = if has_io_row {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area)
    };

    // Línea 1: encabezado
    let label = Line::from(vec![
        Span::styled(disk.mount_point.as_str(), Style::default().fg(Color::Cyan)),
        Span::raw(format!(
            "  {:.1}%  {} / {}",
            disk.usage_pct,
            ByteSize(disk.used_bytes),
            ByteSize(disk.total_bytes),
        )),
    ]);
    f.render_widget(Paragraph::new(label), chunks[0]);

    // Línea 2: barra de uso
    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(Theme::color_for_pct(disk.usage_pct))
                .bg(Color::DarkGray),
        )
        .ratio((disk.usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, chunks[1]);

    // Línea 3: tasas R/W + hint F2
    if has_io_row {
        let write_str = fmt_rate(disk.write_bytes_per_sec);
        let read_str = fmt_rate(disk.read_bytes_per_sec);
        let io_line = Line::from(vec![
            Span::styled("↑ ", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
            Span::styled(write_str, Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled("↓ ", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
            Span::styled(read_str, Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled("[ F2 cambiar ]", Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(io_line), chunks[2]);
    }
}
