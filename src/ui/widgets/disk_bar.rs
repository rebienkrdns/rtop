use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::models::DiskData;
use crate::ui::theme::Theme;

fn fmt_rate(bps: Option<f64>) -> String {
    match bps {
        Some(value) => format!("{}/s", ByteSize(value as u64)),
        None => "N/D".to_string(),
    }
}

pub fn render(f: &mut Frame, area: Rect, disk: &DiskData) {
    if area.height < 2 {
        return;
    }

    let theme = Theme::default_theme();
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
    let header_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .split(chunks[0]);

    let name_str = if disk.mount_point.is_empty() {
        format!("Disco  {}", disk.device)
    } else {
        format!("Disco  {} ({})", disk.device, disk.mount_point)
    };
    f.render_widget(
        Paragraph::new(name_str).style(Style::default().fg(theme.accent)),
        header_cols[0],
    );

    let usage_color = Theme::color_for_pct(disk.usage_pct);
    let usage_str = format!("{:.0}%", disk.usage_pct);
    f.render_widget(
        Paragraph::new(usage_str)
            .style(
                Style::default()
                    .fg(usage_color)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(ratatui::layout::Alignment::Right),
        header_cols[1],
    );

    // Línea 2: barra de uso — relleno coral (Obsidian tertiary-fixed-dim)
    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(theme.disk_fill)
                .bg(ratatui::style::Color::Rgb(51, 52, 61)), // #33343d
        )
        .ratio((disk.usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, chunks[1]);

    // Línea 3: tasas R/W + hint F2
    if has_io_row {
        let io_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(14)])
            .split(chunks[2]);

        let write_str = fmt_rate(disk.write_bytes_per_sec);
        let read_str = fmt_rate(disk.read_bytes_per_sec);

        let io_line = Line::from(vec![
            Span::styled(
                "↑ Escritura ",
                Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
            ),
            Span::styled(write_str, Style::default().fg(theme.ok)),
            Span::raw("     "),
            Span::styled(
                "↓ Lectura ",
                Style::default()
                    .fg(theme.accent_dim)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(read_str, Style::default().fg(theme.accent_dim)),
        ]);
        f.render_widget(Paragraph::new(io_line), io_cols[0]);

        let hint = Paragraph::new("[ F2 cambiar ]")
            .style(Style::default().fg(theme.muted))
            .alignment(ratatui::layout::Alignment::Right);
        f.render_widget(hint, io_cols[1]);
    }
}
