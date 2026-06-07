use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(area);

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
            f.render_widget(msg, layout[0]);
        }
        Some(data) => {
            let recv_str = format_bps(data.recv_bytes_per_sec);
            let sent_str = format_bps(data.sent_bytes_per_sec);

            let is_total = state.selected_nic.is_none();
            let label = if is_total { "Todas (sumatoria)" } else { data.interface.as_str() };

            let info_line = Line::from(vec![
                Span::styled("Interfaz: ", Style::default().fg(theme.muted)),
                Span::styled(
                    label,
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            f.render_widget(Paragraph::new(info_line), layout[0]);

            let rates_line = Line::from(vec![
                Span::styled("↓ ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled("Entrada ", Style::default().fg(theme.muted)),
                Span::styled(
                    recv_str,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("     "),
                Span::styled("↑ ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                Span::styled("Salida ", Style::default().fg(theme.muted)),
                Span::styled(
                    sent_str,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            f.render_widget(Paragraph::new(rates_line), layout[1]);
        }
    }

    let hint = Paragraph::new(Line::from(vec![
        Span::styled(
            "[ F3 cambiar ]",
            Style::default().fg(theme.muted),
        ),
    ]))
    .alignment(Alignment::Right);
    f.render_widget(hint, layout[1]);
}

fn format_bps(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}
