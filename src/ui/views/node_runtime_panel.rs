use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{canvas::Canvas, Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::collectors::node_runtime::{NodeConnectionStatus, NodeMonitorData};
use crate::ui::theme::Theme;

/// Renderiza el panel lateral de telemetría V8 / Node.js runtime.
pub fn render_node_panel(f: &mut Frame, area: Rect, state: &AppState, runtime_label: &str) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Span::styled(
            format!(" {} V8 Engine Telemetry ", runtime_label),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let monitor = match &state.node_monitor {
        Some(m) => m,
        None => {
            let p = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  [Node.js Inspector] Initializing...",
                    Style::default().fg(theme.muted),
                )),
            ]);
            f.render_widget(p, inner);
            return;
        }
    };

    match &monitor.status {
        NodeConnectionStatus::Connecting => {
            let p = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  [{}] Connecting to V8 Inspector...", runtime_label),
                    Style::default().fg(theme.warn),
                )),
            ]);
            f.render_widget(p, inner);
        }
        NodeConnectionStatus::Unavailable => {
            render_unavailable(f, inner, runtime_label, &theme);
        }
        NodeConnectionStatus::Disconnected => {
            render_unavailable(f, inner, runtime_label, &theme);
        }
        NodeConnectionStatus::Connected => {
            render_connected(f, inner, monitor, state, &theme);
        }
    }
}

fn render_unavailable(f: &mut Frame, area: Rect, runtime_label: &str, theme: &Theme) {
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  [{}] V8 Inspector Unavailable", runtime_label),
            Style::default()
                .fg(theme.crit)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Start with: node --inspect app.js",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "  or: NODE_OPTIONS=--inspect",
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Ports probed: 9229-9239",
            Style::default().fg(theme.muted),
        )),
    ];
    let p = Paragraph::new(lines);
    f.render_widget(p, area);
}

fn render_connected(
    f: &mut Frame,
    area: Rect,
    monitor: &NodeMonitorData,
    state: &AppState,
    theme: &Theme,
) {
    let m = &monitor.metrics;
    let elu = m.event_loop.utilization_pct;
    let delay = m.event_loop.delay_ms;
    let heap_pct = m.heap.used_pct();
    let major_gc_cont = m.heap.major_gc_rate > 0.5 && heap_pct > 85.0;

    // Alertas
    let elu_alert = elu > 85.0 || delay > 50.0;
    let oom_risk = major_gc_cont;

    if state.history_mode {
        render_history_mode(f, area, monitor, state, theme, elu_alert, oom_risk);
    } else {
        render_live_mode(f, area, monitor, theme, elu_alert, oom_risk);
    }
}

fn render_live_mode(
    f: &mut Frame,
    area: Rect,
    monitor: &NodeMonitorData,
    theme: &Theme,
    elu_alert: bool,
    oom_risk: bool,
) {
    let m = &monitor.metrics;
    let elu = m.event_loop.utilization_pct;
    let delay = m.event_loop.delay_ms;
    let heap_pct = m.heap.used_pct();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // ELU gauge
            Constraint::Length(3), // Heap gauge
            Constraint::Length(1), // separador
            Constraint::Min(1),    // detalles de texto
        ])
        .split(area);

    // ELU gauge
    let elu_color = if elu > 85.0 {
        Color::Red
    } else if elu > 60.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    let elu_label = if elu_alert {
        format!(" ELU {:.1}%  ⚠ BLOCKED  delay: {:.1}ms ", elu, delay)
    } else {
        format!(" ELU {:.1}%  delay: {:.1}ms ", elu, delay)
    };
    let elu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " Event Loop ",
            Style::default().fg(elu_color).add_modifier(Modifier::BOLD),
        )))
        .gauge_style(Style::default().fg(elu_color))
        .ratio((elu / 100.0).clamp(0.0, 1.0))
        .label(elu_label);
    f.render_widget(elu_gauge, chunks[0]);

    // Heap gauge
    let heap_color = if heap_pct > 85.0 {
        Color::Red
    } else if heap_pct > 70.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    let heap_label = if oom_risk {
        format!(
            " {:.0}% ({} / {})  [OOM RISK] ",
            heap_pct,
            format_bytes(m.heap.used_bytes),
            format_bytes(m.heap.total_bytes)
        )
    } else {
        format!(
            " {:.0}% ({} / {}) ",
            heap_pct,
            format_bytes(m.heap.used_bytes),
            format_bytes(m.heap.total_bytes)
        )
    };
    let heap_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " V8 Heap ",
            Style::default()
                .fg(heap_color)
                .add_modifier(Modifier::BOLD),
        )))
        .gauge_style(Style::default().fg(heap_color))
        .ratio((heap_pct / 100.0).clamp(0.0, 1.0))
        .label(heap_label);
    f.render_widget(heap_gauge, chunks[1]);

    // Detalles de texto
    let detail_lines = build_detail_lines(m, theme, elu_alert, oom_risk);
    let p = Paragraph::new(detail_lines);
    f.render_widget(p, chunks[3]);
}

