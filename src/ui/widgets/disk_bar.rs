use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::localization::{translate, Language};
use crate::models::DiskData;
use crate::ui::theme::Theme;

fn fmt_rate(bps: Option<f64>) -> String {
    match bps {
        Some(value) => format!("{}/s", ByteSize(value as u64)),
        None => "N/A".to_string(),
    }
}

fn fmt_latency(ms: Option<f64>) -> String {
    match ms {
        Some(v) if v < 1.0 => format!("{:.2}ms", v),
        Some(v) => format!("{:.1}ms", v),
        None => "—".to_string(),
    }
}

pub fn render(f: &mut Frame, area: Rect, disk: &DiskData, lang: Language) {
    if area.height < 2 {
        return;
    }

    let theme = Theme::default_theme();
    let has_io_row = area.height >= 3;
    let has_latency_row = area.height >= 4 && disk.read_latency_ms.is_some();

    let mut constraints = vec![
        Constraint::Length(1), // encabezado
        Constraint::Length(1), // barra de uso
    ];
    if has_io_row {
        constraints.push(Constraint::Length(1));
    }
    if has_latency_row {
        constraints.push(Constraint::Length(1));
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Línea 0: encabezado
    let header_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .split(chunks[0]);

    let disk_label = translate("Disk", lang);
    let size_info = if disk.total_bytes > 0 {
        let free = disk.total_bytes.saturating_sub(disk.used_bytes);
        format!(
            "  [{} / {}  free {}]",
            ByteSize(disk.used_bytes),
            ByteSize(disk.total_bytes),
            ByteSize(free),
        )
    } else {
        String::new()
    };
    let name_str = if disk.mount_point.is_empty() {
        format!("{}  {}{}", disk_label, disk.device, size_info)
    } else {
        format!(
            "{}  {} ({}){}",
            disk_label, disk.device, disk.mount_point, size_info
        )
    };
    f.render_widget(
        Paragraph::new(name_str).style(Style::default().fg(theme.accent)),
        header_cols[0],
    );

    let usage_color = Theme::color_for_pct(disk.usage_pct);
    let alert = disk.io_util_pct.unwrap_or(0.0) >= 90.0 || disk.usage_pct >= 90.0;
    let usage_str = if alert {
        format!("{:.0}%⚠", disk.usage_pct)
    } else {
        format!("{:.0}%", disk.usage_pct)
    };
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

    // Línea 1: barra de uso
    let gauge = Gauge::default()
        .gauge_style(
            Style::default()
                .fg(theme.disk_fill)
                .bg(ratatui::style::Color::Rgb(51, 52, 61)),
        )
        .ratio((disk.usage_pct / 100.0).clamp(0.0, 1.0))
        .label("");
    f.render_widget(gauge, chunks[1]);

    // Línea 2: tasas R/W
    if has_io_row {
        let io_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(14)])
            .split(chunks[2]);

        let write_str = fmt_rate(disk.write_bytes_per_sec);
        let read_str = fmt_rate(disk.read_bytes_per_sec);

        let io_line = Line::from(vec![
            Span::styled(
                "↑ W ",
                Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
            ),
            Span::styled(write_str, Style::default().fg(theme.ok)),
            Span::raw("  "),
            Span::styled(
                "↓ R ",
                Style::default()
                    .fg(theme.accent_dim)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(read_str, Style::default().fg(theme.accent_dim)),
        ]);
        f.render_widget(Paragraph::new(io_line), io_cols[0]);

        let hint = Paragraph::new(format!("[ F2 {} ]", translate("Change tab", lang)))
            .style(Style::default().fg(theme.muted))
            .alignment(ratatui::layout::Alignment::Right);
        f.render_widget(hint, io_cols[1]);
    }

    // Línea 3: latencia R/W + util%
    if has_latency_row {
        let r_lat = fmt_latency(disk.read_latency_ms);
        let w_lat = fmt_latency(disk.write_latency_ms);
        let util = disk
            .io_util_pct
            .map(|v| format!("{:.0}%", v))
            .unwrap_or_else(|| "—".to_string());
        let util_color = Theme::color_for_pct(disk.io_util_pct.unwrap_or(0.0));

        let lat_line = Line::from(vec![
            Span::styled("Lat ", Style::default().fg(theme.muted)),
            Span::styled("R:", Style::default().fg(theme.accent_dim)),
            Span::styled(r_lat, Style::default().fg(theme.text)),
            Span::styled(" W:", Style::default().fg(theme.ok)),
            Span::styled(w_lat, Style::default().fg(theme.text)),
            Span::styled("  util:", Style::default().fg(theme.muted)),
            Span::styled(
                util,
                Style::default().fg(util_color).add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(Paragraph::new(lat_line), chunks[3]);
    }
}
