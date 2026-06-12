use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

use crate::models::{ProcessData, ProcessSortColumn, ProcessStatus};
use crate::ui::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessStatusFilter {
    #[default]
    All,
    Running,
    Sleeping,
}

impl ProcessStatusFilter {
    pub fn label(self, lang: crate::localization::Language) -> &'static str {
        match lang {
            crate::localization::Language::Spanish => match self {
                ProcessStatusFilter::Running => "ejecutando",
                ProcessStatusFilter::Sleeping => "durmiendo",
                ProcessStatusFilter::All => "todos",
            },
            crate::localization::Language::English => match self {
                ProcessStatusFilter::Running => "running",
                ProcessStatusFilter::Sleeping => "sleeping",
                ProcessStatusFilter::All => "all",
            },
        }
    }

    pub fn next(self) -> Self {
        match self {
            ProcessStatusFilter::Running => ProcessStatusFilter::Sleeping,
            ProcessStatusFilter::Sleeping => ProcessStatusFilter::All,
            ProcessStatusFilter::All => ProcessStatusFilter::Running,
        }
    }

    pub fn matches(self, status: ProcessStatus) -> bool {
        match self {
            ProcessStatusFilter::All => true,
            ProcessStatusFilter::Running => status == ProcessStatus::Running,
            ProcessStatusFilter::Sleeping => status == ProcessStatus::Sleeping,
        }
    }
}

pub struct ProcessTableState {
    pub filter: String,
    pub filter_active: bool,
    pub sort_col: ProcessSortColumn,
    pub sort_asc: bool,
    pub cursor: usize,
    pub scroll: usize,
    pub status_filter: ProcessStatusFilter,
}

impl Default for ProcessTableState {
    fn default() -> Self {
        Self {
            filter: String::new(),
            filter_active: false,
            sort_col: ProcessSortColumn::Cpu,
            sort_asc: false,
            cursor: 0,
            scroll: 0,
            status_filter: ProcessStatusFilter::All,
        }
    }
}

