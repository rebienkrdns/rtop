use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table,
    },
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;
use crate::ui::views::process_detail::fmt_latency;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();

    let title = " Traefik Router Latency ";
    let hint = " [Esc] Volver • [↑/↓] Navegar ";

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                title,
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {} ", hint), Style::default().fg(theme.muted)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    if inner.height < 3 || inner.width < 10 {
        return;
    }

    let monitor_data = match &state.proxy_monitor {
        Some(data) => data,
        None => {
            let paragraph =
                Paragraph::new("Loading proxy data...").style(Style::default().fg(theme.muted));
            f.render_widget(paragraph, inner);
            return;
        }
    };

    // Sort routers by p99 descending (worst offenders first).
    let mut routers: Vec<(&String, &crate::collectors::proxy::RouterLatencySnapshot)> =
        monitor_data.router_percentiles.iter().collect();
    routers.sort_by(|a, b| {
        b.1.p99_ms
            .partial_cmp(&a.1.p99_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if routers.is_empty() {
        let no_data = Paragraph::new("No router latency data available.")
            .style(Style::default().fg(theme.muted));
        f.render_widget(no_data, inner);
        return;
    }

    let header_style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);

    let header_row = Row::new(vec![
        Cell::from("Router").style(header_style),
        Cell::from("p50").style(header_style),
        Cell::from("p95").style(header_style),
        Cell::from("p99").style(header_style),
    ])
    .bottom_margin(1);

    let constraints = [
        Constraint::Min(40),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Length(12),
    ];

    let viewport_height = (inner.height as usize).saturating_sub(2);
    let total_routers = routers.len();

    let start_idx = if state.router_cursor >= viewport_height {
        state.router_cursor - viewport_height + 1
    } else {
        0
    };
    let end_idx = (start_idx + viewport_height).min(total_routers);

    let mut rows = Vec::new();
    for (idx, (router, snapshot)) in routers.iter().enumerate().take(end_idx).skip(start_idx) {
        let is_selected = idx == state.router_cursor;
        let style = if is_selected {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
                .bg(Color::Rgb(40, 42, 54))
        } else {
            Style::default().fg(Color::White)
        };

        rows.push(
            Row::new(vec![
                Cell::from(router.to_string()),
                Cell::from(fmt_latency(snapshot.p50_ms)),
                Cell::from(fmt_latency(snapshot.p95_ms)),
                Cell::from(fmt_latency(snapshot.p99_ms)),
            ])
            .style(style),
        );
    }

    let table = Table::new(rows, constraints)
        .header(header_row)
        .column_spacing(2);

    f.render_widget(table, inner);

    if total_routers > viewport_height {
        let max_scroll = total_routers.saturating_sub(viewport_height);
        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(start_idx);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(None)
                .thumb_symbol("┃"),
            inner,
            &mut scrollbar_state,
        );
    }
}
