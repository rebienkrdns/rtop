use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();

    let title_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(theme.muted);

    let manager_badge = if state.swarm_data.is_manager {
        Span::styled(" Swarm Manager ", title_style)
    } else {
        Span::styled(" Swarm ", title_style)
    };

    let hint = Span::styled(
        " [Esc] Volver  [Tab] Cambiar foco  [j/k] Navegar ",
        hint_style,
    );

    let block = Block::default()
        .title(Line::from(vec![manager_badge, hint]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_dim));

    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    if inner.height < 4 {
        return;
    }

    if !state.swarm_data.available || !state.swarm_data.is_manager {
        let msg = state
            .swarm_data
            .message
            .as_deref()
            .unwrap_or("Swarm no disponible");
        let p = Paragraph::new(msg).style(Style::default().fg(theme.muted));
        f.render_widget(p, inner);
        return;
    }

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(inner);

    render_services(f, split[0], state, &theme);
    render_nodes(f, split[1], state, &theme);
}

fn render_services(f: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let focused = state.swarm_focus == crate::app::SwarmFocus::Services;
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.accent_dim)
    };

    let block = Block::default()
        .title(Span::styled(
            " Servicios ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let header_style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from("Nombre").style(header_style),
        Cell::from("Replicas").style(header_style),
        Cell::from("Reinicios").style(header_style),
        Cell::from("Imagen").style(header_style),
    ])
    .bottom_margin(1);

    let constraints = [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Min(30),
    ];

    let services = &state.swarm_data.services;
    let cursor = state.swarm_service_cursor;

    let viewport = inner.height.saturating_sub(2) as usize;
    let start = if cursor >= viewport {
        cursor - viewport + 1
    } else {
        0
    };
    let end = (start + viewport).min(services.len());

    let rows: Vec<Row> = services
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(idx, svc)| {
            let is_selected = focused && idx == cursor;
            let base_style = if is_selected {
                Style::default()
                    .bg(theme.selected_bg)
                    .fg(theme.selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            let replica_color = if svc.replicas_running < svc.replicas_desired {
                Color::Red
            } else {
                Color::Green
            };

            Row::new(vec![
                Cell::from(svc.name.clone()).style(base_style),
                Cell::from(format!("{}/{}", svc.replicas_running, svc.replicas_desired)).style(
                    if is_selected {
                        base_style
                    } else {
                        Style::default()
                            .fg(replica_color)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                Cell::from(svc.restart_count.to_string()).style(if is_selected {
                    base_style
                } else if svc.restart_count > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(theme.muted)
                }),
                Cell::from(truncate_image(&svc.image, 40)).style(base_style),
            ])
        })
        .collect();

    let table = Table::new(rows, constraints).header(header);
    f.render_widget(table, inner);
}

fn render_nodes(f: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
    let focused = state.swarm_focus == crate::app::SwarmFocus::Nodes;
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.accent_dim)
    };

    let block = Block::default()
        .title(Span::styled(
            " Nodos ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let header_style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::BOLD);
    let header = Row::new(vec![
        Cell::from("Hostname").style(header_style),
        Cell::from("Rol").style(header_style),
        Cell::from("Estado").style(header_style),
        Cell::from("Disponibilidad").style(header_style),
    ])
    .bottom_margin(1);

    let constraints = [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(16),
    ];

    let nodes = &state.swarm_data.nodes;
    let cursor = state.swarm_node_cursor;

    let viewport = inner.height.saturating_sub(2) as usize;
    let start = if cursor >= viewport {
        cursor - viewport + 1
    } else {
        0
    };
    let end = (start + viewport).min(nodes.len());

    let rows: Vec<Row> = nodes
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(idx, node)| {
            let is_selected = focused && idx == cursor;
            let base_style = if is_selected {
                Style::default()
                    .bg(theme.selected_bg)
                    .fg(theme.selected_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };

            let status_lower = node.status.to_lowercase();
            let status_color = if status_lower.contains("ready") {
                Color::Green
            } else {
                Color::Red
            };

            let role_color = if node.role.to_lowercase().contains("manager") {
                theme.accent
            } else {
                theme.text
            };

            Row::new(vec![
                Cell::from(node.hostname.clone()).style(base_style),
                Cell::from(node.role.clone()).style(if is_selected {
                    base_style
                } else {
                    Style::default()
                        .fg(role_color)
                        .add_modifier(Modifier::BOLD)
                }),
                Cell::from(node.status.clone()).style(if is_selected {
                    base_style
                } else {
                    Style::default().fg(status_color)
                }),
                Cell::from(node.availability.clone()).style(base_style),
            ])
        })
        .collect();

    let table = Table::new(rows, constraints).header(header);
    f.render_widget(table, inner);
}

fn truncate_image(image: &str, max: usize) -> String {
    let short = image.rsplit('/').next().unwrap_or(image);
    if short.len() <= max {
        short.to_string()
    } else {
        format!("{}...", &short[..max.saturating_sub(3)])
    }
}