fn render_history_mode(
    f: &mut Frame,
    area: Rect,
    monitor: &NodeMonitorData,
    _state: &AppState,
    theme: &Theme,
    elu_alert: bool,
    oom_risk: bool,
) {
    let m = &monitor.metrics;
    let elu = m.event_loop.utilization_pct;
    let heap_pct = m.heap.used_pct();

    let chart_h = 5u16.max(area.height / 4);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(chart_h),
            Constraint::Length(chart_h),
            Constraint::Min(1),
        ])
        .split(area);

    // ELU history chart
    let elu_color = if elu_alert { Color::Red } else { Color::Green };
    let empty: std::collections::VecDeque<u64> = std::collections::VecDeque::new();
    render_node_braille_chart(
        f,
        chunks[0],
        &monitor.elu_history,
        &empty,
        format!(" ELU {:.1}% ", elu),
        elu_color,
        100,
    );

    // Heap history chart
    let heap_color = if oom_risk { Color::Red } else if heap_pct > 70.0 { Color::Yellow } else { Color::Green };
    let max_heap = monitor.heap_used_history.iter().copied().max().unwrap_or(1).max(1);
    render_node_braille_chart(
        f,
        chunks[1],
        &monitor.heap_used_history,
        &empty,
        format!(" Heap {:.0}%  {} ", heap_pct, format_bytes(m.heap.used_bytes)),
        heap_color,
        max_heap,
    );

    // Detail text
    let detail_lines = build_detail_lines(m, theme, elu_alert, oom_risk);
    let p = Paragraph::new(detail_lines);
    f.render_widget(p, chunks[2]);
}

fn render_node_braille_chart(
    f: &mut Frame,
    area: Rect,
    primary: &std::collections::VecDeque<u64>,
    _secondary: &std::collections::VecDeque<u64>,
    title: String,
    color: Color,
    max_val: u64,
) {
    let max_samples = 60.0f64;
    let len = primary.len();
    let values: Vec<f64> = primary
        .iter()
        .map(|&v| v as f64 / max_val as f64 * 100.0)
        .collect();

    let canvas = Canvas::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(title, Style::default().fg(color).add_modifier(Modifier::BOLD)))
                .border_style(Style::default().fg(color)),
        )
        .x_bounds([0.0, max_samples])
        .y_bounds([0.0, 100.0])
        .marker(Marker::Braille)
        .paint(move |ctx| {
            let skip = len.saturating_sub(60);
            let slice = &values[skip..];
            let n = slice.len();
            for i in 1..n {
                let x0 = (max_samples - n as f64) + (i - 1) as f64;
                let x1 = (max_samples - n as f64) + i as f64;
                let y0 = slice[i - 1].clamp(0.0, 100.0);
                let y1 = slice[i].clamp(0.0, 100.0);
                ctx.draw(&ratatui::widgets::canvas::Line {
                    x1: x0,
                    y1: y0,
                    x2: x1,
                    y2: y1,
                    color,
                });
            }
        });

    f.render_widget(canvas, area);
}

