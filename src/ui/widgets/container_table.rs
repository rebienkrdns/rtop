use bytesize::ByteSize;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::{ContainerData, ContainerStatus};
use crate::ui::theme::Theme;

fn fmt_rate(rate: f64) -> String {
    format!("{}/s", ByteSize(rate as u64))
}

fn status_color(status: &ContainerStatus) -> Color {
    match status {
        ContainerStatus::Running => Color::Green,
        ContainerStatus::Paused => Color::Yellow,
        ContainerStatus::Restarting => Color::Magenta,
        ContainerStatus::Exited => Color::DarkGray,
        ContainerStatus::Dead => Color::Red,
        ContainerStatus::Unknown => Color::Gray,
    }
}

#[allow(dead_code)]
pub fn render(f: &mut Frame, area: Rect, containers: &[ContainerData]) {
    render_with_cursor(f, area, containers, 0);
}

pub fn render_with_cursor(f: &mut Frame, area: Rect, containers: &[ContainerData], cursor: usize) {
    let header_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);

    let mut lines = vec![Line::from(vec![
        Span::styled(format!("{:<18}", "Nombre"), header_style),
        Span::styled(format!("{:>6}", "CPU%"), header_style),
        Span::raw("  "),
        Span::styled(format!("{:>12}", "RAM"), header_style),
        Span::raw("  "),
        Span::styled(format!("{:>14}", "Red ↓/↑"), header_style),
        Span::raw("  "),
        Span::styled(format!("{:>14}", "Disco R/W"), header_style),
        Span::raw("  "),
        Span::styled("Estado", header_style),
    ])];

    if containers.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Sin contenedores activos",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, c) in containers.iter().enumerate().take(area.height.saturating_sub(1) as usize) {
            let selected = i == cursor;
            let row_style = if selected {
                Style::default().bg(Theme::default_theme().accent).fg(Color::Black)
            } else {
                Style::default()
            };
            let mem_str = format!("{}/{}", ByteSize(c.memory_bytes), ByteSize(c.memory_limit_bytes));
            let net_str = format!("↓{} ↑{}", fmt_rate(c.net_recv_per_sec), fmt_rate(c.net_sent_per_sec));
            let disk_str = format!("R{} W{}", fmt_rate(c.disk_read_per_sec), fmt_rate(c.disk_write_per_sec));
            let status_label = c.status.as_str();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<18}", c.name.chars().take(17).collect::<String>()),
                    row_style.fg(if selected { Color::Black } else { Color::White }),
                ),
                Span::styled(
                    format!("{:>6.1}", c.cpu_pct),
                    row_style.fg(if selected { Color::Black } else { Theme::color_for_pct(c.cpu_pct) }),
                ),
                Span::raw("  "),
                Span::styled(format!("{:>12}", mem_str), row_style.fg(if selected { Color::Black } else { Color::White })),
                Span::raw("  "),
                Span::styled(format!("{:>14}", net_str), row_style.fg(if selected { Color::Black } else { Color::Cyan })),
                Span::raw("  "),
                Span::styled(format!("{:>14}", disk_str), row_style.fg(if selected { Color::Black } else { Color::Yellow })),
                Span::raw("  "),
                Span::styled(
                    format!("● {}", status_label),
                    if selected { row_style } else { Style::default().fg(status_color(&c.status)) },
                ),
            ]));
        }
    }

    f.render_widget(Paragraph::new(lines), area);
}
