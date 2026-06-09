use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        canvas::{Canvas, Line as CanvasLine},
        Block, Paragraph,
    },
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();

    let display_data: Option<crate::models::NetworkData> = if state.selected_nic.is_none() {
        state.current_network_total()
    } else {
        state.current_network().cloned()
    };

    match display_data {
        None => {
            let msg = Paragraph::new(Line::from(vec![Span::styled(
                "Detectando interfaz…",
                Style::default().fg(theme.muted),
            )]));
            f.render_widget(msg, area);
        }
        Some(data) => {
            let recv_str = format_bps(data.recv_bytes_per_sec);
            let sent_str = format_bps(data.sent_bytes_per_sec);

            let is_total = state.selected_nic.is_none();
            let label = if is_total {
                "Todas (sumatoria)"
            } else {
                data.interface.as_str()
            };

            let ip = if !is_total {
                state
                    .available_nics
                    .iter()
                    .find(|nic| nic.name == data.interface)
                    .and_then(|nic| nic.ip_address.as_ref())
            } else {
                None
            };

            // Layout: left text | right canvas (if wide enough) | hint
            let has_canvas = area.width >= 60 && area.height >= 3;
            let canvas_width = if has_canvas {
                (area.width / 3).clamp(20, 50)
            } else {
                0
            };

            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(canvas_width),
                    Constraint::Length(16),
                ])
                .split(area);

            // Left: text data
            let mut left_lines = Vec::new();

            let mut interface_spans = vec![
                Span::styled("Interfaz: ", Style::default().fg(theme.muted)),
                Span::styled(
                    label,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            if let Some(ip_addr) = ip {
                interface_spans.push(Span::raw("  "));
                interface_spans.push(Span::styled(
                    format!("[ IP: {} ]", ip_addr),
                    Style::default().fg(theme.muted),
                ));
            }
            left_lines.push(Line::from(interface_spans));

            if area.height >= 5 {
                left_lines.push(Line::from(vec![Span::styled(
                    "─".repeat(horizontal[0].width as usize),
                    Style::default().fg(theme.muted),
                )]));
            }

            let recv_total_str = format!("{}", ByteSize(data.total_recv_bytes));
            left_lines.push(Line::from(vec![
                Span::styled(
                    "↓ ",
                    Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
                ),
                Span::styled("Entrada: ", Style::default().fg(theme.muted)),
                Span::styled(
                    format!("{:<10}", recv_str),
                    Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
                ),
                Span::raw("   "),
                Span::styled(
                    format!("(Total Recibido: {})", recv_total_str),
                    Style::default().fg(theme.muted),
                ),
            ]));

            let sent_total_str = format!("{}", ByteSize(data.total_sent_bytes));
            left_lines.push(Line::from(vec![
                Span::styled(
                    "↑ ",
                    Style::default()
                        .fg(theme.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Salida:  ", Style::default().fg(theme.muted)),
                Span::styled(
                    format!("{:<10}", sent_str),
                    Style::default()
                        .fg(theme.accent_dim)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("   "),
                Span::styled(
                    format!("(Total Enviado: {})", sent_total_str),
                    Style::default().fg(theme.muted),
                ),
            ]));

            f.render_widget(Paragraph::new(left_lines), horizontal[0]);

            // Center: Braille usage % canvas
            if has_canvas && canvas_width > 0 {
                let current_pct = state
                    .network_usage_pct_history
                    .back()
                    .copied()
                    .unwrap_or(0.0);
                let graph_color = if current_pct >= 80.0 {
                    Color::Red
                } else if current_pct >= 50.0 {
                    Color::Yellow
                } else {
                    theme.ok
                };

                let history: Vec<f64> = state.network_usage_pct_history.iter().copied().collect();
                let n_samples = history.len();
                let max_x = canvas_width as f64 * 2.0; // Braille gives 2 dots per char column

                let canvas = Canvas::default()
                    .block(Block::default())
                    .x_bounds([0.0, max_x])
                    .y_bounds([0.0, 100.0])
                    .marker(Marker::Braille)
                    .paint(move |ctx| {
                        if n_samples > 1 {
                            for i in 0..(n_samples - 1) {
                                // Align newest sample to right edge
                                let x2 = max_x - (n_samples - 1 - (i + 1)) as f64;
                                let x1 = max_x - (n_samples - 1 - i) as f64;
                                if x2 < 0.0 {
                                    continue;
                                }
                                ctx.draw(&CanvasLine {
                                    x1: x1.max(0.0),
                                    y1: history[i],
                                    x2,
                                    y2: history[i + 1],
                                    color: graph_color,
                                });
                            }
                        }
                    });

                // Render canvas in a sub-area, leaving row 0 for the % label
                let canvas_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Min(1)])
                    .split(horizontal[1]);

                f.render_widget(
                    Paragraph::new(Line::from(vec![Span::styled(
                        format!("Uso: {:.1}%", current_pct),
                        Style::default().fg(graph_color).add_modifier(Modifier::DIM),
                    )])),
                    canvas_split[0],
                );
                f.render_widget(canvas, canvas_split[1]);
            }

            // Right: hint
            let hint = Paragraph::new(Line::from(vec![Span::styled(
                "[ F3 cambiar ]",
                Style::default().fg(theme.muted),
            )]))
            .alignment(Alignment::Right);
            f.render_widget(hint, horizontal[2]);
        }
    }
}

fn format_bps(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}
