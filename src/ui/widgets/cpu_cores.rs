use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::{CpuCoreData, CpuData};
use crate::ui::theme::Theme;
use crate::ui::widgets::cpu_bar;

pub fn render_cpu_cores(f: &mut Frame, area: Rect, cpu: &CpuData, data_loaded: bool) {
    if area.height < 2 || cpu.per_core.is_empty() {
        return;
    }
    let theme = Theme::default_theme();
    let core_count = cpu.per_core.len();

    let show_breakdown = cpu.user_pct.is_some();
    let show_ctx = cpu.ctx_switches_per_sec.is_some();

    let mut remaining = area.height.saturating_sub(2) as usize; // subtract aggregate row (height 2)

    let actually_breakdown = show_breakdown && remaining > 0;
    if actually_breakdown { remaining -= 1; }

    let actually_ctx = show_ctx && remaining > 0;
    if actually_ctx { remaining -= 1; }

    // Dynamic layout strategy based on core count and available vertical space
    let strat_2col_2row_req = (core_count + 1) / 2 * 2;
    let strat_2col_1row_req = (core_count + 1) / 2;
    let strat_4col_2row_req = (core_count + 3) / 4 * 2;

    let (columns, row_height, core_rows) = if strat_2col_2row_req <= remaining {
        (2, 2, (core_count + 1) / 2)
    } else if strat_2col_1row_req <= remaining {
        (2, 1, (core_count + 1) / 2)
    } else if strat_4col_2row_req <= remaining && area.width > 80 {
        (4, 2, (core_count + 3) / 4)
    } else {
        // Fallback to most compact layout possible
        let cols = if area.width > 80 { 4 } else { 2 };
        (cols, 1, (core_count + (cols - 1)) / cols)
    };

    let visible_core_rows = core_rows.min(remaining / row_height);

    // Build constraints
    let mut constraints = vec![Constraint::Length(2)]; // aggregate
    if actually_breakdown { constraints.push(Constraint::Length(1)); }
    if actually_ctx       { constraints.push(Constraint::Length(1)); }
    for _ in 0..visible_core_rows { constraints.push(Constraint::Length(row_height as u16)); }
    constraints.push(Constraint::Min(0));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut idx = 0;

    // Fila 0: barra de CPU agregada (using cpu_bar gauge)
    cpu_bar::render_with_loading(f, chunks[idx], cpu, data_loaded);
    idx += 1;

    // Fila 1: desglose USR/SYS/IOW/STL
    if actually_breakdown {
        render_cpu_breakdown(f, chunks[idx], cpu, &theme);
        idx += 1;
    }

    // Fila 2: CTX/s INT/s
    if actually_ctx {
        render_ctx_int(f, chunks[idx], cpu, &theme);
        idx += 1;
    }

    // Constraints for columns
    let col_constraints = if columns == 4 {
        vec![
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ]
    } else {
        vec![
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ]
    };

    // Filas de hilos
    for r in 0..visible_core_rows {
        let row_area = chunks[idx + r];
        let row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(row_area);

        for c in 0..columns {
            let core_idx = r * columns + c;
            if core_idx < core_count {
                let core = &cpu.per_core[core_idx];
                let usage = if data_loaded { core.usage_pct } else { 0.0 };
                let mut cell_area = row_chunks[c];
                if c < columns - 1 {
                    cell_area.width = cell_area.width.saturating_sub(1); // Espacio entre columnas
                }
                render_core_row(f, cell_area, core, usage, &theme);
            }
        }
    }
}

// Barra de bloque: █ llenos + ░ pista vacía (siempre visible aunque el uso sea 0%)
fn make_bar(usage: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let filled = ((usage / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn render_cpu_breakdown(f: &mut Frame, area: Rect, cpu: &CpuData, theme: &Theme) {
    let mut spans = vec![];

    if let Some(usr) = cpu.user_pct {
        spans.push(Span::styled("USR:", Style::default().fg(theme.muted)));
        spans.push(Span::styled(
            format!("{:.1}%", usr),
            Style::default().fg(theme.ok),
        ));
        spans.push(Span::raw(" "));
    }
    if let Some(sys) = cpu.system_pct {
        spans.push(Span::styled("SYS:", Style::default().fg(theme.muted)));
        spans.push(Span::styled(
            format!("{:.1}%", sys),
            Style::default().fg(theme.accent),
        ));
        spans.push(Span::raw(" "));
    }
    if let Some(iow) = cpu.iowait_pct {
        let alert = iow >= 20.0;
        let color = if alert { Color::Yellow } else { theme.text };
        spans.push(Span::styled("IOW:", Style::default().fg(theme.muted)));
        spans.push(Span::styled(format!("{:.1}%", iow), Style::default().fg(color)));
        if alert {
            spans.push(Span::styled("⚠", Style::default().fg(Color::Yellow)));
        }
        spans.push(Span::raw(" "));
    }
    if let Some(stl) = cpu.steal_pct {
        if stl > 0.1 {
            let alert = stl >= 5.0;
            let color = if alert { Color::Red } else { theme.text };
            spans.push(Span::styled("STL:", Style::default().fg(theme.muted)));
            spans.push(Span::styled(format!("{:3.1}%", stl), Style::default().fg(color)));
            if alert {
                spans.push(Span::styled("⚠", Style::default().fg(Color::Red)));
            }
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_ctx_int(f: &mut Frame, area: Rect, cpu: &CpuData, theme: &Theme) {
    let mut spans = vec![];
    if let Some(ctx) = cpu.ctx_switches_per_sec {
        spans.push(Span::styled("CTX:", Style::default().fg(theme.muted)));
        spans.push(Span::styled(fmt_kilo(ctx), Style::default().fg(theme.text)));
        spans.push(Span::styled("/s  ", Style::default().fg(theme.muted)));
    }
    if let Some(intr) = cpu.interrupts_per_sec {
        spans.push(Span::styled("INT:", Style::default().fg(theme.muted)));
        spans.push(Span::styled(fmt_kilo(intr), Style::default().fg(theme.text)));
        spans.push(Span::styled("/s", Style::default().fg(theme.muted)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn fmt_kilo(v: f64) -> String {
    if v >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

fn render_core_row(f: &mut Frame, area: Rect, core: &CpuCoreData, usage: f64, theme: &Theme) {
    let color = Theme::color_for_pct(usage);
    let type_label = core.core_type.label();
    let type_color = core.core_type.color();

    if area.height >= 2 {
        let label_area = Rect { height: 1, ..area };
        let bar_area = Rect { y: area.y + 1, height: 1, ..area };

        let label = Line::from(vec![
            Span::styled(
                format!("Core {:>2} ", core.core_id),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!("[{}] ", type_label),
                Style::default().fg(type_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>5.1}%", usage),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]);

        let bar_w = bar_area.width as usize;
        let bar = make_bar(usage, bar_w);

        f.render_widget(Paragraph::new(label), label_area);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(bar, Style::default().fg(color)))),
            bar_area,
        );
    } else {
        // " 0 P " (5 chars) + bar + "  XX%" (5 chars) = 10 chars fijos
        let bar_w = (area.width as usize).saturating_sub(10);
        let bar = make_bar(usage, bar_w);

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{:>2} ", core.core_id),
                    Style::default().fg(theme.muted),
                ),
                Span::styled(
                    format!("{} ", type_label),
                    Style::default().fg(type_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(bar, Style::default().fg(color)),
                Span::styled(
                    format!("{:>4.0}%", usage),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
            ])),
            area,
        );
    }
}
