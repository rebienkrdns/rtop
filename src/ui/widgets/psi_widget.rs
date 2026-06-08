use std::collections::VecDeque;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph},
    Frame,
};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};
use ratatui::symbols::Marker;

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

#[allow(clippy::too_many_arguments)]
fn render_psi_chart(
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

    // Línea de leyenda
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("  {:.2}%  60s:{:.2}%  300s:{:.2}%", avg10, avg60, avg300),
                Style::default().fg(theme.muted),
            ),
        ])),
        chunks[0],
    );

    // Canvas braille
    let max_samples = range_samples as f64;
    let s_len = history.len();

    let canvas = Canvas::default()
        .block(Block::default().style(Style::default().bg(Color::Rgb(51, 52, 61))))
        .x_bounds([0.0, max_samples])
        .y_bounds([0.0, 100.0])
        .marker(Marker::Braille)
        .paint(move |ctx| {
            if s_len > 1 {
                let samples: Vec<f64> = history.iter().copied().collect();
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
                        color,
                    });
                }
            }
        });

    f.render_widget(Clear, chunks[1]);
    f.render_widget(canvas, chunks[1]);
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
                    Style::default().fg(theme.muted).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "(Solo Linux con CONFIG_PSI=y)",
                    Style::default().fg(theme.muted),
                )),
            ];
            f.render_widget(Paragraph::new(lines).alignment(ratatui::layout::Alignment::Center), area);
        }
        Some(psi) => {
            // Tres filas: CPU PSI, MEM PSI, I/O PSI
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Ratio(1, 3),
                    Constraint::Ratio(1, 3),
                    Constraint::Ratio(1, 3),
                ])
                .split(area);

            // Tail de historial al rango actual
            let tail = |deque: &VecDeque<f64>| -> VecDeque<f64> {
                let skip = deque.len().saturating_sub(range_samples);
                deque.iter().skip(skip).copied().collect()
            };

            render_psi_chart(
                f,
                chunks[0],
                &tail(&state.psi_history_cpu),
                range_samples,
                "CPU",
                psi.cpu_some.avg10,
                psi.cpu_some.avg60,
                psi.cpu_some.avg300,
            );

            render_psi_chart(
                f,
                chunks[1],
                &tail(&state.psi_history_mem),
                range_samples,
                "MEM",
                psi.memory_some.avg10,
                psi.memory_some.avg60,
                psi.memory_some.avg300,
            );

            render_psi_chart(
                f,
                chunks[2],
                &tail(&state.psi_history_io),
                range_samples,
                "I/O",
                psi.io_some.avg10,
                psi.io_some.avg60,
                psi.io_some.avg300,
            );
        }
    }
}
