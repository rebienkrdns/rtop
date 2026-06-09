use std::collections::HashSet;

use bytesize::ByteSize;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::{build_container_visual_rows, ContainerVisualRow};
use crate::models::{ContainerData, ContainerSortColumn, ContainerStatus};
use crate::ui::theme::Theme;

fn format_bps(bps: f64) -> String {
    format!("{}/s", ByteSize(bps as u64))
}

fn status_color(status: &ContainerStatus) -> Color {
    match status {
        ContainerStatus::Running => Color::Rgb(165, 213, 102),
        ContainerStatus::Paused => Color::Rgb(235, 192, 109),
        ContainerStatus::Restarting => Color::Rgb(235, 192, 109),
        ContainerStatus::Exited => Color::Rgb(136, 147, 145),
        ContainerStatus::Dead => Color::Rgb(255, 180, 171),
        ContainerStatus::Unknown => Color::Rgb(136, 147, 145),
    }
}

fn col_header(label: &str, active: bool, asc: bool) -> String {
    if active {
        let indicator = if asc { "▲" } else { "▼" };
        format!("{} {}", label, indicator)
    } else {
        label.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_with_cursor(
    f: &mut Frame,
    area: Rect,
    containers: &[ContainerData],
    cursor: usize,
    sort_col: ContainerSortColumn,
    sort_asc: bool,
    collapsed_groups: &HashSet<String>,
    lang: crate::localization::Language,
) {
    let theme = Theme::default_theme();
    let t = |key: &'static str| crate::localization::translate(key, lang);

    if containers.is_empty() {
        let msg = Paragraph::new(Line::from(Span::styled(
            format!("  {}", t("NoContainers")),
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(msg, area);
        return;
    }

    let is_wide = f.size().width >= 120;

    let constraints = if is_wide {
        vec![
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Min(12),
            ratatui::layout::Constraint::Length(7),
            ratatui::layout::Constraint::Length(15),
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(10),
        ]
    } else {
        vec![
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Min(10),
            ratatui::layout::Constraint::Length(7),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(10),
        ]
    };

    let is_cpu = sort_col == ContainerSortColumn::Cpu;
    let is_mem = sort_col == ContainerSortColumn::Memory;
    let is_net_rx = sort_col == ContainerSortColumn::NetRecv;
    let is_net_tx = sort_col == ContainerSortColumn::NetSent;
    let is_disk_r = sort_col == ContainerSortColumn::DiskRead;
    let is_disk_w = sort_col == ContainerSortColumn::DiskWrite;

    let header_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);

    let header_cells = vec![
        Cell::from(Span::styled("ID", header_style)),
        Cell::from(Span::styled(
            col_header(
                t("Process"),
                sort_col == ContainerSortColumn::Name,
                sort_asc,
            ),
            if sort_col == ContainerSortColumn::Name {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                header_style
            },
        )),
        Cell::from(
            Line::from(Span::styled(
                col_header("CPU%", is_cpu, sort_asc),
                if is_cpu {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled(
                col_header("RAM", is_mem, sort_asc),
                if is_mem {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled(
                col_header(
                    &if is_wide {
                        format!("{} ↓ (Tot)", t("Network"))
                    } else {
                        format!("{} ↓", t("Network"))
                    },
                    is_net_rx,
                    sort_asc,
                ),
                if is_net_rx {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled(
                col_header(
                    &if is_wide {
                        format!("{} ↑ (Tot)", t("Network"))
                    } else {
                        format!("{} ↑", t("Network"))
                    },
                    is_net_tx,
                    sort_asc,
                ),
                if is_net_tx {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled(
                col_header(
                    &if is_wide {
                        format!("{} R (Tot)", t("Disk"))
                    } else {
                        format!("{} R", t("Disk"))
                    },
                    is_disk_r,
                    sort_asc,
                ),
                if is_disk_r {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(
            Line::from(Span::styled(
                col_header(
                    &if is_wide {
                        format!("{} W (Tot)", t("Disk"))
                    } else {
                        format!("{} W", t("Disk"))
                    },
                    is_disk_w,
                    sort_asc,
                ),
                if is_disk_w {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(Span::styled(t("State"), header_style)),
    ];
    let header_row = Row::new(header_cells).height(1);

    let visual_rows = build_container_visual_rows(containers, collapsed_groups);
    let group_header_bg = Color::Rgb(40, 44, 52);
    let group_header_fg = Color::Rgb(180, 190, 200);

    let mut rows = Vec::new();
    for (row_idx, visual_row) in visual_rows.iter().enumerate() {
        let selected = row_idx == cursor;
        let row_style = if selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default()
        };

        match visual_row {
            ContainerVisualRow::GroupHeader {
                label,
                count,
                collapsed,
                cpu_sum,
                mem_sum,
                ..
            } => {
                let toggle_icon = if *collapsed { "▶" } else { "▼" };
                let header_text = format!(
                    "{} {} ({} containers)  CPU:{:.1}%  RAM:{}",
                    toggle_icon,
                    label,
                    count,
                    cpu_sum,
                    ByteSize(*mem_sum)
                );

                let style = if selected {
                    row_style
                } else {
                    Style::default().bg(group_header_bg).fg(group_header_fg).add_modifier(Modifier::BOLD)
                };

                // Group header spans all columns via a single wide cell + empty cells
                let cells: Vec<Cell> = vec![
                    Cell::from(Line::from(vec![Span::styled(header_text, style)])),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ];
                rows.push(Row::new(cells).style(style));
            }
            ContainerVisualRow::Container { real_idx } => {
                let c = &containers[*real_idx];
                let sel_fg = theme.selected_fg;
                let normal_fg = theme.text;

                // Indent name for containers that belong to a group
                let is_in_group = c.compose_project.is_some();
                let name_prefix = if is_in_group { "  " } else { "" };

                let id_cell = Cell::from(Line::from(vec![Span::styled(
                    format!("{:<8}", c.id.chars().take(8).collect::<String>()),
                    row_style.fg(if selected { sel_fg } else { normal_fg }),
                )]));

                let name_cell = Cell::from(Line::from(vec![Span::styled(
                    format!("{}{}", name_prefix, c.name),
                    row_style.fg(if selected { sel_fg } else { normal_fg }),
                )]));

                let cpu_color = if selected {
                    sel_fg
                } else {
                    Theme::color_for_pct(c.cpu_pct)
                };
                let cpu_cell = Cell::from(
                    Line::from(vec![Span::styled(
                        format!("{:.1}%", c.cpu_pct),
                        row_style.fg(cpu_color),
                    )])
                    .alignment(ratatui::layout::Alignment::Right),
                );

                let ram_str = format!("{} ({:.1}%)", ByteSize(c.memory_bytes), c.memory_pct);
                let ram_cell = Cell::from(
                    Line::from(vec![Span::styled(
                        ram_str,
                        row_style.fg(if selected { sel_fg } else { normal_fg }),
                    )])
                    .alignment(ratatui::layout::Alignment::Right),
                );

                let rx_str = if is_wide {
                    format!(
                        "{} ({})",
                        format_bps(c.net_recv_per_sec),
                        ByteSize(c.net_recv_total)
                    )
                } else {
                    format_bps(c.net_recv_per_sec)
                };
                let rx_cell = Cell::from(
                    Line::from(vec![Span::styled(
                        rx_str,
                        row_style.fg(if selected { sel_fg } else { theme.ok }),
                    )])
                    .alignment(ratatui::layout::Alignment::Right),
                );

                let tx_str = if is_wide {
                    format!(
                        "{} ({})",
                        format_bps(c.net_sent_per_sec),
                        ByteSize(c.net_sent_total)
                    )
                } else {
                    format_bps(c.net_sent_per_sec)
                };
                let tx_cell = Cell::from(
                    Line::from(vec![Span::styled(
                        tx_str,
                        row_style.fg(if selected { sel_fg } else { theme.accent_dim }),
                    )])
                    .alignment(ratatui::layout::Alignment::Right),
                );

                let disk_r_str = if is_wide {
                    format!(
                        "{} ({})",
                        format_bps(c.disk_read_per_sec),
                        ByteSize(c.disk_read_total)
                    )
                } else {
                    format_bps(c.disk_read_per_sec)
                };
                let disk_r_cell = Cell::from(
                    Line::from(vec![Span::styled(
                        disk_r_str,
                        row_style.fg(if selected { sel_fg } else { theme.accent_dim }),
                    )])
                    .alignment(ratatui::layout::Alignment::Right),
                );

                let disk_w_str = if is_wide {
                    format!(
                        "{} ({})",
                        format_bps(c.disk_write_per_sec),
                        ByteSize(c.disk_write_total)
                    )
                } else {
                    format_bps(c.disk_write_per_sec)
                };
                let disk_w_cell = Cell::from(
                    Line::from(vec![Span::styled(
                        disk_w_str,
                        row_style.fg(if selected { sel_fg } else { theme.ok }),
                    )])
                    .alignment(ratatui::layout::Alignment::Right),
                );

                let status_indicator = "● ";
                let scolor = if selected {
                    sel_fg
                } else {
                    status_color(&c.status)
                };
                let status_text = t(c.status.as_str());
                let status_cell = Cell::from(Line::from(vec![
                    Span::styled(status_indicator, row_style.fg(scolor)),
                    Span::styled(
                        status_text,
                        row_style.fg(if selected { sel_fg } else { normal_fg }),
                    ),
                ]));

                let row = Row::new(vec![
                    id_cell,
                    name_cell,
                    cpu_cell,
                    ram_cell,
                    rx_cell,
                    tx_cell,
                    disk_r_cell,
                    disk_w_cell,
                    status_cell,
                ])
                .style(row_style);
                rows.push(row);
            }
        }
    }

    let table = Table::new(rows, constraints)
        .header(header_row)
        .column_spacing(2);

    f.render_widget(table, area);
}