fn fmt_rate(rate: Option<f64>) -> String {
    match rate {
        Some(v) => format!("{}/s", ByteSize(v as u64)),
        None => "–".to_string(),
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

pub fn render(
    f: &mut Frame,
    area: Rect,
    processes: &[ProcessData],
    state: &ProcessTableState,
    lang: crate::localization::Language,
) {
    let theme = Theme::default_theme();
    let t = |key: &'static str| crate::localization::translate(key, lang);

    // Split: filter bar (1 line) + table
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Filter bar — shows hint text and active status filter chip
    let status_label = state.status_filter.label(lang);
    let filter_line = if state.filter_active {
        Line::from(vec![
            Span::styled(
                format!("{}: /", t("Filter")),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(state.filter.as_str(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(theme.accent)),
        ])
    } else if !state.filter.is_empty() {
        Line::from(vec![
            Span::styled(
                format!("{}: ", t("Filter")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(state.filter.as_str(), Style::default().fg(Color::White)),
            Span::styled(
                format!("  {}  ", t("Clean filter")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "  [f] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                status_label,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("/ ", Style::default().fg(theme.accent)),
            Span::styled(
                format!("{}  ", t("PressSlashToFilter")),
                Style::default().fg(theme.muted),
            ),
            Span::styled("c", Style::default().fg(theme.accent)),
            Span::styled(" CPU  ", Style::default().fg(theme.muted)),
            Span::styled("m", Style::default().fg(theme.accent)),
            Span::styled(" RAM  ", Style::default().fg(theme.muted)),
            Span::styled("r", Style::default().fg(theme.accent)),
            Span::styled(" DiskR  ", Style::default().fg(theme.muted)),
            Span::styled("w", Style::default().fg(theme.accent)),
            Span::styled(" DiskW  ", Style::default().fg(theme.muted)),
            Span::styled("i", Style::default().fg(theme.accent)),
            Span::styled(" NetRX  ", Style::default().fg(theme.muted)),
            Span::styled("o", Style::default().fg(theme.accent)),
            Span::styled(
                format!(" NetTX  ↑↓ {}  Enter {}  ", t("Navigate"), t("EnterDetail")),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                "[f] ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                status_label,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    };
    f.render_widget(Paragraph::new(filter_line), chunks[0]);

    let table_area = chunks[1];
    if table_area.height < 2 {
        return;
    }

    // Apply filter
    let filter_lower = state.filter.to_lowercase();
    let mut filtered: Vec<&ProcessData> = processes
        .iter()
        .filter(|p| {
            let name_ok = filter_lower.is_empty() || p.name.to_lowercase().contains(&filter_lower);
            let status_ok = state.status_filter.matches(p.status);
            name_ok && status_ok
        })
        .collect();

    // Render process count over filter bar (right-aligned)
    let count_text = if filtered.len() == processes.len() {
        format!(" {} {} ", filtered.len(), t("processes"))
    } else {
        format!(
            " {}/{} {} ",
            filtered.len(),
            processes.len(),
            t("processes")
        )
    };
    let count_widget = Paragraph::new(Line::from(Span::styled(
        count_text,
        Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(count_widget, chunks[0]);

    // Apply sort
    filtered.sort_by(|a, b| {
        let ord = match state.sort_col {
            ProcessSortColumn::Cpu => a
                .cpu_pct
                .partial_cmp(&b.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal),
            ProcessSortColumn::Memory => a.memory_bytes.cmp(&b.memory_bytes),
            ProcessSortColumn::DiskRead => {
                let ar = a.disk_read_per_sec.unwrap_or(0.0);
                let br = b.disk_read_per_sec.unwrap_or(0.0);
                ar.partial_cmp(&br).unwrap_or(std::cmp::Ordering::Equal)
            }
            ProcessSortColumn::DiskWrite => {
                let aw = a.disk_write_per_sec.unwrap_or(0.0);
                let bw = b.disk_write_per_sec.unwrap_or(0.0);
                aw.partial_cmp(&bw).unwrap_or(std::cmp::Ordering::Equal)
            }
            ProcessSortColumn::NetRx => {
                let ar = a.net_rx_per_sec.unwrap_or(0.0);
                let br = b.net_rx_per_sec.unwrap_or(0.0);
                ar.partial_cmp(&br).unwrap_or(std::cmp::Ordering::Equal)
            }
            ProcessSortColumn::NetTx => {
                let at = a.net_tx_per_sec.unwrap_or(0.0);
                let bt = b.net_tx_per_sec.unwrap_or(0.0);
                at.partial_cmp(&bt).unwrap_or(std::cmp::Ordering::Equal)
            }
            ProcessSortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        if state.sort_asc {
            ord
        } else {
            ord.reverse()
        }
    });

    let is_cpu = state.sort_col == ProcessSortColumn::Cpu;
    let is_mem = state.sort_col == ProcessSortColumn::Memory;
    let is_dr = state.sort_col == ProcessSortColumn::DiskRead;
    let is_dw = state.sort_col == ProcessSortColumn::DiskWrite;
    let is_nrx = state.sort_col == ProcessSortColumn::NetRx;
    let is_ntx = state.sort_col == ProcessSortColumn::NetTx;
    let is_name = state.sort_col == ProcessSortColumn::Name;

    // Only show network columns when at least one process has network data
    let show_net = processes
        .iter()
        .any(|p| p.net_rx_per_sec.is_some() || p.net_rx_total.is_some());

    let header_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);

    let mut header_cells = vec![
        Cell::from(
            Line::from(Span::styled("PID", header_style))
                .alignment(ratatui::layout::Alignment::Right),
        ),
        Cell::from(Span::styled(
            col_header(t("Process"), is_name, state.sort_asc),
            if is_name {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                header_style
            },
        )),
        Cell::from(
            Line::from(Span::styled(
                col_header("CPU%", is_cpu, state.sort_asc),
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
                col_header("RAM", is_mem, state.sort_asc),
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
                col_header(t("Disk R"), is_dr, state.sort_asc),
                if is_dr {
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
                col_header(t("Disk W"), is_dw, state.sort_asc),
                if is_dw {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ),
    ];

    if show_net {
        header_cells.push(Cell::from(
            Line::from(Span::styled(
                col_header("Net RX", is_nrx, state.sort_asc),
                if is_nrx {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ));
        header_cells.push(Cell::from(
            Line::from(Span::styled(
                col_header("Net TX", is_ntx, state.sort_asc),
                if is_ntx {
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    header_style
                },
            ))
            .alignment(ratatui::layout::Alignment::Right),
        ));
        header_cells.push(Cell::from(
            Line::from(Span::styled("Net RX Tot", header_style))
                .alignment(ratatui::layout::Alignment::Right),
        ));
        header_cells.push(Cell::from(
            Line::from(Span::styled("Net TX Tot", header_style))
                .alignment(ratatui::layout::Alignment::Right),
        ));
    }

    header_cells.push(Cell::from(Span::styled(t("State"), header_style)));

    let header_row = Row::new(header_cells).height(1);

    let visible_rows = (table_area.height as usize).saturating_sub(1); // minus header
    let mut scroll_offset = state.scroll;
    if state.cursor < scroll_offset {
        scroll_offset = state.cursor;
    } else if state.cursor >= scroll_offset + visible_rows {
        scroll_offset = state.cursor.saturating_sub(visible_rows - 1);
    }
    if scroll_offset + visible_rows > filtered.len() {
        scroll_offset = filtered.len().saturating_sub(visible_rows);
    }
    let visible_end = (scroll_offset + visible_rows).min(filtered.len());

    let mut rows = Vec::new();
    for (i, p) in filtered[scroll_offset..visible_end].iter().enumerate() {
        let abs_idx = scroll_offset + i;
        let selected = abs_idx == state.cursor;
        let row_style = if selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default()
        };

        let cpu_color = if selected {
            theme.selected_fg
        } else {
            Theme::color_for_pct(p.cpu_pct)
        };

        let normal_fg = theme.text;
        let sel_fg = theme.selected_fg;

        let status_color = if selected {
            sel_fg
        } else {
            match p.status {
                crate::models::ProcessStatus::Running => theme.ok,
                _ => theme.muted,
            }
        };

        let pid_cell = Cell::from(
            Line::from(vec![Span::styled(
                format!("{}", p.pid),
                row_style.fg(if selected { sel_fg } else { normal_fg }),
            )])
            .alignment(ratatui::layout::Alignment::Right),
        );

        let process_cell = Cell::from(Line::from(vec![Span::styled(
            p.name.clone(),
            row_style.fg(if selected { sel_fg } else { normal_fg }),
        )]));

        let cpu_cell = Cell::from(
            Line::from(vec![Span::styled(
                format!("{:.1}%", p.cpu_pct),
                row_style.fg(cpu_color),
            )])
            .alignment(ratatui::layout::Alignment::Right),
        );

        let ram_cell = Cell::from(
            Line::from(vec![Span::styled(
                format!("{}", ByteSize(p.memory_bytes)),
                row_style.fg(if selected { sel_fg } else { normal_fg }),
            )])
            .alignment(ratatui::layout::Alignment::Right),
        );

        let disk_r_cell = Cell::from(
            Line::from(vec![Span::styled(
                fmt_rate(p.disk_read_per_sec),
                row_style.fg(if selected { sel_fg } else { theme.accent_dim }),
            )])
            .alignment(ratatui::layout::Alignment::Right),
        );

        let disk_w_cell = Cell::from(
            Line::from(vec![Span::styled(
                fmt_rate(p.disk_write_per_sec),
                row_style.fg(if selected { sel_fg } else { theme.ok }),
            )])
            .alignment(ratatui::layout::Alignment::Right),
        );

        let status_cell = Cell::from(Line::from(vec![Span::styled(
            p.status.to_localized_str(lang),
            row_style.fg(status_color),
        )]));

        let mut cells = vec![
            pid_cell,
            process_cell,
            cpu_cell,
            ram_cell,
            disk_r_cell,
            disk_w_cell,
        ];

        if show_net {
            let net_rx_color = if selected { sel_fg } else { Color::Cyan };
            let net_tx_color = if selected { sel_fg } else { Color::Magenta };

            cells.push(Cell::from(
                Line::from(vec![Span::styled(
                    fmt_rate(p.net_rx_per_sec),
                    row_style.fg(net_rx_color),
                )])
                .alignment(ratatui::layout::Alignment::Right),
            ));
            cells.push(Cell::from(
                Line::from(vec![Span::styled(
                    fmt_rate(p.net_tx_per_sec),
                    row_style.fg(net_tx_color),
                )])
                .alignment(ratatui::layout::Alignment::Right),
            ));
            cells.push(Cell::from(
                Line::from(vec![Span::styled(
                    p.net_rx_total
                        .map(|v| ByteSize(v).to_string())
                        .unwrap_or_else(|| "–".to_string()),
                    row_style.fg(if selected { sel_fg } else { net_rx_color }),
                )])
                .alignment(ratatui::layout::Alignment::Right),
            ));
            cells.push(Cell::from(
                Line::from(vec![Span::styled(
                    p.net_tx_total
                        .map(|v| ByteSize(v).to_string())
                        .unwrap_or_else(|| "–".to_string()),
                    row_style.fg(if selected { sel_fg } else { net_tx_color }),
                )])
                .alignment(ratatui::layout::Alignment::Right),
            ));
        }

        cells.push(status_cell);

        let row = Row::new(cells).style(row_style);
        rows.push(row);
    }

    let mut constraints = vec![
        Constraint::Length(6),  // PID
        Constraint::Min(12),    // Process Name
        Constraint::Length(8),  // CPU%
        Constraint::Length(10), // RAM
        Constraint::Length(10), // Disk Read
        Constraint::Length(10), // Disk Write
    ];
    if show_net {
        constraints.push(Constraint::Length(10)); // Net RX/s
        constraints.push(Constraint::Length(10)); // Net TX/s
        constraints.push(Constraint::Length(10)); // Net RX Tot
        constraints.push(Constraint::Length(10)); // Net TX Tot
    }
    constraints.push(Constraint::Length(10)); // Status

    let table = Table::new(rows, constraints)
        .header(header_row)
        .column_spacing(2);

    f.render_widget(table, table_area);

    if filtered.len() > visible_rows {
        let max_scroll = filtered.len().saturating_sub(visible_rows);
        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(scroll_offset);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .track_symbol(None)
                .thumb_symbol("┃"),
            table_area,
            &mut scrollbar_state,
        );
    }
}
