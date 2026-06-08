use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::models::{ProcessData, ProcessSortColumn, ProcessStatus};
use crate::ui::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessStatusFilter {
    #[default]
    Running,
    Sleeping,
    All,
}

impl ProcessStatusFilter {
    pub fn label(self) -> &'static str {
        match self {
            ProcessStatusFilter::Running => "ejecutando",
            ProcessStatusFilter::Sleeping => "durmiendo",
            ProcessStatusFilter::All => "todos",
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
            status_filter: ProcessStatusFilter::Running,
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

pub fn render(f: &mut Frame, area: Rect, processes: &[ProcessData], state: &ProcessTableState) {
    let theme = Theme::default_theme();

    // Split: filter bar (1 line) + table
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Filter bar
    let status_label = state.status_filter.label();
    let filter_line = if state.filter_active {
        Line::from(vec![
            Span::styled("Filtrar: /", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(state.filter.as_str(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(theme.accent)),
        ])
    } else if !state.filter.is_empty() {
        Line::from(vec![
            Span::styled("Filtro: ", Style::default().fg(theme.muted)),
            Span::styled(state.filter.as_str(), Style::default().fg(Color::White)),
            Span::styled("  [ESC limpiar]  ", Style::default().fg(theme.muted)),
            Span::styled("f estado: ", Style::default().fg(theme.muted)),
            Span::styled(status_label, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "/ filtrar  c CPU  m RAM  r DiskR  w DiskW  f estado: ",
                Style::default().fg(theme.muted),
            ),
            Span::styled(status_label, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
            Span::styled("  ↑↓ navegar  Enter detalle", Style::default().fg(theme.muted)),
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

    // Apply sort
    filtered.sort_by(|a, b| {
        let ord = match state.sort_col {
            ProcessSortColumn::Cpu => a.cpu_pct.partial_cmp(&b.cpu_pct).unwrap_or(std::cmp::Ordering::Equal),
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
        };
        if state.sort_asc { ord } else { ord.reverse() }
    });

    let is_cpu = state.sort_col == ProcessSortColumn::Cpu;
    let is_mem = state.sort_col == ProcessSortColumn::Memory;
    let is_dr = state.sort_col == ProcessSortColumn::DiskRead;
    let is_dw = state.sort_col == ProcessSortColumn::DiskWrite;

    let header_style = Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);

    let header_cells = vec![
        Cell::from(Line::from(Span::styled("PID", header_style)).alignment(ratatui::layout::Alignment::Right)),
        Cell::from(Span::styled("Proceso", header_style)),
        Cell::from(Line::from(Span::styled(
            col_header("CPU%", is_cpu, state.sort_asc),
            if is_cpu { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { header_style }
        )).alignment(ratatui::layout::Alignment::Right)),
        Cell::from(Line::from(Span::styled(
            col_header("RAM", is_mem, state.sort_asc),
            if is_mem { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { header_style }
        )).alignment(ratatui::layout::Alignment::Right)),
        Cell::from(Line::from(Span::styled(
            col_header("Disco R", is_dr, state.sort_asc),
            if is_dr { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { header_style }
        )).alignment(ratatui::layout::Alignment::Right)),
        Cell::from(Line::from(Span::styled(
            col_header("Disco W", is_dw, state.sort_asc),
            if is_dw { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { header_style }
        )).alignment(ratatui::layout::Alignment::Right)),
        Cell::from(Span::styled("Estado", header_style)),
    ];
    let header_row = Row::new(header_cells).height(1);

    let visible_rows = (table_area.height as usize).saturating_sub(1); // minus header
    let scroll = state.scroll;
    let visible_end = (scroll + visible_rows).min(filtered.len());

    let mut rows = Vec::new();
    for (i, p) in filtered[scroll..visible_end].iter().enumerate() {
        let abs_idx = scroll + i;
        let selected = abs_idx == state.cursor;
        let row_style = if selected {
            Style::default().bg(theme.selected_bg).fg(theme.selected_fg)
        } else {
            Style::default()
        };

        let cpu_color = if selected { theme.selected_fg } else { Theme::color_for_pct(p.cpu_pct) };

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

        let pid_cell = Cell::from(Line::from(vec![
            Span::styled(
                format!("{}", p.pid),
                row_style.fg(if selected { sel_fg } else { normal_fg })
            )
        ]).alignment(ratatui::layout::Alignment::Right));

        let process_cell = Cell::from(Line::from(vec![
            Span::styled(
                p.name.clone(),
                row_style.fg(if selected { sel_fg } else { normal_fg })
            )
        ]));

        let cpu_cell = Cell::from(Line::from(vec![
            Span::styled(
                format!("{:.1}%", p.cpu_pct),
                row_style.fg(cpu_color)
            )
        ]).alignment(ratatui::layout::Alignment::Right));

        let ram_cell = Cell::from(Line::from(vec![
            Span::styled(
                format!("{}", ByteSize(p.memory_bytes)),
                row_style.fg(if selected { sel_fg } else { normal_fg })
            )
        ]).alignment(ratatui::layout::Alignment::Right));

        let disk_r_cell = Cell::from(Line::from(vec![
            Span::styled(
                fmt_rate(p.disk_read_per_sec),
                row_style.fg(if selected { sel_fg } else { theme.accent_dim })
            )
        ]).alignment(ratatui::layout::Alignment::Right));

        let disk_w_cell = Cell::from(Line::from(vec![
            Span::styled(
                fmt_rate(p.disk_write_per_sec),
                row_style.fg(if selected { sel_fg } else { theme.ok })
            )
        ]).alignment(ratatui::layout::Alignment::Right));

        let status_cell = Cell::from(Line::from(vec![
            Span::styled(
                p.status.to_string_es(),
                row_style.fg(status_color)
            )
        ]));

        let row = Row::new(vec![
            pid_cell,
            process_cell,
            cpu_cell,
            ram_cell,
            disk_r_cell,
            disk_w_cell,
            status_cell,
        ]).style(row_style);
        rows.push(row);
    }

    let constraints = vec![
        Constraint::Length(6),  // PID
        Constraint::Min(15),    // Process Name
        Constraint::Length(8),  // CPU%
        Constraint::Length(12), // RAM
        Constraint::Length(12), // Disk Read
        Constraint::Length(12), // Disk Write
        Constraint::Length(12), // Status
    ];

    let table = Table::new(rows, constraints)
        .header(header_row)
        .column_spacing(2);

    f.render_widget(table, table_area);
}
