use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::models::GpuData;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, gpus: &[GpuData]) {
    if gpus.is_empty() {
        return;
    }

    let theme = Theme::default_theme();

    let rows_per_gpu = 4u16; // name, util bar, vram bar, separator line
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            gpus.iter()
                .map(|_| Constraint::Length(rows_per_gpu))
                .collect::<Vec<_>>(),
        )
        .split(area);

    for (i, gpu) in gpus.iter().enumerate() {
        if i >= chunks.len() {
            break;
        }
        render_gpu(f, chunks[i], gpu, &theme);
    }
}

fn render_gpu(f: &mut Frame, area: Rect, gpu: &GpuData, theme: &Theme) {
    if area.height < 3 {
        return;
    }

    let block = Block::default()
        .title(format!(
            " GPU {} — {} — {}°C ",
            gpu.index, gpu.name, gpu.temperature_c
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    // — Utilización CUDA —
    let util_label = Line::from(vec![
        Span::styled("CUDA", Style::default().fg(theme.accent)),
        Span::styled(
            format!("  {:3.0}%  {} procs", gpu.utilization_pct, gpu.processes.len()),
            Style::default().fg(theme.text),
        ),
    ]);
    f.render_widget(Paragraph::new(util_label), layout[0]);

    // — VRAM —
    let vram_pct = gpu.memory_usage_pct();
    let vram_label = Line::from(vec![
        Span::styled("VRAM", Style::default().fg(theme.accent)),
        Span::styled(
            format!(
                "  {:.1}%  {} / {}",
                vram_pct,
                ByteSize(gpu.memory_used_bytes),
                ByteSize(gpu.memory_total_bytes)
            ),
            Style::default().fg(theme.text),
        ),
    ]);

    // Split VRAM row into label + gauge
    if layout[1].width > 20 {
        let label_width = 35u16.min(layout[1].width / 2);
        let row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(label_width),
                Constraint::Min(1),
            ])
            .split(layout[1]);

        f.render_widget(Paragraph::new(vram_label), row[0]);

        let gauge = Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(Theme::color_for_pct(vram_pct))
                    .bg(Color::Rgb(51, 52, 61)),
            )
            .ratio((vram_pct / 100.0).clamp(0.0, 1.0))
            .label("");
        f.render_widget(gauge, row[1]);
    } else {
        f.render_widget(Paragraph::new(vram_label), layout[1]);
    }
}
