use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
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
                state.t("SelectNIC"),
                Style::default().fg(theme.muted),
            )]));
            f.render_widget(msg, area);
        }
        Some(data) => {
            let is_total = state.selected_nic.is_none();
            let label = if is_total {
                format!("{} {}", state.t("AllNICs"), state.t("Summation"))
            } else {
                data.interface.clone()
            };

            // Resolve current usage %
            let usage_pct = state
                .network_usage_pct_history
                .back()
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);

            let usage_color = Theme::color_for_pct(usage_pct);

            let has_rate_row = area.height >= 3;
            let has_total_row = area.height >= 4;

            let constraints: Vec<Constraint> = {
                let mut c = vec![
                    Constraint::Length(1), // header: "Red  <iface>"  X%
                    Constraint::Length(1), // gauge bar
                ];
                if has_rate_row {
                    c.push(Constraint::Length(1)); // ↓ entrada  ↑ salida  [F3]
                }
                if has_total_row {
                    c.push(Constraint::Length(1)); // totales acumulados
                }
                c
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);

            // Row 0: header
            let header_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(8)])
                .split(chunks[0]);

            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{}  ", state.t("Network")), Style::default().fg(theme.muted)),
                    Span::styled(
                        label,
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                header_cols[0],
            );
            f.render_widget(
                Paragraph::new(format!("{:.0}%", usage_pct))
                    .style(
                        Style::default()
                            .fg(usage_color)
                            .add_modifier(Modifier::BOLD),
                    )
                    .alignment(Alignment::Right),
                header_cols[1],
            );

            // Row 1: gauge bar (same style as disk)
            let gauge = Gauge::default()
                .gauge_style(
                    Style::default()
                        .fg(usage_color)
                        .bg(ratatui::style::Color::Rgb(51, 52, 61)),
                )
                .ratio(usage_pct / 100.0)
                .label("");
            f.render_widget(gauge, chunks[1]);

            // Row 2: rates + F3 hint
            if has_rate_row {
                let recv_str = format_bps(data.recv_bytes_per_sec);
                let sent_str = format_bps(data.sent_bytes_per_sec);

                let rate_cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(0), Constraint::Length(14)])
                    .split(chunks[2]);

                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            format!("↓ {} ", "In"),
                            Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(recv_str, Style::default().fg(theme.ok)),
                        Span::raw("     "),
                        Span::styled(
                            format!("↑ {} ", "Out"),
                            Style::default()
                                .fg(theme.accent_dim)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(sent_str, Style::default().fg(theme.accent_dim)),
                    ])),
                    rate_cols[0],
                );
                f.render_widget(
                    Paragraph::new(format!("[ F3 {} ]", state.t("Change tab")))
                        .style(Style::default().fg(theme.muted))
                        .alignment(Alignment::Right),
                    rate_cols[1],
                );
            }

            // Row 3: cumulative totals
            if has_total_row {
                let recv_total = format!("{}", ByteSize(data.total_recv_bytes));
                let sent_total = format!("{}", ByteSize(data.total_sent_bytes));
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(format!("↓ {}: ", state.t("Net Tot")), Style::default().fg(theme.muted)),
                        Span::styled(recv_total, Style::default().fg(theme.ok)),
                        Span::raw("     "),
                        Span::styled(format!("↑ {}: ", state.t("Net Tot")), Style::default().fg(theme.muted)),
                        Span::styled(sent_total, Style::default().fg(theme.accent_dim)),
                    ])),
                    chunks[3],
                );
            }
        }
    }
}

fn format_bps(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}
