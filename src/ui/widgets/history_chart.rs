use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Sparkline},
    Frame,
};

use crate::ui::history::{HistoryRange, MetricSample};
use crate::ui::theme::Theme;

fn sparkline_data(samples: &[&MetricSample], f: impl Fn(&MetricSample) -> f64) -> Vec<u64> {
    samples.iter().map(|s| f(s) as u64).collect()
}

fn max_bps(samples: &[&MetricSample], f: impl Fn(&MetricSample) -> f64) -> f64 {
    samples.iter().map(|s| f(s)).fold(0.0_f64, f64::max)
}

pub fn render_cpu_ram(
    f: &mut Frame,
    area: Rect,
    samples: &[&MetricSample],
    range: HistoryRange,
) {
    if area.height < 4 {
        return;
    }
    let theme = Theme::default_theme();

    let spark_height = (area.height - 2) / 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),            // CPU label
            Constraint::Length(spark_height), // CPU sparkline
            Constraint::Length(1),            // RAM label
            Constraint::Length(spark_height), // RAM sparkline
            Constraint::Min(0),               // spacer/range
        ])
        .split(area);

    let cpu_last = samples.last().map(|s| s.cpu_pct).unwrap_or(0.0);
    let mem_last = samples.last().map(|s| s.mem_pct).unwrap_or(0.0);

    // CPU label
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("CPU", Style::default().fg(theme.accent)),
            Span::styled(
                format!("  {:.1}%  [h] historial · [t] {}", cpu_last, range.label()),
                Style::default().fg(theme.muted),
            ),
        ])),
        chunks[0],
    );

    // CPU sparkline
    let cpu_data = sparkline_data(samples, |s| s.cpu_pct);
    let cpu_color = Theme::color_for_pct(cpu_last);
    f.render_widget(
        Sparkline::default()
            .data(&cpu_data)
            .max(100)
            .style(Style::default().fg(cpu_color).bg(Color::Rgb(51, 52, 61))),
        chunks[1],
    );

    // RAM label
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("RAM", Style::default().fg(theme.accent)),
            Span::styled(
                format!("  {:.1}%", mem_last),
                Style::default().fg(theme.muted),
            ),
        ])),
        chunks[2],
    );

    // RAM sparkline
    let ram_data = sparkline_data(samples, |s| s.mem_pct);
    let ram_color = Theme::color_for_pct(mem_last);
    f.render_widget(
        Sparkline::default()
            .data(&ram_data)
            .max(100)
            .style(Style::default().fg(ram_color).bg(Color::Rgb(51, 52, 61))),
        chunks[3],
    );
}

pub fn render_disk_net(
    f: &mut Frame,
    area: Rect,
    samples: &[&MetricSample],
    _range: HistoryRange,
) {
    if area.height < 4 {
        return;
    }
    let theme = Theme::default_theme();

    let spark_height = (area.height - 2) / 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),            // Disco label
            Constraint::Length(spark_height), // Disco sparkline (read)
            Constraint::Length(1),            // Red label
            Constraint::Length(spark_height), // Red sparkline (recv)
            Constraint::Min(0),
        ])
        .split(area);

    let disk_read_last = samples.last().map(|s| s.disk_read_bps).unwrap_or(0.0);
    let disk_write_last = samples.last().map(|s| s.disk_write_bps).unwrap_or(0.0);
    let net_recv_last = samples.last().map(|s| s.net_recv_bps).unwrap_or(0.0);
    let net_sent_last = samples.last().map(|s| s.net_sent_bps).unwrap_or(0.0);

    // Disco label
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Disco", Style::default().fg(theme.accent)),
            Span::styled(
                format!(
                    "  ↑{}/s  ↓{}/s",
                    ByteSize(disk_write_last as u64),
                    ByteSize(disk_read_last as u64)
                ),
                Style::default().fg(theme.muted),
            ),
        ])),
        chunks[0],
    );

    // Disco sparkline (lectura)
    let disk_max = max_bps(samples, |s| s.disk_read_bps.max(s.disk_write_bps)).max(1.0);
    let disk_data = sparkline_data(samples, |s| s.disk_read_bps);
    f.render_widget(
        Sparkline::default()
            .data(&disk_data)
            .max(disk_max as u64)
            .style(
                Style::default()
                    .fg(theme.disk_fill)
                    .bg(Color::Rgb(51, 52, 61)),
            ),
        chunks[1],
    );

    // Red label
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Red", Style::default().fg(theme.accent)),
            Span::styled(
                format!(
                    "  ↓{}/s  ↑{}/s",
                    ByteSize(net_recv_last as u64),
                    ByteSize(net_sent_last as u64)
                ),
                Style::default().fg(theme.muted),
            ),
        ])),
        chunks[2],
    );

    // Red sparkline (entrada)
    let net_max = max_bps(samples, |s| s.net_recv_bps.max(s.net_sent_bps)).max(1.0);
    let net_data = sparkline_data(samples, |s| s.net_recv_bps);
    f.render_widget(
        Sparkline::default()
            .data(&net_data)
            .max(net_max as u64)
            .style(
                Style::default()
                    .fg(theme.ok)
                    .bg(Color::Rgb(51, 52, 61)),
            ),
        chunks[3],
    );
}

#[allow(dead_code)]
pub fn render_load(f: &mut Frame, area: Rect, samples: &[&MetricSample], range: HistoryRange) {
    if area.height < 2 {
        return;
    }
    let theme = Theme::default_theme();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    let load_last = samples.last().map(|s| s.load1).unwrap_or(0.0);
    let load_max = samples.iter().map(|s| s.load1).fold(0.0_f64, f64::max).max(1.0);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Load", Style::default().fg(theme.accent)),
            Span::styled(
                format!("  {:.2}  [{}]", load_last, range.label()),
                Style::default().fg(theme.muted).add_modifier(Modifier::DIM),
            ),
        ])),
        chunks[0],
    );

    let load_data = sparkline_data(samples, |s| s.load1 * 10.0); // x10 para mejor resolución
    f.render_widget(
        Sparkline::default()
            .data(&load_data)
            .max((load_max * 10.0) as u64)
            .style(
                Style::default()
                    .fg(theme.accent_dim)
                    .bg(Color::Rgb(51, 52, 61)),
            ),
        chunks[1],
    );
}
