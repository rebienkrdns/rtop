use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::models::PsiData;
use crate::ui::theme::Theme;

pub fn render(f: &mut Frame, area: Rect, psi_opt: Option<&PsiData>, is_wide: bool) {
    let theme = Theme::default_theme();

    let psi = match psi_opt {
        Some(p) => p,
        _ => {
            // Draw unavailable message (usually 3 lines centered)
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
            let msg = Paragraph::new(lines).alignment(Alignment::Center);
            f.render_widget(msg, area);
            return;
        }
    };

    if is_wide {
        // Wide view table (Resource, avg10, avg60, avg300)
        let header = Line::from(vec![
            Span::styled(
                format!("{:<11}", "Recurso"),
                Style::default().fg(theme.muted),
            ),
            Span::styled(format!("{:<10}", "avg10"), Style::default().fg(theme.muted)),
            Span::styled(format!("{:<10}", "avg60"), Style::default().fg(theme.muted)),
            Span::styled(
                format!("{:<10}", "avg300"),
                Style::default().fg(theme.muted),
            ),
        ]);

        let cpu_line = Line::from(vec![
            Span::styled(
                format!("{:<11}", "CPU some"),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.cpu_some.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.cpu_some.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.cpu_some.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let mem_some_line = Line::from(vec![
            Span::styled(
                format!("{:<11}", "MEM some"),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.memory_some.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.memory_some.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.memory_some.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let mem_full_line = Line::from(vec![
            Span::styled(
                format!("{:<11}", "    full"),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.memory_full.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.memory_full.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.memory_full.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let io_some_line = Line::from(vec![
            Span::styled(
                format!("{:<11}", "I/O some"),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.io_some.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.io_some.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.io_some.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let io_full_line = Line::from(vec![
            Span::styled(
                format!("{:<11}", "    full"),
                Style::default().fg(theme.muted),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.io_full.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.io_full.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<10.2}%", psi.io_full.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let lines = vec![
            header,
            cpu_line,
            Line::from(""), // spacer
            mem_some_line,
            mem_full_line,
            Line::from(""), // spacer
            io_some_line,
            io_full_line,
        ];
        f.render_widget(Paragraph::new(lines), area);
    } else {
        // Compact view (Resource, 10s, 60s, 300s - showing 'some' pressure only)
        let header = Line::from(vec![
            Span::styled(
                format!("{:<11}", "PSI (some)"),
                Style::default().fg(theme.muted),
            ),
            Span::styled(format!("{:<8}", "10s"), Style::default().fg(theme.muted)),
            Span::styled(format!("{:<8}", "60s"), Style::default().fg(theme.muted)),
            Span::styled(format!("{:<8}", "300s"), Style::default().fg(theme.muted)),
        ]);

        let cpu_line = Line::from(vec![
            Span::styled(format!("{:<11}", "CPU"), Style::default().fg(theme.text)),
            Span::styled(
                format!("{:<8.2}%", psi.cpu_some.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<8.2}%", psi.cpu_some.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<8.2}%", psi.cpu_some.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let mem_line = Line::from(vec![
            Span::styled(format!("{:<11}", "MEM"), Style::default().fg(theme.text)),
            Span::styled(
                format!("{:<8.2}%", psi.memory_some.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<8.2}%", psi.memory_some.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<8.2}%", psi.memory_some.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let io_line = Line::from(vec![
            Span::styled(format!("{:<11}", "I/O"), Style::default().fg(theme.text)),
            Span::styled(
                format!("{:<8.2}%", psi.io_some.avg10),
                Style::default().fg(theme.accent),
            ),
            Span::styled(
                format!("{:<8.2}%", psi.io_some.avg60),
                Style::default().fg(theme.text),
            ),
            Span::styled(
                format!("{:<8.2}%", psi.io_some.avg300),
                Style::default().fg(theme.text),
            ),
        ]);

        let lines = vec![header, cpu_line, mem_line, io_line];
        f.render_widget(Paragraph::new(lines), area);
    }
}
