use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, RenderDirection, Sparkline},
    Frame,
};

use crate::ui::history::{HistoryRange, MetricSample};
use crate::ui::theme::Theme;

fn sparkline_data(
    samples: &[&MetricSample],
    width: usize,
    range: HistoryRange,
    f: impl Fn(&MetricSample) -> f64,
) -> Vec<u64> {
    let max_samples = range.samples();
    let s_len = samples.len();
    if s_len == 0 {
        return vec![0; width];
    }

    let values: Vec<f64> = samples.iter().map(|s| f(s)).collect();
    let t_size = ((s_len as f64 * width as f64) / max_samples as f64).round() as usize;
    let t_size = t_size.clamp(1, width);

    let mut interpolated = Vec::with_capacity(t_size);
    if t_size == 1 {
        interpolated.push(values.last().copied().unwrap_or(0.0) as u64);
    } else if s_len == 1 {
        let val = values[0] as u64;
        interpolated.resize(t_size, val);
    } else {
        for i in 0..t_size {
            let frac = i as f64 / (t_size - 1) as f64;
            let idx = frac * (s_len - 1) as f64;
            let left = idx.floor() as usize;
            let right = idx.ceil() as usize;
            let weight = idx - left as f64;
            let val = (1.0 - weight) * values[left] + weight * values[right];
            interpolated.push(val as u64);
        }
    }

    interpolated.reverse();

    if interpolated.len() < width {
        let pad_len = width - interpolated.len();
        interpolated.extend(vec![0; pad_len]);
    }

    interpolated
}

fn max_bps(samples: &[&MetricSample], f: impl Fn(&MetricSample) -> f64) -> f64 {
    samples
        .iter()
        .map(|s| f(s))
        .fold(0.0_f64, f64::max)
}

pub fn render_cpu_ram(f: &mut Frame, area: Rect, samples: &[&MetricSample], range: HistoryRange) {
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
    let width = chunks[1].width as usize;
    let cpu_data = sparkline_data(samples, width, range, |s| s.cpu_pct);
    let cpu_color = Theme::color_for_pct(cpu_last);
    f.render_widget(
        Sparkline::default()
            .data(&cpu_data)
            .max(100)
            .direction(RenderDirection::RightToLeft)
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
    let width = chunks[3].width as usize;
    let ram_data = sparkline_data(samples, width, range, |s| s.mem_pct);
    let ram_color = Theme::color_for_pct(mem_last);
    f.render_widget(
        Sparkline::default()
            .data(&ram_data)
            .max(100)
            .direction(RenderDirection::RightToLeft)
            .style(Style::default().fg(ram_color).bg(Color::Rgb(51, 52, 61))),
        chunks[3],
    );
}

pub fn render_disk_net(f: &mut Frame, area: Rect, samples: &[&MetricSample], range: HistoryRange) {
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
    let width = chunks[1].width as usize;
    let disk_max = max_bps(samples, |s| s.disk_read_bps.max(s.disk_write_bps)).max(1.0);
    let disk_data = sparkline_data(samples, width, range, |s| s.disk_read_bps);
    f.render_widget(
        Sparkline::default()
            .data(&disk_data)
            .max(disk_max as u64)
            .direction(RenderDirection::RightToLeft)
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
    let width = chunks[3].width as usize;
    let net_max = max_bps(samples, |s| s.net_recv_bps.max(s.net_sent_bps)).max(1.0);
    let net_data = sparkline_data(samples, width, range, |s| s.net_recv_bps);
    f.render_widget(
        Sparkline::default()
            .data(&net_data)
            .max(net_max as u64)
            .direction(RenderDirection::RightToLeft)
            .style(Style::default().fg(theme.ok).bg(Color::Rgb(51, 52, 61))),
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

    let width = chunks[1].width as usize;
    let load_max = max_bps(samples, |s| s.load1).max(1.0);

    let load_data = sparkline_data(samples, width, range, |s| s.load1 * 10.0); // x10 para mejor resolución
    f.render_widget(
        Sparkline::default()
            .data(&load_data)
            .max((load_max * 10.0) as u64)
            .direction(RenderDirection::RightToLeft)
            .style(
                Style::default()
                    .fg(theme.accent_dim)
                    .bg(Color::Rgb(51, 52, 61)),
            ),
        chunks[1],
    );
}
