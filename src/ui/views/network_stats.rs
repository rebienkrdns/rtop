use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Clear, Dataset, GraphType, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " TCP Network Health ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" [Esc] Volver • [N] Red ", Style::default().fg(theme.muted)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    if inner.height < 4 || inner.width < 30 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(4)])
        .split(inner);

    render_metrics_panel(f, chunks[0], state, &theme);
    render_history_chart(f, chunks[1], state, &theme);
}

fn render_metrics_panel(f: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let block = Block::default()
        .title(Span::styled(
            " Métricas TCP ",
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(ref stats) = state.tcp_stats else {
        let msg = Paragraph::new("No disponible (requiere Linux con /proc/net/snmp)")
            .style(Style::default().fg(theme.muted));
        f.render_widget(msg, inner);
        return;
    };

    let rate = stats.tcp_retransmission_rate;
    let (health_color, health_label) = if rate < 1.0 {
        (Color::Green, "● SALUDABLE")
    } else if rate < 5.0 {
        (Color::Yellow, "● ADVERTENCIA")
    } else {
        (Color::Red, "● CRÍTICO")
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Estado:           ", Style::default().fg(theme.muted)),
            Span::styled(
                health_label,
                Style::default()
                    .fg(health_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Retransmisiones:  ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}", stats.tcp_retransmissions),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("Tasa retrans:     ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{:.2}%", rate),
                Style::default()
                    .fg(health_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Conex. fallidas:  ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}", stats.tcp_failed_connections),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("Resets:           ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}", stats.tcp_resets),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("RetransFail:      ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{}", stats.tcp_retrans_fail),
                Style::default().fg(theme.text),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_history_chart(f: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let block = Block::default()
        .title(Span::styled(
            " Historial Tasa de Retransmisión (%) ",
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));

    if state.tcp_retrans_history.is_empty() {
        let msg = Paragraph::new("Sin datos de historial aún...")
            .style(Style::default().fg(theme.muted))
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let data_points: Vec<(f64, f64)> = state
        .tcp_retrans_history
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();

    let max_val = state
        .tcp_retrans_history
        .iter()
        .cloned()
        .fold(0.0_f64, f64::max)
        .max(5.0);

    let n = data_points.len() as f64;

    let datasets = vec![Dataset::default()
        .name("retrans %")
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Cyan))
        .data(&data_points)];

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme.muted))
                .bounds([0.0, n]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme.muted))
                .labels(vec![
                    Span::styled("0%", Style::default().fg(theme.muted)),
                    Span::styled(
                        format!("{:.1}%", max_val / 2.0),
                        Style::default().fg(theme.muted),
                    ),
                    Span::styled(format!("{:.1}%", max_val), Style::default().fg(theme.muted)),
                ])
                .bounds([0.0, max_val]),
        );

    f.render_widget(chart, area);
}
