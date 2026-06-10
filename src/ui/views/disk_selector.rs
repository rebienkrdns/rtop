use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render(f: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 50, f.size());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("SelectDisk")),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.selector_entries.is_empty() {
        let msg = Paragraph::new(state.t("NoDisks"))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, inner);
        return;
    }

    // Última línea: ayuda de teclas
    let hint_height = 1u16;
    let list_height = inner.height.saturating_sub(hint_height + 1);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_height),
            Constraint::Length(1),
            Constraint::Length(hint_height),
        ])
        .split(inner);

    let list_area = layout[0];
    let hint_area = layout[2];

    // Lista de dispositivos
    let visible_start = state
        .disk_selector_cursor
        .saturating_sub(list_area.height as usize / 2);
    let entries = &state.selector_entries;

    let mut lines: Vec<Line> = Vec::new();
    for (i, entry) in entries.iter().enumerate().skip(visible_start) {
        if lines.len() >= list_area.height as usize {
            break;
        }

        let is_cursor = i == state.disk_selector_cursor;
        let is_selected = state
            .cfg
            .selected_disk
            .as_deref()
            .map(|s| s == entry.device_short)
            .unwrap_or(false);

        let cursor_sym = if is_cursor { "> " } else { "  " };
        let mount = if entry.mount_point.is_empty() {
            "(no mount)".to_string()
        } else {
            entry.mount_point.clone()
        };
        let size_str = if entry.total_bytes > 0 {
            ByteSize(entry.total_bytes).to_string()
        } else {
            "?".to_string()
        };
        let sel_mark = if is_selected { format!("  ({})", state.t("Selected")) } else { String::new() };

        let base_style = if is_cursor {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(vec![Span::styled(
            format!(
                "{}{:<14} {:<12} {:>8}{}",
                cursor_sym, entry.device_short, mount, size_str, sel_mark
            ),
            base_style,
        )]));
    }

    f.render_widget(Paragraph::new(lines), list_area);

    let hint = Line::from(vec![
        Span::styled("↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(format!(" {}  ", state.t("Navigate"))),
        Span::styled("Enter", Style::default().fg(Color::Green)),
        Span::raw(format!(" {}  ", state.t("Selected"))),
        Span::styled("ESC", Style::default().fg(Color::Yellow)),
        Span::raw(format!(" {}", state.t("Cancel"))),
    ]);
    f.render_widget(Paragraph::new(hint).alignment(Alignment::Center), hint_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}
