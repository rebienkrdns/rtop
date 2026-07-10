use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, state: &AppState) {
    let theme = Theme::default_theme();
    let area = centered_rect(75, 60, f.size());

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
    // Calculate total speeds (excluding loopback)
    let loopback_names: Vec<&str> = state
        .available_nics
        .iter()
        .filter(|n| n.is_loopback)
        .map(|n| n.name.as_str())
        .collect();
    let mut total_rx = 0.0;
    let mut total_tx = 0.0;
    for (name, data) in &state.network_by_nic {
        if !loopback_names.contains(&name.as_str()) {
            total_rx += data.recv_bytes_per_sec;
            total_tx += data.sent_bytes_per_sec;
        }
    }
    let total_max_bw = state
        .network_max_bw_by_nic
        .iter()
        .filter(|(name, _)| !loopback_names.contains(&name.as_str()))
        .map(|(_, &v)| v)
        .fold(0.0_f64, f64::max)
        .max(125_000_000.0);
    let total_pct = ((total_rx + total_tx) / total_max_bw * 100.0).clamp(0.0, 100.0);

    let all_item = Row::new(vec![
        Cell::from(Span::styled(all_prefix, all_style)),
        Cell::from(Span::styled(state.t("AllNICs"), all_style)),
        Cell::from(Span::styled(
            state.t("Summation"),
            Style::default().fg(theme.muted),
        )),
        Cell::from(Span::styled(
            format!("↓{:>8}", fmt_bps_short(total_rx)),
            Style::default().fg(theme.ok),
        )),
        Cell::from(Span::styled(
            format!("↑{:>8}", fmt_bps_short(total_tx)),
            Style::default().fg(theme.ok),
        )),
        Cell::from(Span::styled(
            format!("{:>5.1}%", total_pct),
            Style::default().fg(theme.ok),
        )),
        Cell::from(Span::styled(
            format!(
                "★ {}",
                if all_is_selected {
                    state.t("NicSelected")
                } else {
                    state.t("SumOfNICs")
                }
            ),
            all_status_style,
        )),
    ]);

    let mut rows: Vec<Row> = vec![all_item];

    for (i, nic) in state.available_nics.iter().enumerate() {
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

        let (rx, tx, pct) = if let Some(net_data) = state.network_by_nic.get(&nic.name) {
            let bw = state
                .network_max_bw_by_nic
                .get(&nic.name)
                .copied()
                .unwrap_or(125_000_000.0);
            let p = ((net_data.recv_bytes_per_sec + net_data.sent_bytes_per_sec) / bw * 100.0)
                .clamp(0.0, 100.0);
            (net_data.recv_bytes_per_sec, net_data.sent_bytes_per_sec, p)
        } else {
            (0.0, 0.0, 0.0)
        };

        let (rx_str, tx_str, pct_str) = if nic.is_up {
            (
                format!("↓{:>8}", fmt_bps_short(rx)),
                format!("↑{:>8}", fmt_bps_short(tx)),
                format!("{:>5.1}%", pct),
            )
        } else {
            ("—".to_string(), "".to_string(), "".to_string())
        };

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

        let traffic_style = if !nic.is_up {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(theme.ok)
        };

        let status_style = Style::default().fg(if nic.is_up {
            Color::Green
        } else {
            Color::DarkGray
        });

        rows.push(Row::new(vec![
            Cell::from(Span::styled(prefix, name_style)),
            Cell::from(Span::styled(nic.name.clone(), name_style)),
            Cell::from(Span::styled(ip_str, ip_style)),
            Cell::from(Span::styled(rx_str, traffic_style)),
            Cell::from(Span::styled(tx_str, traffic_style)),
            Cell::from(Span::styled(pct_str, traffic_style)),
            Cell::from(Span::styled(format!("{} {}", bullet, status), status_style)),
        ]));
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from(state.t("Interface")),
        Cell::from(state.t("IP")),
        Cell::from(format!("{:>9}", state.t("Read"))),
        Cell::from(format!("{:>9}", state.t("Write"))),
        Cell::from(format!("{:>6}", "%")),
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
            Constraint::Length(10),
            Constraint::Min(15), // IP eats extra space
            Constraint::Length(9),
            Constraint::Length(9),
            Constraint::Length(6),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .column_spacing(1);

    f.render_widget(table, layout[0]);

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
