use bytesize::ByteSize;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::app::AppState;

use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, state: &AppState) {
    let area = centered_rect(75, 50, f.size());

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
        let msg = Paragraph::new(state.t("NoDisks")).style(Style::default().fg(Color::DarkGray));
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

    let entries = &state.selector_entries;

    let mut rows: Vec<Row> = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
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
            format!("{:>9}", ByteSize(entry.total_bytes).to_string())
        } else {
            format!("{:>9}", "?")
        };
        let sel_mark = if is_selected {
            format!("  ({})", state.t("Selected"))
        } else {
            String::new()
        };

        let (rx, tx) = state
            .disks
            .iter()
            .find(|d| d.device == entry.device_short && d.mount_point == entry.mount_point)
            .map(|d| {
                (
                    d.read_bytes_per_sec.unwrap_or(0.0),
                    d.write_bytes_per_sec.unwrap_or(0.0),
                )
            })
            .unwrap_or((0.0, 0.0));

        let theme = Theme::default_theme();
        let base_style = if is_cursor {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };

        let mount_style = if is_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        rows.push(Row::new(vec![
            Cell::from(Span::styled(cursor_sym, base_style)),
            Cell::from(Span::styled(entry.device_short.clone(), base_style)),
            Cell::from(Span::styled(mount, mount_style)),
            Cell::from(Span::styled(size_str, base_style)),
            Cell::from(Span::styled(
                format!("↓{:>8}", fmt_bps_short(rx)),
                Style::default().fg(theme.ok),
            )),
            Cell::from(Span::styled(
                format!("↑{:>8}", fmt_bps_short(tx)),
                Style::default().fg(theme.accent_dim),
            )),
            Cell::from(Span::styled(sel_mark, Style::default().fg(Color::Green))),
        ]));
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(state.t("Device")),
        Cell::from(state.t("Mount")),
        Cell::from(format!("{:>9}", state.t("Size"))),
        Cell::from(format!("{:>9}", state.t("Read"))),
        Cell::from(format!("{:>9}", state.t("Write"))),
        Cell::from(state.t("Status")),
    ])
    .style(
        Style::default()
            .fg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(14),
            Constraint::Min(15), // Se estira y empuja lo demás a la derecha
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .column_spacing(1);

    let mut table_state = ratatui::widgets::TableState::default();
    table_state.select(Some(state.disk_selector_cursor));

    f.render_stateful_widget(table, list_area, &mut table_state);

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

/// Helper to format speed compactly (e.g., 1.2M/s instead of 1.2 MB/s)
fn fmt_bps_short(bps: f64) -> String {
    let bs = bytesize::ByteSize(bps as u64).to_string_as(true); // e.g. "1.2 MB"
    let mut parts = bs.split_whitespace();
    if let (Some(num), Some(unit)) = (parts.next(), parts.next()) {
        if unit == "B" {
            format!("{}B/s", num)
        } else {
            format!("{}{}/s", num, unit.chars().next().unwrap_or('B'))
        }
    } else {
        format!("{}/s", bs)
    }
}
