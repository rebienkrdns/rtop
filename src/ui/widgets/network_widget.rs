use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();

    // Determine what data to show: aggregate of all NICs or a single NIC
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
            let label = if is_total { "Todas (sumatoria)" } else { data.interface.as_str() };

            // Find IP if a specific interface is selected
            let ip = if !is_total {
                state.available_nics.iter()
                    .find(|nic| nic.name == data.interface)
                    .and_then(|nic| nic.ip_address.as_ref())
            } else {
                None
            };

            // Layout horizontal: left (content) and right (hint)
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(16),
                ])
                .split(area);

            // Left content layout
            let mut left_lines = Vec::new();

            // Line 1: Interfaz: <label> [ IP: <ip> ]
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

            // Line 2: separator line if height >= 5
            if area.height >= 5 {
                left_lines.push(Line::from(vec![
                    Span::styled(
                        "─".repeat(horizontal[0].width as usize),
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }

            // Line 3: ↓ Entrada: <rate> (Total Recibido: <total>)
            let recv_total_str = format!("{}", ByteSize(data.total_recv_bytes));
            left_lines.push(Line::from(vec![
                Span::styled("↓ ", Style::default().fg(theme.ok).add_modifier(Modifier::BOLD)),
                Span::styled("Entrada: ", Style::default().fg(theme.muted)),
                Span::styled(
                    format!("{:<10}", recv_str),
                    Style::default()
                        .fg(theme.ok)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("   "),
                Span::styled(
                    format!("(Total Recibido: {})", recv_total_str),
                    Style::default().fg(theme.muted),
                ),
            ]));

            // Line 4: ↑ Salida: <rate> (Total Enviado: <total>)
            let sent_total_str = format!("{}", ByteSize(data.total_sent_bytes));
            left_lines.push(Line::from(vec![
                Span::styled("↑ ", Style::default().fg(theme.accent_dim).add_modifier(Modifier::BOLD)),
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

            // Right content (hint)
            let hint = Paragraph::new(Line::from(vec![
                Span::styled(
                    "[ F3 cambiar ]",
                    Style::default().fg(theme.muted),
                ),
            ]))
            .alignment(Alignment::Right);
            f.render_widget(hint, horizontal[1]);
        }
    }
}

fn format_bps(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}
