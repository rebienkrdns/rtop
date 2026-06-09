use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::AppState;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    // Centro el modal: 60% ancho, ~80% alto
    let popup = centered_rect(62, 82, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            state.t("Help Shortcuts"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let sections: Vec<(&str, Vec<(&str, &str)>)> = vec![
        (
            state.t("General Navigation"),
            vec![
                ("[q]", state.t("Quit rtop")),
                ("[Ctrl+C]", state.t("Quit always")),
                ("[Tab]", state.t("Change tab")),
                ("[F1]", state.t("Show hide help")),
                ("[F4]", state.t("Change theme")),
                ("[Esc]", state.t("Close modal exit detail")),
            ],
        ),
        (
            state.t("System"),
            vec![
                ("[[]]", state.t("Decrease refresh interval")),
                ("[]]]", state.t("Increase refresh interval")),
                ("[F2]", state.t("Select disk")),
                ("[F3]", state.t("Select network interface")),
            ],
        ),
        (
            state.t("Processes"),
            vec![
                ("[↑ / ↓]", state.t("Navigate list")),
                ("[Enter]", state.t("View process detail")),
                ("[/]", state.t("Toggle filter by name")),
                ("[Esc]", state.t("Clear filter")),
                ("[c]", state.t("Sort by CPU")),
                ("[m]", state.t("Sort by Memory")),
                ("[n]", state.t("Sort by Name")),
                ("[r]", state.t("Sort by disk read")),
                ("[w]", state.t("Sort by disk write")),
            ],
        ),
        (
            state.t("Containers"),
            vec![
                ("[↑ / ↓]", state.t("Navigate list")),
                ("[Enter]", state.t("View container detail")),
                ("[l]", state.t("View container logs")),
                ("[r]", state.t("Restart container")),
                ("[s]", state.t("Stop container")),
            ],
        ),
        (
            state.t("Container logs"),
            vec![
                ("[↑ / ↓]", state.t("Scroll logs")),
                ("[f]", state.t("Toggle auto-scroll")),
                ("[Esc]", state.t("Back to container detail")),
            ],
        ),
    ];

    // Divide el inner en columnas para las secciones
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(1); inner.height as usize])
        .split(inner);

    let mut line_idx: usize = 0;
    let render_line = |f: &mut Frame, rect: Rect, line: Line| {
        f.render_widget(Paragraph::new(line), rect);
    };

    for (section_title, bindings) in &sections {
        if line_idx >= rows.len() {
            break;
        }
        render_line(
            f,
            rows[line_idx],
            Line::from(vec![Span::styled(
                format!(" {} ", section_title),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )]),
        );
        line_idx += 1;

        for (key, desc) in bindings {
            if line_idx >= rows.len() {
                break;
            }
            render_line(
                f,
                rows[line_idx],
                Line::from(vec![
                    Span::styled(
                        format!("  {:12}", key),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*desc, Style::default().fg(Color::White)),
                ]),
            );
            line_idx += 1;
        }

        // Separador vacío entre secciones
        if line_idx < rows.len() {
            render_line(f, rows[line_idx], Line::from(""));
            line_idx += 1;
        }
    }

    // Pie
    let footer_line = Line::from(vec![Span::styled(
        format!("  {}", state.t("Version")),
        Style::default().fg(Color::DarkGray),
    )])
    .alignment(Alignment::Center);
    if line_idx < rows.len() {
        f.render_widget(Paragraph::new(footer_line), rows[line_idx]);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
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
        .split(popup_layout[1])[1]
}