fn build_detail_lines<'a>(
    m: &'a crate::collectors::node_runtime::NodeRuntimeMetrics,
    theme: &'a Theme,
    elu_alert: bool,
    oom_risk: bool,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();

    // Alertas visuales
    if elu_alert {
        lines.push(Line::from(vec![
            Span::styled(
                " ⚠ BLOCKED THREAD  ",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK),
            ),
            Span::styled(
                format!("ELU:{:.0}%  delay:{:.1}ms", m.event_loop.utilization_pct, m.event_loop.delay_ms),
                Style::default().fg(Color::Red),
            ),
        ]));
    }
    if oom_risk {
        lines.push(Line::from(Span::styled(
            " [OOM RISK]  Major GC continuo + Heap >85% ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )));
    }

    // Heap Spaces
    lines.push(Line::from(vec![
        Span::styled("Heap Spaces  ", Style::default().fg(theme.muted)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  New Space:  ", Style::default().fg(theme.muted)),
        Span::styled(format_bytes(m.heap.new_space_bytes), Style::default().fg(Color::White)),
        Span::raw("   "),
        Span::styled("Old Space:  ", Style::default().fg(theme.muted)),
        Span::styled(format_bytes(m.heap.old_space_bytes), Style::default().fg(Color::White)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Code Space: ", Style::default().fg(theme.muted)),
        Span::styled(format_bytes(m.heap.code_space_bytes), Style::default().fg(Color::White)),
        Span::raw("   "),
        Span::styled("Map Space:  ", Style::default().fg(theme.muted)),
        Span::styled(format_bytes(m.heap.map_space_bytes), Style::default().fg(Color::White)),
    ]));

    // GC Activity
    lines.push(Line::from(vec![
        Span::styled("GC Activity  ", Style::default().fg(theme.muted)),
    ]));
    let minor_color = if m.heap.minor_gc_rate > 10.0 { Color::Yellow } else { Color::Green };
    let major_color = if m.heap.major_gc_rate > 2.0 { Color::Red } else if m.heap.major_gc_rate > 0.5 { Color::Yellow } else { Color::Green };
    lines.push(Line::from(vec![
        Span::styled("  Minor GC:   ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{:.1}/s  avg {:.1}ms", m.heap.minor_gc_rate, m.heap.minor_gc_avg_ms),
            Style::default().fg(minor_color),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Major GC:   ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{:.1}/s  avg {:.1}ms", m.heap.major_gc_rate, m.heap.major_gc_avg_ms),
            Style::default().fg(major_color),
        ),
    ]));

    // Libuv
    lines.push(Line::from(vec![
        Span::styled("Libuv I/O  ", Style::default().fg(theme.muted)),
    ]));
    let handles_color = if m.libuv.active_handles > 1000 { Color::Yellow } else { Color::White };
    lines.push(Line::from(vec![
        Span::styled("  Handles:    ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{}", m.libuv.active_handles),
            Style::default().fg(handles_color),
        ),
        Span::raw("   "),
        Span::styled("Requests: ", Style::default().fg(theme.muted)),
        Span::styled(
            format!("{}", m.libuv.active_requests),
            Style::default().fg(Color::White),
        ),
    ]));
    let tp_color = if m.libuv.threadpool_queue > 8 { Color::Red } else if m.libuv.threadpool_queue > 4 { Color::Yellow } else { Color::Green };
    lines.push(Line::from(vec![
        Span::styled("  Thread Pool:", Style::default().fg(theme.muted)),
        Span::styled(
            format!("  {} queued", m.libuv.threadpool_queue),
            Style::default().fg(tp_color),
        ),
    ]));

    lines
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1}MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}
