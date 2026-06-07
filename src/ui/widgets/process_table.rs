use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::{ProcessData, ProcessSortColumn};
use crate::ui::theme::Theme;

pub struct ProcessTableState {
    pub filter: String,
    pub filter_active: bool,
    pub sort_col: ProcessSortColumn,
    pub sort_asc: bool,
    pub cursor: usize,
    pub scroll: usize,
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
            Span::styled("  [ESC limpiar]", Style::default().fg(theme.muted)),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "/ filtrar  c CPU  m RAM  r DiskR  w DiskW  ↑↓ navegar  Enter detalle",
                Style::default().fg(theme.muted),
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
            if filter_lower.is_empty() {
                true
            } else {
                p.name.to_lowercase().contains(&filter_lower)
            }
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

    // Header
    let header = Line::from(vec![
        Span::styled(format!("{:<22}", "Proceso"), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:>8}", col_header("CPU%", is_cpu, state.sort_asc)),
            if is_cpu { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Cyan) },
        ),
        Span::raw("  "),
        Span::styled(
            format!("{:>12}", col_header("RAM", is_mem, state.sort_asc)),
            if is_mem { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Cyan) },
        ),
        Span::raw("  "),
        Span::styled(
            format!("{:>12}", col_header("Disco R", is_dr, state.sort_asc)),
            if is_dr { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Cyan) },
        ),
        Span::raw("  "),
        Span::styled(
            format!("{:>12}", col_header("Disco W", is_dw, state.sort_asc)),
            if is_dw { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Cyan) },
        ),
        Span::raw("  "),
        Span::styled(format!("{:<12}", "Estado"), Style::default().fg(Color::Cyan)),
    ]);

    let visible_rows = (table_area.height as usize).saturating_sub(1); // minus header
    let scroll = state.scroll;
    let visible_end = (scroll + visible_rows).min(filtered.len());

    let mut lines = vec![header];

    for (i, p) in filtered[scroll..visible_end].iter().enumerate() {
        let abs_idx = scroll + i;
        let selected = abs_idx == state.cursor;
        let row_style = if selected {
            Style::default().bg(theme.accent).fg(Color::Black)
        } else {
            Style::default()
        };

        let cpu_color = if selected { Color::Black } else { Theme::color_for_pct(p.cpu_pct) };
        let name_display = if p.name.len() > 21 {
            format!("{:.19}…", &p.name[..19])
        } else {
            p.name.clone()
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{:<22}", name_display), row_style.fg(if selected { Color::Black } else { Color::White })),
            Span::styled(format!("{:>8.1}", p.cpu_pct), row_style.fg(cpu_color)),
            Span::raw("  "),
            Span::styled(format!("{:>12}", ByteSize(p.memory_bytes)), row_style.fg(if selected { Color::Black } else { Color::White })),
            Span::raw("  "),
            Span::styled(format!("{:>12}", fmt_rate(p.disk_read_per_sec)), row_style.fg(if selected { Color::Black } else { Color::Blue })),
            Span::raw("  "),
            Span::styled(format!("{:>12}", fmt_rate(p.disk_write_per_sec)), row_style.fg(if selected { Color::Black } else { Color::Yellow })),
            Span::raw("  "),
            Span::styled(
                format!("{:<12}", p.status.to_string_es()),
                row_style.fg(if selected { Color::Black } else { Color::Green }),
            ),
        ]));
    }

    // Status bar
    let status = format!(
        " {} procesos{}",
        filtered.len(),
        if !filter_lower.is_empty() { format!(" (filtrado de {})", processes.len()) } else { String::new() }
    );
    if let Some(last_line) = lines.last_mut() {
        let _ = last_line; // keep borrow checker happy
    }

    // Render table
    f.render_widget(Paragraph::new(lines), table_area);

    // Status line at bottom if space
    let status_area = Rect {
        x: table_area.x,
        y: table_area.y + table_area.height.saturating_sub(0),
        width: table_area.width,
        height: 0, // no extra space, skip
    };
    let _ = status_area;
    let _ = status;
}
