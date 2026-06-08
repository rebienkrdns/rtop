use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::symbols::Marker;



use crate::ui::history::{HistoryRange, MetricSample};
use crate::ui::theme::Theme;

type MetricExtractor = fn(&MetricSample) -> f64;
type MetricLine = (Color, MetricExtractor);

fn render_history_canvas_dual(
    f: &mut Frame,
    area: Rect,
    samples: &[&MetricSample],
    range: HistoryRange,
    max_val: f64,
    line1: MetricLine,
    line2: Option<MetricLine>,
) {
    let max_samples = range.samples() as f64;
    let s_len = samples.len();

    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(Color::Rgb(51, 52, 61))))
        .x_bounds([0.0, max_samples])
        .y_bounds([0.0, max_val])
        .marker(Marker::Braille)
        .paint(|ctx| {
            if s_len > 1 {
                for i in 0..(s_len - 1) {
                    let x1 = max_samples - (s_len - 1 - i) as f64;
                    let x2 = max_samples - (s_len - 1 - (i + 1)) as f64;
                    if x2 < 0.0 {
                        continue;
                    }
                    let x1_clamped = x1.max(0.0);

                    // Line 1
                    let y1_1 = line1.1(samples[i]);
                    let y2_1 = line1.1(samples[i + 1]);
                    ctx.draw(&CanvasLine {
                        x1: x1_clamped,
                        y1: y1_1,
                        x2,
                        y2: y2_1,
                        color: line1.0,
                    });

                    // Line 2
                    if let Some(ref l2) = line2 {
                        let y1_2 = l2.1(samples[i]);
                        let y2_2 = l2.1(samples[i + 1]);
                        ctx.draw(&CanvasLine {
                            x1: x1_clamped,
                            y1: y1_2,
                            x2,
                            y2: y2_2,
                            color: l2.0,
                        });
                    }
                }
            }
        });
    f.render_widget(canvas, area);
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
            Constraint::Length(spark_height), // CPU canvas
            Constraint::Length(1),            // RAM label
            Constraint::Length(spark_height), // RAM canvas
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

    // CPU Canvas
    let cpu_color = Theme::color_for_pct(cpu_last);
    render_history_canvas_dual(
        f,
        chunks[1],
        samples,
        range,
        100.0,
        (cpu_color, |s| s.cpu_pct),
        None,
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

    // RAM Canvas
    let ram_color = Theme::color_for_pct(mem_last);
    render_history_canvas_dual(
        f,
        chunks[3],
        samples,
        range,
        100.0,
        (ram_color, |s| s.mem_pct),
        None,
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
            Constraint::Length(spark_height), // Disco canvas
            Constraint::Length(1),            // Red label
            Constraint::Length(spark_height), // Red canvas
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

    // Disco Canvas (read as primary, write as secondary)
    let disk_max = max_bps(samples, |s| s.disk_read_bps.max(s.disk_write_bps)).max(1.0);
    render_history_canvas_dual(
        f,
        chunks[1],
        samples,
        range,
        disk_max,
        (theme.disk_fill, |s| s.disk_read_bps),
        Some((Color::Rgb(244, 143, 177), |s| s.disk_write_bps)),
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

    // Red Canvas (recv as primary, sent as secondary)
    let net_max = max_bps(samples, |s| s.net_recv_bps.max(s.net_sent_bps)).max(1.0);
    render_history_canvas_dual(
        f,
        chunks[3],
        samples,
        range,
        net_max,
        (theme.ok, |s| s.net_recv_bps),
        Some((theme.accent, |s| s.net_sent_bps)),
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

    let load_max = max_bps(samples, |s| s.load1).max(1.0);
    render_history_canvas_dual(
        f,
        chunks[1],
        samples,
        range,
        load_max,
        (theme.accent_dim, |s| s.load1),
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn test_sparkline_rendering_direction() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        let s1 = MetricSample {
            cpu_pct: 10.0,
            mem_pct: 10.0,
            load1: 1.0,
            net_recv_bps: 0.0,
            net_sent_bps: 0.0,
            disk_read_bps: 0.0,
            disk_write_bps: 0.0,
        };
        let s2 = MetricSample {
            cpu_pct: 80.0,
            mem_pct: 10.0,
            load1: 1.0,
            net_recv_bps: 0.0,
            net_sent_bps: 0.0,
            disk_read_bps: 0.0,
            disk_write_bps: 0.0,
        };
        let samples = vec![&s1, &s2]; // oldest to newest

        terminal.draw(|f| {
            let area = f.size();
            render_cpu_ram(f, area, &samples, HistoryRange::OneMin);
        }).unwrap();

        let buffer = terminal.backend().buffer();
        for y in 0..5 {
            let mut line = String::new();
            for x in 0..40 {
                line.push_str(buffer.get(x, y).symbol());
            }
            println!("Row {}: {}", y, line);
        }
    }
}

