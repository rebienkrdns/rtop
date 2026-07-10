use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

/// Formatea bytes/s con unidad adecuada.
fn fmt_bps(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}

/// Convierte bytes/s a bits/s con unidad adecuada (Kbps, Mbps, Gbps).
fn fmt_bitrate(bps: f64) -> String {
    let bits = bps * 8.0;
    if bits >= 1_000_000_000.0 {
        format!("{:.1} Gbps", bits / 1_000_000_000.0)
    } else if bits >= 1_000_000.0 {
        format!("{:.1} Mbps", bits / 1_000_000.0)
    } else if bits >= 1_000.0 {
        format!("{:.1} Kbps", bits / 1_000.0)
    } else {
        format!("{:.0} bps", bits)
    }
}

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

            let cap_bps = if let Some(ref name) = state.selected_nic {
                state
                    .network_max_bw_by_nic
                    .get(name)
                    .copied()
                    .unwrap_or(125_000_000.0)
            } else {
                let loopback_names: Vec<&str> = state
                    .available_nics
                    .iter()
                    .filter(|n| n.is_loopback)
                    .map(|n| n.name.as_str())
                    .collect();
                state
                    .network_max_bw_by_nic
                    .iter()
                    .filter(|(name, _)| !loopback_names.contains(&name.as_str()))
                    .map(|(_, &v)| v)
                    .fold(0.0_f64, f64::max)
                    .max(125_000_000.0)
            };

            let usage_pct = state
                .network_usage_pct_history
                .back()
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, 100.0);
            let usage_color = Theme::color_for_pct(usage_pct);

            // Decidir cuántas filas mostrar según espacio disponible
            let h = area.height as usize;
            let has_bitrate = h >= 3;
            let has_peak = h >= 4;
            let has_total = h >= 5;
            let has_errors =
                h >= 6 && (data.rx_errors + data.tx_errors + data.rx_drops + data.tx_drops) > 0;

            let mut constraints = vec![
                Constraint::Length(1), // header: iface + %
                Constraint::Length(1), // gauge
            ];
            if has_bitrate {
                constraints.push(Constraint::Length(1));
            } // bytes/s
            if has_peak {
                constraints.push(Constraint::Length(1));
            } // bitrate
            if has_total {
                constraints.push(Constraint::Length(1));
            } // peak
            if has_errors {
                constraints.push(Constraint::Length(1));
            } // errors/drops
            constraints.push(Constraint::Min(0));

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(area);

            let mut row = 0;

            // Fila 0: header
            let header_cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(0), Constraint::Length(8)])
                .split(chunks[row]);
            row += 1;

            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!("{}  ", state.t("Network")),
                        Style::default().fg(theme.muted),
                    ),
                    Span::styled(
                        format!("{}  ", label),
                        Style::default()
                            .fg(theme.accent)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("[{}]", fmt_bitrate(cap_bps)),
                        Style::default().fg(theme.muted),
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

            // Fila 1: gauge
            let gauge = Gauge::default()
                .gauge_style(
                    Style::default()
                        .fg(usage_color)
                        .bg(ratatui::style::Color::Rgb(51, 52, 61)),
                )
                .ratio(usage_pct / 100.0)
                .label("");
            f.render_widget(gauge, chunks[row]);
            row += 1;

            // Fila 2: velocidad actual bytes/s
            if has_bitrate {
                let rate_cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Min(0), Constraint::Length(14)])
                    .split(chunks[row]);
                row += 1;

                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            "↓ ",
                            Style::default().fg(theme.ok).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            fmt_bps(data.recv_bytes_per_sec),
                            Style::default().fg(theme.ok),
                        ),
                        Span::raw("   "),
                        Span::styled(
                            "↑ ",
                            Style::default()
                                .fg(theme.accent_dim)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            fmt_bps(data.sent_bytes_per_sec),
                            Style::default().fg(theme.accent_dim),
                        ),
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

            // Fila 3: velocidad en bits/s (estilo btop)
            if has_peak {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            fmt_bitrate(data.recv_bytes_per_sec),
                            Style::default().fg(theme.ok),
                        ),
                        Span::raw("   "),
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            fmt_bitrate(data.sent_bytes_per_sec),
                            Style::default().fg(theme.accent_dim),
                        ),
                    ])),
                    chunks[row],
                );
                row += 1;
            }

            // Fila 4: pico histórico de la sesión
            if has_total {
                let (peak_recv, peak_sent) = if is_total {
                    (state.network_peak_recv_bps, state.network_peak_sent_bps)
                } else {
                    // Para NIC individual aún usamos el total agregado como aproximación
                    (state.network_peak_recv_bps, state.network_peak_sent_bps)
                };
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("↓ Top:", Style::default().fg(theme.muted)),
                        Span::styled(
                            format!(" {}", fmt_bps(peak_recv)),
                            Style::default().fg(theme.ok),
                        ),
                        Span::raw("  "),
                        Span::styled("↑ Top:", Style::default().fg(theme.muted)),
                        Span::styled(
                            format!(" {}", fmt_bps(peak_sent)),
                            Style::default().fg(theme.accent_dim),
                        ),
                    ])),
                    chunks[row],
                );
                row += 1;
            }

            // Totales acumulados
            if has_errors && row < chunks.len().saturating_sub(1) {
                // Reutilizamos la fila para errores ya que has_errors=true implica has_total=true
            }

            // Fila 5 (si hay errores/drops): errores de red
            if has_errors {
                let err_color = Color::Yellow;
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("err↓", Style::default().fg(err_color)),
                        Span::styled(
                            format!("{}", data.rx_errors),
                            Style::default().fg(err_color),
                        ),
                        Span::styled(" drp↓", Style::default().fg(err_color)),
                        Span::styled(format!("{}", data.rx_drops), Style::default().fg(err_color)),
                        Span::styled("  err↑", Style::default().fg(err_color)),
                        Span::styled(
                            format!("{}", data.tx_errors),
                            Style::default().fg(err_color),
                        ),
                        Span::styled(" drp↑", Style::default().fg(err_color)),
                        Span::styled(format!("{}", data.tx_drops), Style::default().fg(err_color)),
                    ])),
                    chunks[row],
                );
            }
        }
    }
}
