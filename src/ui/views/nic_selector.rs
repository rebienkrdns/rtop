use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, state: &AppState) {
    let theme = Theme::default_theme();
    let area = centered_rect(62, 60, f.size());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("SelectNIC")),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    // Build list: index 0 = "Todas las interfaces", index 1..N = individual NICs
    let all_is_cursor = state.nic_cursor == 0;
    let all_is_selected = state.selected_nic.is_none();
    let all_prefix = if all_is_cursor { "> " } else { "  " };
    let all_style = if all_is_cursor || all_is_selected {
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let all_status_style = Style::default().fg(if all_is_selected {
        Color::Green
    } else {
        theme.muted
    });
    let all_item = ListItem::new(Line::from(vec![
        Span::styled(all_prefix, all_style),
        Span::styled(format!("{:<12}", state.t("AllNICs")), all_style),
        Span::styled(
            format!("{:<18}", state.t("Summation")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            format!(
                "★ {}",
                if all_is_selected {
                    state.t("NicSelected")
                } else {
                    state.t("SumOfNICs")
                }
            ),
            all_status_style,
        ),
    ]));

    let mut items: Vec<ListItem> = vec![all_item];

    for (i, nic) in state.available_nics.iter().enumerate() {
        // cursor index for this NIC is i+1 (because 0 = All)
        let is_selected = Some(&nic.name) == state.selected_nic.as_ref();
        let is_cursor = (i + 1) == state.nic_cursor;

        let bullet = if nic.is_up { "●" } else { "○" };
        let selected_status;
        let status: &str = if nic.is_loopback {
            "loopback"
        } else if is_selected {
            selected_status = format!("{} ({})", state.t("Active"), state.t("NicSelected"));
            &selected_status
        } else if nic.is_up {
            state.t("Active")
        } else {
            state.t("Inactive")
        };

        let ip_str = nic.ip_address.as_deref().unwrap_or("—");
        let prefix = if is_cursor { "> " } else { "  " };

        let name_style = if !nic.is_up {
            Style::default().fg(Color::DarkGray)
        } else if is_cursor {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let ip_style = if !nic.is_up {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let status_style = Style::default().fg(if nic.is_up {
            Color::Green
        } else {
            Color::DarkGray
        });

        let line = Line::from(vec![
            Span::styled(prefix, name_style),
            Span::styled(format!("{:<12}", nic.name), name_style),
            Span::styled(format!("{:<18}", ip_str), ip_style),
            Span::styled(format!("{} {}", bullet, status), status_style),
        ]);

        items.push(ListItem::new(line));
    }

    f.render_widget(List::new(items), layout[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " ↑↓",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}  ", state.t("Navigate")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "Enter",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}  ", state.t("Selected")),
            Style::default().fg(theme.muted),
        ),
        Span::styled(
            "ESC",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", state.t("Cancel")),
            Style::default().fg(theme.muted),
        ),
    ]));
    f.render_widget(footer, layout[1]);
}

fn centered_rect(width_pct: u16, height_pct: u16, r: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(vert[1])[1]
}
