use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::models::{ContainerData, ContainerStatus};
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, container: &ContainerData, confirm: Option<&ConfirmAction>) {
    let theme = Theme::default_theme();

    let block = Block::default()
        .title(Span::styled(
            format!(" Contenedor: {} ", container.name),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // info
            Constraint::Length(3), // CPU bar
            Constraint::Length(3), // Memory bar
            Constraint::Length(3), // Net bar
            Constraint::Length(3), // Disk bar
            Constraint::Length(4), // ports + volumes
            Constraint::Length(2), // footer
            Constraint::Min(0),
        ])
        .split(inner);

    // Info fields
    let uptime_str = container.uptime_secs.map(format_uptime).unwrap_or_else(|| "—".to_string());
    let status_color = match &container.status {
        ContainerStatus::Running => Color::Green,
        ContainerStatus::Paused => Color::Yellow,
        ContainerStatus::Restarting => Color::Magenta,
        ContainerStatus::Exited => Color::DarkGray,
        ContainerStatus::Dead => Color::Red,
        ContainerStatus::Unknown => Color::Gray,
    };

    let id_short = if container.id.len() > 12 { &container.id[..12] } else { &container.id };
    let info_lines = vec![
        Line::from(vec![
            Span::styled("ID:     ", Style::default().fg(theme.muted)),
            Span::styled(id_short, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Imagen: ", Style::default().fg(theme.muted)),
            Span::styled(container.image.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Estado: ", Style::default().fg(theme.muted)),
            Span::styled(container.status.as_str(), Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            Span::styled("Uptime: ", Style::default().fg(theme.muted)),
            Span::styled(uptime_str, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("RAM:    ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{} / {}", ByteSize(container.memory_bytes), ByteSize(container.memory_limit_bytes)),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("Red Tot: ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("↓ {}  ·  ↑ {}", ByteSize(container.net_recv_total), ByteSize(container.net_sent_total)),
                Style::default().fg(Color::White),
            ),
            Span::raw("   "),
            Span::styled("Disk Tot: ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("R {}  ·  W {}", ByteSize(container.disk_read_total), ByteSize(container.disk_write_total)),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(info_lines), chunks[0]);

    // CPU bar
    let cpu_pct = container.cpu_pct.clamp(0.0, 100.0);
    let cpu_gauge = Gauge::default()
        .block(Block::default()
            .title(Span::styled(format!(" CPU  {:.1}% ", cpu_pct), Style::default().fg(theme.muted)))
            .borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Theme::color_for_pct(cpu_pct)).bg(Color::DarkGray))
        .ratio(cpu_pct / 100.0);
    f.render_widget(cpu_gauge, chunks[1]);

    // Memory bar
    let mem_pct = container.memory_pct.clamp(0.0, 100.0);
    let mem_gauge = Gauge::default()
        .block(Block::default()
            .title(Span::styled(format!(" Memoria  {:.1}% ", mem_pct), Style::default().fg(theme.muted)))
            .borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Theme::color_for_pct(mem_pct)).bg(Color::DarkGray))
        .ratio(mem_pct / 100.0);
    f.render_widget(mem_gauge, chunks[2]);

    // Network bar (recv)
    let net_recv = container.net_recv_per_sec;
    let net_sent = container.net_sent_per_sec;
    let net_ratio = (net_recv.max(net_sent) / 10_000_000.0).clamp(0.0, 1.0);
    let net_gauge = Gauge::default()
        .block(Block::default()
            .title(Span::styled(
                format!(" Red  ↓{}/s (Total: {})  ↑{}/s (Total: {}) ", ByteSize(net_recv as u64), ByteSize(container.net_recv_total), ByteSize(net_sent as u64), ByteSize(container.net_sent_total)),
                Style::default().fg(theme.muted),
            ))
            .borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .ratio(net_ratio);
    f.render_widget(net_gauge, chunks[3]);

    // Disk bar
    let disk_r = container.disk_read_per_sec;
    let disk_w = container.disk_write_per_sec;
    let disk_ratio = (disk_r.max(disk_w) / 100_000_000.0).clamp(0.0, 1.0);
    let disk_gauge = Gauge::default()
        .block(Block::default()
            .title(Span::styled(
                format!(" Disco  R:{}/s (Total: {})  W:{}/s (Total: {}) ", ByteSize(disk_r as u64), ByteSize(container.disk_read_total), ByteSize(disk_w as u64), ByteSize(container.disk_write_total)),
                Style::default().fg(theme.muted),
            ))
            .borders(Borders::ALL).border_style(Style::default().fg(theme.muted)))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .ratio(disk_ratio);
    f.render_widget(disk_gauge, chunks[4]);

    // Ports + Volumes
    let ports_str = if container.ports.is_empty() {
        "—".to_string()
    } else {
        container.ports.join(", ")
    };
    let volumes_str = if container.volumes.is_empty() {
        "—".to_string()
    } else {
        container.volumes.join(", ")
    };
    let pv_lines = vec![
        Line::from(vec![
            Span::styled("Puertos:  ", Style::default().fg(theme.muted)),
            Span::styled(ports_str, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Volúmenes: ", Style::default().fg(theme.muted)),
            Span::styled(volumes_str, Style::default().fg(Color::Yellow)),
        ]),
    ];
    f.render_widget(Paragraph::new(pv_lines), chunks[5]);

    // Footer
    let hint = Line::from(vec![
        Span::styled(" [ESC] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Volver  ", Style::default().fg(theme.muted)),
        Span::styled("[L] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Logs  ", Style::default().fg(theme.muted)),
        Span::styled("[R] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Reiniciar  ", Style::default().fg(theme.muted)),
        Span::styled("[S] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Detener", Style::default().fg(theme.muted)),
    ]);
    f.render_widget(Paragraph::new(hint), chunks[6]);

    // Confirmation overlay
    if let Some(action) = confirm {
        render_confirm_dialog(f, area, action);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmAction {
    Restart(String), // container id
    Stop(String),
}

impl ConfirmAction {
    pub fn label(&self) -> &str {
        match self {
            ConfirmAction::Restart(_) => "reiniciar",
            ConfirmAction::Stop(_) => "detener",
        }
    }

    #[allow(dead_code)]
    pub fn container_id(&self) -> &str {
        match self {
            ConfirmAction::Restart(id) | ConfirmAction::Stop(id) => id,
        }
    }
}

fn render_confirm_dialog(f: &mut Frame, area: Rect, action: &ConfirmAction) {
    let theme = Theme::default_theme();

    // Center a small dialog box
    let dialog_w = 50u16.min(area.width.saturating_sub(4));
    let dialog_h = 5u16;
    let x = area.x + (area.width.saturating_sub(dialog_w)) / 2;
    let y = area.y + (area.height.saturating_sub(dialog_h)) / 2;
    let dialog_area = Rect { x, y, width: dialog_w, height: dialog_h };

    let block = Block::default()
        .title(Span::styled(" Confirmar ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(dialog_area);
    f.render_widget(ratatui::widgets::Clear, dialog_area);
    f.render_widget(block, dialog_area);

    let msg = format!("¿Seguro que quieres {} este contenedor?", action.label());
    let lines = vec![
        Line::from(Span::styled(msg, Style::default().fg(Color::White))),
        Line::from(vec![]),
        Line::from(vec![
            Span::styled("[Enter] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("Confirmar  ", Style::default().fg(theme.muted)),
            Span::styled("[ESC] ", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("Cancelar", Style::default().fg(theme.muted)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    }
}
