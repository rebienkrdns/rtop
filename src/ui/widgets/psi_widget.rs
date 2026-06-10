use std::collections::VecDeque;

use ratatui::symbols::Marker;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::theme::Theme;

fn psi_color(avg10: f64) -> Color {
    if avg10 >= 20.0 {
        Color::Rgb(255, 180, 171) // rojo crítico
    } else if avg10 >= 5.0 {
        Color::Rgb(235, 192, 109) // amarillo advertencia
    } else {
        Color::Rgb(165, 213, 102) // verde sano
    }
}

// Color fijo para la línea `full` (azul claro, contrasta con el color dinámico de `some`)
const COLOR_FULL: Color = Color::Rgb(100, 180, 255);

/// Dibuja un gráfico PSI con una sola línea (CPU usa solo `some`).
#[allow(clippy::too_many_arguments)]
fn render_psi_chart_single(
    f: &mut Frame,
    area: Rect,
    history: &VecDeque<f64>,
    range_samples: usize,
    label: &str,
    avg10: f64,
    avg60: f64,
    avg300: f64,
) {
    if area.height < 2 {
        return;
    }

    let theme = Theme::default_theme();
    let color = psi_color(avg10);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                label,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  avg10:{:.2}%  60s:{:.2}%  300s:{:.2}%",
                    avg10, avg60, avg300
                ),
                Style::default().fg(theme.muted),
            ),
        ])),
        chunks[0],
    );

    render_canvas_lines(f, chunks[1], range_samples, &[(history, color)]);
}

/// Dibuja un gráfico PSI con dos líneas (`some` + `full`) y header partido.
/// Header: "LABEL  some→avg10/avg60/avg300   full→avg10/avg60/avg300"
#[allow(clippy::too_many_arguments)]
fn render_psi_chart_dual(
    f: &mut Frame,
    area: Rect,
    history_some: &VecDeque<f64>,
    history_full: &VecDeque<f64>,
    range_samples: usize,
    label: &str,
    some_avg10: f64,
    some_avg60: f64,
    some_avg300: f64,
    full_avg10: f64,
    full_avg60: f64,
    full_avg300: f64,
) {
    if area.height < 2 {
        return;
    }

    let theme = Theme::default_theme();
    let color_some = psi_color(some_avg10);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    // Header: etiqueta | some values (izq) | full values (der)
    let header = Line::from(vec![
        Span::styled(
            label,
            Style::default().fg(color_some).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  some ", Style::default().fg(color_some)),
        Span::styled(
            format!(
                "avg10:{:.2}% 60s:{:.2}% 300s:{:.2}%",
                some_avg10, some_avg60, some_avg300
            ),
            Style::default().fg(theme.muted),
        ),
        Span::styled("   full ", Style::default().fg(COLOR_FULL)),
        Span::styled(
            format!(
                "avg10:{:.2}% 60s:{:.2}% 300s:{:.2}%",
                full_avg10, full_avg60, full_avg300
            ),
            Style::default().fg(theme.muted),
        ),
    ]);

    f.render_widget(Paragraph::new(header), chunks[0]);

    render_canvas_lines(
        f,
        chunks[1],
        range_samples,
        &[(history_some, color_some), (history_full, COLOR_FULL)],
    );
}

/// Dibuja una o más líneas braille en un canvas compartido.
fn render_canvas_lines(
    f: &mut Frame,
    area: Rect,
    range_samples: usize,
    series: &[(&VecDeque<f64>, Color)],
) {
    let max_samples = range_samples as f64;

    // Clonar datos para move closure
    let series_owned: Vec<(Vec<f64>, Color)> = series
        .iter()
        .map(|(deque, color)| (deque.iter().copied().collect(), *color))
        .collect();

    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(Color::Rgb(51, 52, 61))))
        .x_bounds([0.0, max_samples])
        .y_bounds([0.0, 100.0])
        .marker(Marker::Braille)
        .paint(move |ctx| {
            for (samples, color) in &series_owned {
                let s_len = samples.len();
                if s_len > 1 {
                    for i in 0..(s_len - 1) {
                        let x1 = (max_samples - (s_len - 1 - i) as f64).max(0.0);
                        let x2 = max_samples - (s_len - 1 - (i + 1)) as f64;
                        if x2 < 0.0 {
                            continue;
                        }
                        ctx.draw(&CanvasLine {
                            x1,
                            y1: samples[i],
                            x2,
                            y2: samples[i + 1],
                            color: *color,
                        });
                    }
                }
            }
        });

    f.render_widget(Clear, area);
    f.render_widget(canvas, area);
}

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let theme = Theme::default_theme();
    let range_samples = state.history_range.samples();

    match state.psi.as_ref() {
        None => {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "PSI no disponible",
                    Style::default()
                        .fg(theme.muted)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "(Solo Linux con CONFIG_PSI=y)",
                    Style::default().fg(theme.muted),
                )),
            ];
            f.render_widget(
                Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center),
                area,
            );
        }
        Some(psi) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Ratio(1, 3),
                    Constraint::Ratio(1, 3),
                    Constraint::Ratio(1, 3),
                ])
                .split(area);

            let tail = |deque: &VecDeque<f64>| -> VecDeque<f64> {
                let skip = deque.len().saturating_sub(range_samples);
                deque.iter().skip(skip).copied().collect()
            };

            // CPU — solo `some` (no existe `cpu_full` en el kernel)
            render_psi_chart_single(
                f,
                chunks[0],
                &tail(&state.psi_history_cpu),
                range_samples,
                "CPU",
                psi.cpu_some.avg10,
                psi.cpu_some.avg60,
                psi.cpu_some.avg300,
            );

            // MEM — dos líneas: some (color dinámico) + full (azul)
            render_psi_chart_dual(
                f,
                chunks[1],
                &tail(&state.psi_history_mem),
                &tail(&state.psi_history_mem_full),
                range_samples,
                "MEM",
                psi.memory_some.avg10,
                psi.memory_some.avg60,
                psi.memory_some.avg300,
                psi.memory_full.avg10,
                psi.memory_full.avg60,
                psi.memory_full.avg300,
            );

            // I/O — dos líneas: some (color dinámico) + full (azul)
            render_psi_chart_dual(
                f,
                chunks[2],
                &tail(&state.psi_history_io),
                &tail(&state.psi_history_io_full),
                range_samples,
                "I/O",
                psi.io_some.avg10,
                psi.io_some.avg60,
                psi.io_some.avg300,
                psi.io_full.avg10,
                psi.io_full.avg60,
                psi.io_full.avg300,
            );
        }
    }
}
